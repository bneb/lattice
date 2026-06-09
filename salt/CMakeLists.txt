cmake_minimum_required(VERSION 3.20)
project(salt-opt)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

option(ENABLE_COVERAGE "Enable code coverage instrumentation" OFF)
if(ENABLE_COVERAGE)
    add_compile_options(-fprofile-instr-generate -fcoverage-mapping -stdlib=libc++)
    add_link_options(-fprofile-instr-generate -fcoverage-mapping -stdlib=libc++)
endif()

if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    # add_compile_options(-fsanitize=address,undefined -fno-omit-frame-pointer)
    # add_link_options(-fsanitize=address,undefined)
endif()

# 1. Configuration
find_package(MLIR REQUIRED CONFIG)
find_package(LLVM REQUIRED CONFIG)
find_package(Z3 REQUIRED)

list(APPEND CMAKE_MODULE_PATH "${MLIR_CMAKE_DIR}")
list(APPEND CMAKE_MODULE_PATH "${LLVM_CMAKE_DIR}")

include_directories(${MLIR_INCLUDE_DIRS} ${LLVM_INCLUDE_DIRS})
link_directories(${MLIR_LIBRARY_DIRS} ${LLVM_LIBRARY_DIRS})

include(TableGen)
include(AddLLVM)
include(AddMLIR)
include(HandleLLVMOptions)

add_definitions(${LLVM_DEFINITIONS})
include_directories(${Z3_INCLUDE_DIR} ${Z3_INCLUDE_DIRS} /opt/homebrew/include)
include_directories(${CMAKE_CURRENT_SOURCE_DIR}/src/dialect)
include_directories(${CMAKE_CURRENT_BINARY_DIR}/src/dialect)

# 2. Salt Dialect
add_subdirectory(src/dialect)

# 3. Salt Optimizer & Backend
add_executable(salt-opt 
  src/main.cpp 
  src/passes/Z3Verify.cpp
  src/passes/LowerSalt.cpp
)

target_include_directories(salt-opt PRIVATE
  ${CMAKE_CURRENT_SOURCE_DIR}/src/dialect
  ${CMAKE_CURRENT_BINARY_DIR}/src/dialect
)

# 4. Link Dependencies
target_link_libraries(salt-opt PRIVATE
  SaltDialect
  
  # --- MLIR Core ---
  MLIRIR MLIRSupport MLIRParser
  MLIRBytecodeOpInterface MLIRCallInterfaces MLIRSideEffectInterfaces
  
  # --- Standard Dialects & Transforms ---
  MLIRArithDialect MLIRFuncDialect MLIRSCFDialect MLIRMemRefDialect MLIRControlFlowDialect
  MLIRMathDialect    # Required for cttz
  MLIRVectorDialect  # Required for Vector ops
  MLIRLLVMDialect    # Required for LLVM Dialect
  
  # --- Optimization Pipeline (Linalg/Tensor) ---
  MLIRLinalgDialect
  MLIRLinalgTransforms       # createConvertLinalgToLoopsPass
  MLIRBufferizationDialect
  MLIRBufferizationTransforms # createOneShotBufferizePass
  MLIRAffineDialect
  MLIRTensorDialect
  MLIRReconcileUnrealizedCasts # FIX: Added back
  MLIRVectorTransforms         # FIX: Added back just in case
  MLIRTilingInterface          # <--- Generic Interface
  MLIRSCFTransforms            # <--- Tiling Implementation
  MLIRTensorToLinalg           # <--- createConvertTensorToLinalgPass
  MLIRMemRefTransforms         # <--- createExpandStridedMetadataPass
  
  # --- Conversions (The Bridges) ---
  MLIRArithToLLVM
  MLIRMathToLLVM
  MLIRMemRefToLLVM
  MLIRFuncToLLVM
  MLIRSCFToControlFlow
  MLIRControlFlowToLLVM
  MLIRVectorToLLVM           # Base conversion logic
  MLIRVectorToLLVMPass       # <--- FIX: This contains the actual Pass creation function
  
  # --- LLVM Translation & CodeGen ---
  MLIRLLVMToLLVMIRTranslation MLIRBuiltinToLLVMIRTranslation
  MLIROpenACCToLLVMIRTranslation
  MLIROpenMPToLLVMIRTranslation
  MLIRArmNeonToLLVMIRTranslation
  MLIRArmSMEToLLVMIRTranslation
  MLIRArmSVEToLLVMIRTranslation
  MLIRGPUToLLVMIRTranslation
  MLIRNVVMToLLVMIRTranslation
  MLIRROCDLToLLVMIRTranslation
  MLIRSPIRVToLLVMIRTranslation
  MLIROptLib
  
  # --- LLVM Backend Libraries ---
  LLVM
  
  # --- External ---
  ${Z3_LIBRARIES}
)
