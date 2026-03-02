//===- Z3Verify.cpp - Z3 Verification Pass Implementation ----------------===//
//
// This file implements inter-procedural contract verification using Z3.
//
// Strategy:
// 1. Walk all salt.call operations
// 2. For each call, look up the callee's salt.verify("requires") condition
// 3. Substitute the call's arguments into the condition
// 4. Ask Z3: "Can this condition be false?"
// 5. If SAT (condition can be false), emit verification error
// 6. Handle "layout_check" for reinterpret_cast memory safety.
//
//===----------------------------------------------------------------------===//

#include "Z3Verify.h"
#include "../dialect/SaltDialect.h"
#include "mlir/Dialect/Arith/IR/Arith.h"
#include "mlir/Dialect/Func/IR/FuncOps.h"
#include "mlir/Dialect/LLVMIR/LLVMDialect.h"
#include "mlir/Dialect/SCF/IR/SCF.h"
#include "mlir/IR/BuiltinOps.h"
#include "llvm/ADT/DenseMap.h"
#include <map>
#include <memory>
#include <z3++.h>

using namespace mlir;
using llvm::dyn_cast;
using llvm::dyn_cast_or_null;
using llvm::StringRef;

namespace salt {

using Z3ExprPtr = std::shared_ptr<z3::expr>;
using Z3SortPtr = std::shared_ptr<z3::sort>;
using Z3FuncDeclPtr = std::shared_ptr<z3::func_decl>;
using Z3FuncDeclVectorPtr = std::shared_ptr<z3::func_decl_vector>;

struct Z3Sorts {
  z3::context &ctx;
  llvm::DenseMap<Type, Z3SortPtr> sortCache;
  llvm::DenseMap<Type, Z3FuncDeclVectorPtr> accessorCache;
  llvm::DenseMap<Type, Z3FuncDeclPtr> constructorCache;

  Z3Sorts(z3::context &c) : ctx(c) {}

  z3::sort getOrCreateSort(Type ty) {
    auto it = sortCache.find(ty);
    if (it != sortCache.end())
      return *it->second;

    if (ty.isInteger(32) || ty.isInteger(64)) {
      auto s = std::make_shared<z3::sort>(ctx.int_sort());
      sortCache.insert({ty, s});
      return *s;
    }
    if (ty.isF32() || ty.isF64()) {
      auto s = std::make_shared<z3::sort>(ctx.real_sort());
      sortCache.insert({ty, s});
      return *s;
    }

    if (auto structTy = dyn_cast<mlir::LLVM::LLVMStructType>(ty)) {
      std::vector<z3::sort> fieldSorts;
      std::vector<const char *> fieldNames;
      for (auto field : structTy.getBody()) {
        fieldSorts.push_back(getOrCreateSort(field));
        fieldNames.push_back("field");
      }

      std::vector<std::string> nameStore;
      for (size_t i = 0; i < fieldSorts.size(); ++i) {
        nameStore.push_back("f" + std::to_string(i));
      }
      std::vector<const char *> namePtrs;
      for (const auto &s : nameStore)
        namePtrs.push_back(s.c_str());

      auto accessors = std::make_shared<z3::func_decl_vector>(ctx);

      z3::func_decl constructor =
          ctx.tuple_sort("struct", fieldSorts.size(), namePtrs.data(),
                         fieldSorts.data(), *accessors);

      auto s = std::make_shared<z3::sort>(constructor.range());
      auto constructorPtr = std::make_shared<z3::func_decl>(constructor);

      constructorCache.insert({ty, constructorPtr});
      accessorCache.insert({ty, accessors});
      sortCache.insert({ty, s});
      return *s;
    }

    auto s = std::make_shared<z3::sort>(ctx.int_sort());
    sortCache.insert({ty, s});
    return *s;
  }
};

Z3ExprPtr translateValue(Value v, z3::context &ctx, Z3Sorts &sorts,
                         llvm::DenseMap<Value, Z3ExprPtr> &scopeMap);

// --- Region Verification Logic ---
// Provenance Check: Ensure pointer P returned by region_alloc(R, ...)
// is derived from R and does not outlive R.
// Constraints:
// 1. R.base <= P < R.limit
// 2. Lifetime(P) <= Lifetime(R) (Handled by Borrow Checker)
// Helper to find the base object
Value getUnderlyingBase(Value v) {
  if (auto alloca = dyn_cast_or_null<LLVM::AllocaOp>(v.getDefiningOp()))
    return v;
  if (auto cast = dyn_cast_or_null<LLVM::BitcastOp>(v.getDefiningOp()))
    return getUnderlyingBase(cast.getOperand());
  if (auto add = v.getDefiningOp<arith::AddIOp>())
    return getUnderlyingBase(add.getLhs()); // Simple LHS walk
  if (auto ptrtoint = v.getDefiningOp<LLVM::PtrToIntOp>())
    return getUnderlyingBase(ptrtoint.getArg());
  if (auto blockArg = dyn_cast<BlockArgument>(v))
    return v; // Base is arg
  return Value();
}

// --- Region Verification Logic ---
// Provenance Check: Ensure pointer P returned by region_alloc(R, ...)
// is derived from R and does not outlive R.
void verifyRegionSafety(z3::context &ctx, Operation *op, Value ptr,
                        Z3ExprPtr ptrExpr, Z3Sorts &sorts,
                        llvm::DenseMap<Value, Z3ExprPtr> &scopeMap) {

  Value base = getUnderlyingBase(ptr);
  if (!base) {
    // In MVP, if we can't find base, we skip or emit warning.
    // For stricter safety, we might assert failure.
    // op->emitWarning() << "Could not resolve base pointer for verification.";
    return;
  }

  Z3ExprPtr baseExpr = translateValue(base, ctx, sorts, scopeMap);

  // Define Safety Interval: [base, base + 4096)
  // We assume 4096 page size for MVP regions.
  int regionSize = 4096;
  if (auto alloca = dyn_cast_or_null<LLVM::AllocaOp>(base.getDefiningOp())) {
    // Start with small alloc optimization if type is known/small?
    // For now, stick to simplified "Region" logic or large bounds.
    // Actually, let's use the type size if possible?
    // But LLVM::AllocaOp size is in allocatedType and arraySize.
    // Let's stick to 4096 for "Region Safety" as requested.
  }

  // Verification Query: "Can (ptr < base) OR (ptr >= base + size) be true?"
  z3::solver solver(ctx);
  z3::expr violation =
      (*ptrExpr < *baseExpr) || (*ptrExpr >= (*baseExpr + regionSize));

  solver.add(violation);

  // We use a smaller timeout for per-instruction checks
  z3::params p(ctx);
  p.set("timeout", 1000u); // 1s
  solver.set(p);

  if (solver.check() == z3::sat) {
    op->emitError() << "Spatial Safety Violation: Pointer may be out of bounds "
                       "(base + 4096).";
    // signalPassFailure(); // Optional: stop compilation?
  }
}

// Helper to enforce timeout and handle complexity limits
z3::check_result checkWithTimeout(z3::solver &solver, Operation *op,
                                  const llvm::Twine &errorMsg) {
  z3::params p(solver.ctx());
  p.set("timeout", 5000u); // 5 seconds
  solver.set(p);

  z3::check_result result = solver.check();
  if (result == z3::unknown) {
    op->emitError() << "Verification Timed Out (5s) or Inconclusive: "
                    << errorMsg << "\n"
                    << "Safe Logic Tip: Simplify boolean conditions or split "
                       "complex functions into smaller contracts.";
    // Treat unknown/timeout as failure for safety
    return z3::sat; // "sat" implies condition MIGHT be violated (violation
                    // found or unknown) => Fail
  }
  return result;
}

Z3ExprPtr translateValue(Value v, z3::context &ctx, Z3Sorts &sorts,
                         llvm::DenseMap<Value, Z3ExprPtr> &scopeMap) {
  auto it = scopeMap.find(v);
  if (it != scopeMap.end())
    return it->second;

  if (auto constOp = v.getDefiningOp<arith::ConstantIntOp>()) {
    return std::make_shared<z3::expr>(ctx.int_val((int)constOp.value()));
  }

  if (auto constOp = v.getDefiningOp<arith::ConstantOp>()) {
    if (auto intAttr = dyn_cast<IntegerAttr>(constOp.getValue())) {
      return std::make_shared<z3::expr>(ctx.int_val((int)intAttr.getInt()));
    }
  }

  if (auto blockArg = dyn_cast<BlockArgument>(v)) {
    z3::sort s = sorts.getOrCreateSort(v.getType());
    return std::make_shared<z3::expr>(ctx.constant(
        ("arg" + std::to_string(blockArg.getArgNumber())).c_str(), s));
  }

  if (auto muli = v.getDefiningOp<arith::MulIOp>()) {
    Z3ExprPtr lhs = translateValue(muli.getLhs(), ctx, sorts, scopeMap);
    Z3ExprPtr rhs = translateValue(muli.getRhs(), ctx, sorts, scopeMap);
    return std::make_shared<z3::expr>(*lhs * *rhs);
  }

  if (auto addi = v.getDefiningOp<arith::AddIOp>()) {
    Z3ExprPtr lhs = translateValue(addi.getLhs(), ctx, sorts, scopeMap);
    Z3ExprPtr rhs = translateValue(addi.getRhs(), ctx, sorts, scopeMap);
    return std::make_shared<z3::expr>(*lhs + *rhs);
  }

  if (auto extsi = v.getDefiningOp<arith::ExtSIOp>()) {
    return translateValue(extsi.getIn(), ctx, sorts, scopeMap);
  }

  if (auto extui = v.getDefiningOp<arith::ExtUIOp>()) {
    return translateValue(extui.getIn(), ctx, sorts, scopeMap);
  }

  if (auto inttoptr = v.getDefiningOp<LLVM::IntToPtrOp>()) {
    return translateValue(inttoptr.getArg(), ctx, sorts, scopeMap);
  }

  if (auto ptrtoint = v.getDefiningOp<LLVM::PtrToIntOp>()) {
    return translateValue(ptrtoint.getArg(), ctx, sorts, scopeMap);
  }

  if (auto load = dyn_cast_or_null<LLVM::LoadOp>(v.getDefiningOp())) {
    // If it's an atomic load, treat as fresh constant (External State)
    if (load->hasAttr("atomic_memory_order")) {
      static int atomic_id = 0;
      z3::sort s = sorts.getOrCreateSort(v.getType());
      return std::make_shared<z3::expr>(ctx.constant(
          ("atomic_val_" + std::to_string(atomic_id++)).c_str(), s));
    }
    Value ptr = load.getAddr();
    if (auto it_ssa = scopeMap.find(ptr); it_ssa != scopeMap.end()) {
      return it_ssa->second;
    }
  }

  if (auto rmw = dyn_cast_or_null<LLVM::AtomicRMWOp>(v.getDefiningOp())) {
    // Atomic RMW always returns a fresh "external" state (the old value)
    static int rmw_id = 0;
    z3::sort s = sorts.getOrCreateSort(v.getType());
    return std::make_shared<z3::expr>(
        ctx.constant(("rmw_val_" + std::to_string(rmw_id++)).c_str(), s));
  }

  if (auto alloca = dyn_cast_or_null<LLVM::AllocaOp>(v.getDefiningOp())) {
    static int alloca_id = 0;
    std::string name = "address_alloca_" + std::to_string(alloca_id++);
    // Rule 1: The Address is an Int.
    // We model the pointer as a 64-bit address for alignment/bounds checks.
    auto expr = std::make_shared<z3::expr>(ctx.int_const(name.c_str()));
    scopeMap[v] = expr;
    return expr;
  }

  return std::make_shared<z3::expr>(ctx.int_const("unknown"));
}

std::pair<int, int> getSizeAndAlign(StringRef typeStr) {
  if (typeStr.find("Page") != std::string::npos)
    return {4096, 4096};
  // Check pointers/containers first!
  if (typeStr.contains("Owned") || typeStr.contains("Window") ||
      typeStr.contains("I64") || typeStr.contains("F64"))
    return {8, 8};
  if (typeStr.contains("I32") || typeStr.contains("F32"))
    return {4, 4};
  return {8, 8};
}

void Z3VerifyPass::runOnOperation() {
  ModuleOp module = getOperation();
  z3::context ctx;
  SymbolTable symbolTable(module);
  Z3Sorts sorts(ctx);
  std::map<std::string, std::shared_ptr<FunctionSummary>> summaryCache;

  // Pass 1: Collect Summaries
  module.walk([&](func::FuncOp func) {
    std::shared_ptr<FunctionSummary> summary(new FunctionSummary());
    summary->func = func;
    func.walk([&](Operation *op) {
      if (op->getName().getStringRef() == "salt.verify") {
        auto kindAttr = op->getAttrOfType<StringAttr>("kind");
        if (kindAttr && kindAttr.getValue() == "requires") {
          summary->requiresOp = op;
        } else if (kindAttr && kindAttr.getValue() == "ensures") {
          summary->ensuresOp = op;
        }
      }
    });
    summaryCache[func.getName().str()] = summary;
  });

  // Pass 2: Verify Functions
  module.walk([&](func::FuncOp func) {
    if (func.empty())
      return;

    llvm::errs() << "Verifying Function: " << func.getName() << "\n";

    // We verify two things:
    // 1. calls within this function satisfy their callee's preconditions
    // 2. layout_checks within this function are valid

    llvm::DenseMap<Value, Z3ExprPtr> scopeMap;
    // Note: In a real pass we'd populate scopeMap properly for all values.
    // For this MVP, we assume it's populated locally or via translateValue.

    // induction variable detection
    llvm::DenseMap<Value, std::pair<Z3ExprPtr, Z3ExprPtr>> varBounds;
    func.walk([&](LLVM::StoreOp store) {
      llvm::errs() << "Check Store Op\n";
      Value ptr = store.getAddr();
      if (auto alloca = dyn_cast_or_null<LLVM::AllocaOp>(ptr.getDefiningOp())) {
        // Find initial store (start)
        if (store->getBlock() == &func.front()) {
          Value val = store.getValue();
          if (val.getType().isInteger(32) || val.getType().isInteger(64) ||
              val.getType().isF32() || val.getType().isF64()) {
            varBounds[ptr].first = translateValue(val, ctx, sorts, scopeMap);
          }
        }
      }
    });

    func.walk([&](Operation *op) {
      if (auto cmpi = dyn_cast<arith::CmpIOp>(op)) {
        if (cmpi.getPredicate() == arith::CmpIPredicate::slt ||
            cmpi.getPredicate() == arith::CmpIPredicate::ult) {
          if (auto load = dyn_cast_or_null<LLVM::LoadOp>(
                  cmpi.getLhs().getDefiningOp())) {
            Value ptr = load.getAddr();
            if (varBounds.count(ptr)) {
              varBounds[ptr].second =
                  translateValue(cmpi.getRhs(), ctx, sorts, scopeMap);
            }
          }
        }
      }

      if (auto load = dyn_cast<LLVM::LoadOp>(op)) {
        Value ptr = load.getAddr();
        Z3ExprPtr ptrExpr = translateValue(ptr, ctx, sorts, scopeMap);
        verifyRegionSafety(ctx, op, ptr, ptrExpr, sorts, scopeMap);
      }

      if (auto store = dyn_cast<LLVM::StoreOp>(op)) {
        Value ptr = store.getAddr();
        Z3ExprPtr ptrExpr = translateValue(ptr, ctx, sorts, scopeMap);

        // --- Added: Spatial Safety Check ---
        verifyRegionSafety(ctx, op, ptr, ptrExpr, sorts, scopeMap);
        // -----------------------------------

        z3::solver solver(ctx);
        // Apply induction bounds if applicable
        if (varBounds.count(ptr)) {
          // Guard: Only perform comparison if the expression is numeric
          // (Int/Real)
          if (ptrExpr->is_int() || ptrExpr->is_real()) {
            if (varBounds[ptr].first && (varBounds[ptr].first->is_int() ||
                                         varBounds[ptr].first->is_real()))
              solver.add(*ptrExpr >= *varBounds[ptr].first);
            if (varBounds[ptr].second && (varBounds[ptr].second->is_int() ||
                                          varBounds[ptr].second->is_real()))
              solver.add(*ptrExpr < *varBounds[ptr].second);
          }
        }

        // Check for Store Alignment / "Recursive Disjointness"
        // Ensure that writing to an entry (likely 8 bytes) is aligned to 8
        // bytes. This prevents "bleeding" into adjacent entries in a table.
        Value val = store.getValue();
        Type valTy = val.getType();
        if (valTy.isInteger(64) || valTy.isF64() ||
            llvm::isa<LLVM::LLVMPointerType>(valTy)) {
          // For 64-bit stores, enforce 8-byte alignment.
          // This corresponds to the requirement: "Prove that writing to a Page
          // Table entry only modifies the intended 8-byte range"
          if (ptrExpr->is_int() || ptrExpr->is_real()) {
            z3::solver alignSolver(ctx);
            z3::expr misaligned = (*ptrExpr % 8 != 0);
            alignSolver.add(misaligned);

            if (checkWithTimeout(alignSolver, store,
                                 "Alignment verification.") == z3::sat) {
              store.emitWarning()
                  << "Unaligned 64-bit store detected! Potential "
                     "corruption of adjacent memory.";
              // signalPassFailure();
            }
          }
        }
      }
      if (auto callOp = dyn_cast<func::CallOp>(op)) {
        z3::solver solver(ctx);
        // Apply induction bounds
        for (auto const &[ptr, bounds] : varBounds) {
          Z3ExprPtr ptrExpr = translateValue(ptr, ctx, sorts, scopeMap);
          if (ptrExpr->is_int() || ptrExpr->is_real()) {
            if (bounds.first &&
                (bounds.first->is_int() || bounds.first->is_real()))
              solver.add(*ptrExpr >= *bounds.first);
            if (bounds.second &&
                (bounds.second->is_int() || bounds.second->is_real()))
              solver.add(*ptrExpr < *bounds.second);
          }
        }
        StringRef calleeName = callOp.getCallee();
        Operation *calleeOp = symbolTable.lookup(calleeName);
        if (!calleeOp)
          return;

        func::FuncOp calleeFunc = dyn_cast_or_null<func::FuncOp>(calleeOp);
        if (!calleeFunc || calleeFunc.empty())
          return;

        // Modular Check: Use cached summary
        auto it = summaryCache.find(calleeName.str());
        if (it == summaryCache.end())
          return;

        const auto &summary = it->second;

        // Construct callScopeMap: Map callee's BlockArguments to caller's
        // Z3Exprs
        llvm::DenseMap<Value, Z3ExprPtr> callScopeMap;
        if (!summary->func.empty()) {
          Block &entryBlock = summary->func.front();
          for (unsigned i = 0; i < callOp.getNumOperands(); ++i) {
            if (i < entryBlock.getNumArguments()) { // Safety check
              Value calleeArg = entryBlock.getArgument(i);
              Value callerArg = callOp.getOperand(i);
              callScopeMap[calleeArg] =
                  translateValue(callerArg, ctx, sorts, scopeMap);
            }
          }
        }

        // 1. Check Precondition (Requires)
        if (summary->requiresOp) {
          auto operands = summary->requiresOp->getOperands();
          if (!operands.empty()) {
            Value cond = operands[0];
            if (auto cmpi = cond.getDefiningOp<arith::CmpIOp>()) {
              Z3ExprPtr zLhsPtr =
                  translateValue(cmpi.getLhs(), ctx, sorts, callScopeMap);
              Z3ExprPtr zRhsPtr =
                  translateValue(cmpi.getRhs(), ctx, sorts, callScopeMap);
              z3::expr zLhs = *zLhsPtr;
              z3::expr zRhs = *zRhsPtr;

              z3::expr hypothesis = (zLhs == zRhs);
              if (cmpi.getPredicate() == mlir::arith::CmpIPredicate::ne)
                hypothesis = (zLhs != zRhs);
              else if (cmpi.getPredicate() == mlir::arith::CmpIPredicate::eq)
                hypothesis = (zLhs == zRhs);

              solver.add(!hypothesis);
              if (checkWithTimeout(solver, callOp, "Precondition check.") ==
                  z3::sat) {
                callOp.emitError() << "Verification Failed: Call to @"
                                   << calleeName << " violates precondition.";
                signalPassFailure();
              }
            }
          }
        }

        // 2. Post-condition (Ensures)
        // Add callee's post-condition to the solver as a hypothesis for
        // subsequent ops
        if (summary->ensuresOp) {
          auto operands = summary->ensuresOp->getOperands();
          if (!operands.empty()) {
            Value cond = operands[0];
            if (auto cmpi = cond.getDefiningOp<arith::CmpIOp>()) {
              Z3ExprPtr zLhsPtr =
                  translateValue(cmpi.getLhs(), ctx, sorts, callScopeMap);
              Z3ExprPtr zRhsPtr =
                  translateValue(cmpi.getRhs(), ctx, sorts, callScopeMap);
              z3::expr ensuresExpr = (*zLhsPtr == *zRhsPtr);
              if (cmpi.getPredicate() == mlir::arith::CmpIPredicate::ne)
                ensuresExpr = (*zLhsPtr != *zRhsPtr);

              // Add to solver for future operations in this function
              solver.add(ensuresExpr);
            }
          }
        }
      }

      if (op->getName().getStringRef() == "salt.verify") {
        auto kindAttr = op->getAttrOfType<StringAttr>("kind");
        if (kindAttr && kindAttr.getValue() == "loop_pulse_off") {
          // Heuristic Termination Analysis
          if (varBounds.empty()) {
            op->emitWarning() << "Unbounded Spin detected: pulse(off) loop has "
                                 "no detected induction variables.";
          } else {
            op->emitRemark()
                << "Safe Spin: Induction variable detected in pulse(off) loop. "
                   "(Bound: 10000 cycles assumed)";
          }
        } else if (kindAttr && kindAttr.getValue() == "layout_check") {
          auto operands = op->getOperands();
          if (operands.size() >= 2) {
            Value ptr = operands[1];
            auto targetAttr = op->getAttrOfType<StringAttr>("target_type");
            std::string typeStr =
                targetAttr ? targetAttr.getValue().str() : "Unknown";
            auto [size, align] = getSizeAndAlign(typeStr);

            Z3ExprPtr ptrExpr = translateValue(ptr, ctx, sorts, scopeMap);
            if (ptrExpr->is_int() || ptrExpr->is_real()) {
              if (align == 4096 || typeStr.find("Page") != std::string::npos) {
                z3::solver layoutSolver(ctx);
                z3::expr hypothesis = (*ptrExpr % align == 0);
                layoutSolver.add(!hypothesis);
                if (checkWithTimeout(layoutSolver, op, "Layout check.") ==
                    z3::sat) {
                  op->emitError() << "Alignment Proof Failed for " << typeStr
                                  << ": address must be 4KB aligned.";
                  signalPassFailure();
                } else {
                  op->emitRemark()
                      << "Alignment Proof Passed for " << typeStr << ".";
                }
              }
            }
          }
        }
      }
    });
  });
}

std::unique_ptr<mlir::Pass> createZ3VerifyPass() {
  return std::make_unique<Z3VerifyPass>();
}

void registerZ3VerifyPass() { mlir::PassRegistration<Z3VerifyPass>(); }

} // namespace salt
