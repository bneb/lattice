# Salt Dialect CMake Configuration
# Proper TableGen generation for Salt MLIR Dialect

set(LLVM_TARGET_DEFINITIONS SaltDialect.td)

# Generate dialect and operation declarations
mlir_tablegen(SaltDialect.h.inc -gen-dialect-decls -dialect=salt)
mlir_tablegen(SaltDialect.cpp.inc -gen-dialect-defs -dialect=salt)
mlir_tablegen(SaltTypes.h.inc -gen-typedef-decls -typedefs-dialect=salt)
mlir_tablegen(SaltTypes.cpp.inc -gen-typedef-defs -typedefs-dialect=salt)
mlir_tablegen(SaltOps.h.inc -gen-op-decls)
mlir_tablegen(SaltOps.cpp.inc -gen-op-defs)

add_public_tablegen_target(SaltOpsIncGen)

# Create the dialect library
add_mlir_dialect_library(SaltDialect
  SaltDialect.cpp
  SaltOps.cpp
  
  ADDITIONAL_HEADER_DIRS
  ${CMAKE_CURRENT_SOURCE_DIR}
  
  DEPENDS
  SaltOpsIncGen
  
  LINK_LIBS PUBLIC
  MLIRIR
  MLIRSupport
  MLIRFuncDialect
)
