#include "mlir/IR/MLIRContext.h"
#include "mlir/Parser/Parser.h"
#include "mlir/Pass/Pass.h"
#include "mlir/Pass/PassManager.h"
#include "mlir/Support/FileUtilities.h"
#include "mlir/Target/LLVMIR/Dialect/All.h"
#include "mlir/Target/LLVMIR/Dialect/Builtin/BuiltinToLLVMIRTranslation.h"
#include "mlir/Target/LLVMIR/Dialect/LLVMIR/LLVMToLLVMIRTranslation.h"
#include "mlir/Target/LLVMIR/Export.h"
#include "mlir/Tools/mlir-opt/MlirOptMain.h"
#include "llvm/InitializePasses.h"
#include "llvm/LinkAllPasses.h"

// Specific Dialects
#include "mlir/Dialect/Arith/IR/Arith.h"
#include "mlir/Dialect/ControlFlow/IR/ControlFlowOps.h"
#include "mlir/Dialect/Func/IR/FuncOps.h"
#include "mlir/Dialect/LLVMIR/LLVMDialect.h"
#include "mlir/Dialect/Math/IR/Math.h"
#include "mlir/Dialect/MemRef/IR/MemRef.h"
#include "mlir/Dialect/MemRef/Transforms/Passes.h"
#include "mlir/Dialect/SCF/IR/SCF.h"

// Conversion Passes
#include "mlir/Conversion/ArithToLLVM/ArithToLLVM.h"
#include "mlir/Conversion/ControlFlowToLLVM/ControlFlowToLLVM.h"
#include "mlir/Conversion/FuncToLLVM/ConvertFuncToLLVMPass.h"
#include "mlir/Conversion/MathToLLVM/MathToLLVM.h"
#include "mlir/Conversion/MemRefToLLVM/MemRefToLLVM.h"
#include "mlir/Conversion/Passes.h"
#include "mlir/Conversion/ReconcileUnrealizedCasts/ReconcileUnrealizedCasts.h"
#include "mlir/Conversion/SCFToControlFlow/SCFToControlFlow.h"
#include "mlir/Conversion/VectorToLLVM/ConvertVectorToLLVM.h"

// Linalg & Tensors
#include "mlir/Dialect/Affine/IR/AffineOps.h"
#include "mlir/Dialect/Bufferization/IR/Bufferization.h"
#include "mlir/Dialect/Bufferization/Transforms/OneShotAnalysis.h"
#include "mlir/Dialect/Bufferization/Transforms/Passes.h"
#include "mlir/Dialect/Linalg/IR/Linalg.h"
#include "mlir/Dialect/Linalg/Passes.h"
#include "mlir/Dialect/Linalg/Transforms/Transforms.h"
#include "mlir/Dialect/Tensor/IR/Tensor.h"
#include "mlir/Dialect/Tensor/Transforms/BufferizableOpInterfaceImpl.h"
#include "mlir/Dialect/Vector/IR/VectorOps.h"

// Optimization Passes
#include "mlir/Transforms/Passes.h"
#include "passes/Z3Verify.h"

// LLVM CodeGen
#include "llvm/Analysis/TargetLibraryInfo.h"
#include "llvm/Analysis/TargetTransformInfo.h"
#include "llvm/IR/LegacyPassManager.h"
#include "llvm/IR/Module.h"
#include "llvm/InitializePasses.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/PassRegistry.h"
#include "llvm/Support/CommandLine.h"
#include "llvm/Support/FileSystem.h"
#include "llvm/Support/SourceMgr.h"
#include "llvm/Support/TargetSelect.h"
#include "llvm/Support/ToolOutputFile.h"
#include "llvm/Target/TargetMachine.h"
#include "llvm/TargetParser/Host.h"
#include "llvm/TargetParser/Triple.h"

#include "dialect/SaltDialect.h"
#include <optional>

namespace cl = llvm::cl;

static cl::opt<bool> EmitObj("emit-obj", cl::desc("Emit object file (.o)"),
                             cl::init(false));
static cl::opt<bool> EmitLLVM("emit-llvm", cl::desc("Emit LLVM IR (.ll)"),
                              cl::init(false));

static cl::opt<std::string> OutputFilename("output",
                                           cl::desc("Output filename"),
                                           cl::value_desc("filename"),
                                           cl::init("-"));
static cl::opt<std::string>
    InputFilename(cl::Positional, cl::desc("<input file>"), cl::init("-"));

static cl::opt<bool> RunVerification("verify",
                                     cl::desc("Run formal verification (Z3)"),
                                     cl::init(true));

// Declare the factory function (implemented in LowerSalt.cpp)
std::unique_ptr<mlir::Pass> createLowerSaltPass();

// Declare GenericTilerPass
std::unique_ptr<mlir::Pass> createGenericTilerPass();

// Returns 0 on success, 1 on failure
int emitObjectFile(llvm::Module &llvmModule,
                   const std::string &outputFilename) {
  // 1. Setup Target Triple
  auto tripleStr = llvm::sys::getDefaultTargetTriple();
  llvm::Triple triple(tripleStr);
  llvmModule.setTargetTriple(triple);

  std::string error;
  auto target = llvm::TargetRegistry::lookupTarget(triple.str(), error);
  if (!target) {
    llvm::errs() << "Target Lookup Failed: " << error << "\n";
    return 1;
  }

  // 2. Configure Target Machine
  std::string cpu = "generic";
  std::string features = "";
  llvm::TargetOptions opt;
  auto rm = std::optional<llvm::Reloc::Model>();
  auto targetMachine =
      target->createTargetMachine(triple, cpu, features, opt, rm);

  // 3. Configure Data Layout
  llvmModule.setDataLayout(targetMachine->createDataLayout());

  // 4. Open Output File
  std::error_code ec;
  llvm::ToolOutputFile out(outputFilename, ec, llvm::sys::fs::OF_None);
  if (ec) {
    llvm::errs() << "File Error: " << ec.message() << "\n";
    return 1;
  }

  // 5. Emit Object Code
  llvm::legacy::PassManager pass;
  auto fileType = llvm::CodeGenFileType::ObjectFile;

  // Add TLI and TTI (inherited from original implementation)
  llvm::TargetLibraryInfoImpl tlii(triple);
  pass.add(new llvm::TargetLibraryInfoWrapperPass(tlii));
  pass.add(llvm::createTargetTransformInfoWrapperPass(
      targetMachine->getTargetIRAnalysis()));

  if (targetMachine->addPassesToEmitFile(pass, out.os(), nullptr, fileType)) {
    llvm::errs() << "Error: TargetMachine can't emit a file of this type\n";
    return 1;
  }

  pass.run(llvmModule);
  out.keep();
  return 0;
}

void buildLoweringPipeline(mlir::PassManager &pm, mlir::ModuleOp module) {
  llvm::errs() << "DEBUG: Building Lowering Pipeline\n";

  // Detect if the module uses tensor/linalg/vector dialect ops.
  // Salt kernel MLIR only uses func/arith/cf/llvm — applying
  // tensor/bufferization passes to this IR causes segfaults on cf.br back-edges
  // in mixed-dialect functions.
  bool needsTensorPipeline = false;
  module.walk([&](mlir::Operation *op) {
    auto dialectNamespace = op->getDialect()->getNamespace();
    if (dialectNamespace == "tensor" || dialectNamespace == "linalg" ||
        dialectNamespace == "bufferization" || dialectNamespace == "vector") {
      needsTensorPipeline = true;
      return mlir::WalkResult::interrupt();
    }
    return mlir::WalkResult::advance();
  });

  // Pre-processing (module level)
  pm.addPass(mlir::createCanonicalizerPass());
  pm.addPass(mlir::createCSEPass());

  // 0. Formal Verification
  if (RunVerification) {
    pm.addPass(salt::createZ3VerifyPass());
  }

  // 1. Lower Salt High-Level Ops EARLY
  pm.addPass(createLowerSaltPass());

  if (needsTensorPipeline) {
    llvm::errs() << "DEBUG: Using full tensor/linalg lowering pipeline\n";
    // A2. Convert remaining tensor ops to linalg
    pm.addPass(mlir::createConvertTensorToLinalgPass());

    // B. "Bufferize" (Tensor -> MemRef)
    pm.addPass(mlir::bufferization::createEmptyTensorToAllocTensorPass());

    mlir::bufferization::OneShotBufferizePassOptions bufferizationOpts;
    bufferizationOpts.bufferizeFunctionBoundaries = true;
    bufferizationOpts.allowUnknownOps = true;
    pm.addPass(
        mlir::bufferization::createOneShotBufferizePass(bufferizationOpts));

    // B2. Expand strided metadata from tiled subviews
    pm.addPass(mlir::memref::createExpandStridedMetadataPass());

    // C. Lower Linalg to Loops (Stable)
    pm.addPass(mlir::createConvertLinalgToLoopsPass());

    // 4. Vector Lowering
    pm.addPass(mlir::createConvertVectorToLLVMPass());
  } else {
    llvm::errs()
        << "DEBUG: Using kernel fast-path (no tensor/linalg ops detected)\n";
  }

  // 5. Standard Lowering (scf/cf/arith/math/memref to LLVM)
  pm.addPass(mlir::createSCFToControlFlowPass());
  pm.addPass(mlir::createConvertControlFlowToLLVMPass());
  pm.addPass(mlir::createArithToLLVMConversionPass());
  pm.addPass(mlir::createConvertMathToLLVMPass());
  pm.addPass(mlir::createFinalizeMemRefToLLVMConversionPass());

  // 6. FuncToLLVM - must run LAST after all memref/arith lowering
  pm.addPass(mlir::createConvertFuncToLLVMPass());

  pm.addPass(mlir::createReconcileUnrealizedCastsPass());
}

void buildOptimizationPipeline(mlir::PassManager &pm) {
  pm.addPass(mlir::createCanonicalizerPass());
  pm.addPass(mlir::createCSEPass());
  pm.addPass(mlir::createInlinerPass());
  pm.addPass(mlir::createSymbolDCEPass());
  pm.addPass(mlir::createLoopInvariantCodeMotionPass());
  pm.addPass(mlir::createStripDebugInfoPass());
  // pm.addPass(mlir::createSuperVectorizePass({})); // Optional: Vectorization
}

int main(int argc, char **argv) {
  // 0. Register custom passes FIRST (before CLI parsing)
  salt::registerZ3VerifyPass();

  // 1. Initialize LLVM Targets
  llvm::InitializeAllTargetInfos();
  llvm::InitializeAllTargets();
  llvm::InitializeAllTargetMCs();
  llvm::InitializeAllAsmParsers();
  llvm::InitializeAllAsmPrinters();

  // 2. Initialize Common Passes
  auto &registry = *llvm::PassRegistry::getPassRegistry();
  llvm::initializeCore(registry);
  llvm::initializeCodeGen(registry);
  llvm::initializeTarget(registry); // Generic target passes
  llvm::initializeAnalysis(registry);
  llvm::initializeTransformUtils(registry);
  llvm::initializeInstCombine(registry);
  llvm::initializeScalarOpts(registry);
  llvm::initializeVectorization(registry);
  llvm::initializeIPO(registry);

  // 3. Register MLIR
  mlir::DialectRegistry mlirRegistry;
  mlirRegistry.insert<salt::SaltDialect>();
  mlirRegistry.insert<mlir::func::FuncDialect, mlir::arith::ArithDialect,
                      mlir::scf::SCFDialect, mlir::memref::MemRefDialect,
                      mlir::cf::ControlFlowDialect, mlir::LLVM::LLVMDialect,
                      mlir::linalg::LinalgDialect, mlir::tensor::TensorDialect,
                      mlir::vector::VectorDialect, mlir::affine::AffineDialect,
                      mlir::math::MathDialect,
                      mlir::bufferization::BufferizationDialect>();

  // Register tensor bufferization patterns (tensor.extract -> memref.load etc.)
  mlir::tensor::registerBufferizableOpInterfaceExternalModels(mlirRegistry);
  mlir::registerAllToLLVMIRTranslations(mlirRegistry);
  mlir::registerBuiltinDialectTranslation(mlirRegistry);

  cl::ParseCommandLineOptions(argc, argv, "Salt Optimizer & Backend\n");

  if (!EmitObj && !EmitLLVM) {
    return mlir::asMainReturnCode(
        mlir::MlirOptMain(argc, argv, "Salt Optimizer", mlirRegistry));
  }

  // Backend Pipeline
  mlir::MLIRContext context(mlirRegistry);

  // Optimization Pipeline (Always enable for now or check flag)
  // Ideally checking opt level. Assuming -O3 behavior for benchmarks.

  std::string errorMessage;
  auto file = mlir::openInputFile(InputFilename, &errorMessage);
  if (!file) {
    llvm::errs() << errorMessage << "\n";
    return 1;
  }

  llvm::SourceMgr sourceMgr;
  sourceMgr.AddNewSourceBuffer(std::move(file), llvm::SMLoc());
  auto module = mlir::parseSourceFile<mlir::ModuleOp>(sourceMgr, &context);
  if (!module)
    return 1;

  mlir::PassManager pm(&context);

  // Apply optimizations FIRST
  llvm::errs() << "DEBUG: Starting Optimization Pipeline\n";
  buildOptimizationPipeline(pm);

  llvm::errs() << "DEBUG: Starting Lowering Pipeline\n";
  buildLoweringPipeline(pm, *module);
  if (mlir::failed(pm.run(*module))) {
    llvm::errs() << "Lowering failed.\n";
    return 1;
  }
  llvm::errs() << "DEBUG: Lowering Complete\n";

  llvm::LLVMContext llvmContext;
  llvm::errs() << "DEBUG: Starting LLVM Translation\n";
  auto llvmModule = mlir::translateModuleToLLVMIR(*module, llvmContext);
  if (!llvmModule) {
    llvm::errs() << "LLVM Translation failed.\n";
    return 1;
  }
  llvm::errs() << "DEBUG: LLVM Translation Complete\n";

  if (EmitLLVM) {
    std::error_code ec;
    llvm::ToolOutputFile out(OutputFilename, ec, llvm::sys::fs::OF_None);
    if (ec) {
      llvm::errs() << "File Error: " << ec.message() << "\n";
      return 1;
    }
    llvmModule->print(out.os(), nullptr);
    out.os().flush();
    out.keep();
    // Force exit to avoid destructor hangs in Z3/LLVM static objects
    exit(0);
  }

  // Emit Object File (Utilizing the new helper)
  if (EmitObj) {
    int ret = emitObjectFile(*llvmModule, OutputFilename);
    exit(ret);
  }

  exit(0);
}
