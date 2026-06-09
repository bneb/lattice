#include "SaltDialect.h"
#include "SaltOps.h"
#include "mlir/Conversion/FuncToLLVM/ConvertFuncToLLVM.h"
#include "mlir/Conversion/LLVMCommon/Pattern.h"
#include "mlir/Conversion/LLVMCommon/TypeConverter.h"
#include "mlir/Dialect/Func/IR/FuncOps.h"
#include "mlir/Dialect/LLVMIR/LLVMDialect.h"
#include "mlir/Pass/Pass.h"
#include "mlir/Transforms/DialectConversion.h"
// Include dialects to mark as legal
#include "mlir/Dialect/Arith/IR/Arith.h"
#include "mlir/Dialect/Bufferization/IR/Bufferization.h"
#include "mlir/Dialect/ControlFlow/IR/ControlFlowOps.h"
#include "mlir/Dialect/Linalg/IR/Linalg.h"
#include "mlir/Dialect/MemRef/IR/MemRef.h"
#include "mlir/Dialect/SCF/IR/SCF.h"
#include "mlir/Dialect/Tensor/IR/Tensor.h"

using namespace mlir;

namespace {

class SaltToLLVMTypeConverter : public LLVMTypeConverter {
public:
  SaltToLLVMTypeConverter(MLIRContext *ctx) : LLVMTypeConverter(ctx) {
    addConversion([&](salt::RegionType type) {
      return LLVM::LLVMPointerType::get(type.getContext());
    });
  }
};

struct VerifyOpLowering : public ConvertOpToLLVMPattern<salt::VerifyOp> {
  using ConvertOpToLLVMPattern<salt::VerifyOp>::ConvertOpToLLVMPattern;
  LogicalResult
  matchAndRewrite(salt::VerifyOp op, OpAdaptor adaptor,
                  ConversionPatternRewriter &rewriter) const override {
    rewriter.eraseOp(op);
    return success();
  }
};

struct CallOpLowering : public ConvertOpToLLVMPattern<salt::CallOp> {
  using ConvertOpToLLVMPattern<salt::CallOp>::ConvertOpToLLVMPattern;
  LogicalResult
  matchAndRewrite(salt::CallOp op, OpAdaptor adaptor,
                  ConversionPatternRewriter &rewriter) const override {
    SmallVector<Type> resultTypes;
    if (failed(typeConverter->convertTypes(op.getResultTypes(), resultTypes)))
      return failure();
    rewriter.replaceOpWithNewOp<LLVM::CallOp>(
        op, resultTypes, op.getCalleeAttr(), adaptor.getOperands());
    return success();
  }
};

struct SaltFuncSignatureConversion : public OpConversionPattern<func::FuncOp> {
  using OpConversionPattern<func::FuncOp>::OpConversionPattern;
  LogicalResult
  matchAndRewrite(func::FuncOp op, OpAdaptor adaptor,
                  ConversionPatternRewriter &rewriter) const override {
    auto funcType = op.getFunctionType();
    TypeConverter::SignatureConversion result(funcType.getNumInputs());
    if (failed(
            typeConverter->convertSignatureArgs(funcType.getInputs(), result)))
      return failure();

    SmallVector<Type, 1> newResults;
    if (failed(typeConverter->convertTypes(funcType.getResults(), newResults)))
      return failure();

    auto newFuncType = FunctionType::get(
        op.getContext(), result.getConvertedTypes(), newResults);

    if (newFuncType == funcType)
      return failure();

    rewriter.modifyOpInPlace(op, [&] {
      op.setFunctionType(newFuncType);
      if (!op.isExternal()) {
        rewriter.applySignatureConversion(&op.getBody().front(), result);
      }
    });
    return success();
  }
};

struct LowerSaltPass
    : public PassWrapper<LowerSaltPass, OperationPass<ModuleOp>> {
  MLIR_DEFINE_EXPLICIT_INTERNAL_INLINE_TYPE_ID(LowerSaltPass)

  llvm::StringRef getArgument() const override { return "lower-salt"; }
  llvm::StringRef getDescription() const override {
    return "Lowers Salt Dialect to LLVM Dialect";
  }

  void getDependentDialects(DialectRegistry &registry) const override {
    registry.insert<LLVM::LLVMDialect>();
    registry.insert<func::FuncDialect>();
    registry.insert<arith::ArithDialect>();
    registry.insert<scf::SCFDialect>();
    registry.insert<cf::ControlFlowDialect>();
  }

  void runOnOperation() override {
    ModuleOp m = getOperation();
    SaltToLLVMTypeConverter converter(&getContext());
    RewritePatternSet patterns(&getContext());

    patterns.add<VerifyOpLowering>(converter);
    patterns.add<CallOpLowering>(converter);
    patterns.add<SaltFuncSignatureConversion>(converter, &getContext());
    // NOTE: FuncToLLVM is now handled by the standard pipeline pass, not here
    // populateFuncToLLVMConversionPatterns was causing type mismatches with
    // memref lowering

    ConversionTarget target(getContext());
    target.addLegalDialect<LLVM::LLVMDialect>();
    target.addLegalDialect<arith::ArithDialect>();
    target.addLegalDialect<scf::SCFDialect>();
    target.addLegalDialect<cf::ControlFlowDialect>();
    target.addDynamicallyLegalOp<func::FuncOp>([&](func::FuncOp op) {
      return converter.isSignatureLegal(op.getFunctionType());
    });
    target.addLegalDialect<linalg::LinalgDialect>(); // Still in IR when we run
    target.addLegalDialect<tensor::TensorDialect>(); // Still in IR when we run
    target.addLegalDialect<memref::MemRefDialect>(); // Still in IR when we run
    target.addLegalDialect<
        bufferization::BufferizationDialect>(); // Bufferization
                                                // ops

    target.addIllegalDialect<salt::SaltDialect>();

    // Explicitly allow ModuleOp
    target.addLegalOp<ModuleOp>();

    if (failed(applyPartialConversion(m, target, std::move(patterns))))
      signalPassFailure();
  }
};

} // namespace

std::unique_ptr<mlir::Pass> createLowerSaltPass() {
  return std::make_unique<LowerSaltPass>();
}

static mlir::PassRegistration<LowerSaltPass> reg;
