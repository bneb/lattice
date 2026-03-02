#!/bin/bash
# =============================================================================
# Docker entrypoint: Build salt-front + salt-opt from source
# =============================================================================
set -euo pipefail

echo "=== Building salt-front (Rust compiler frontend) ==="
cd /workspace/salt-front
cargo build --release

echo "=== Building salt-opt (MLIR/LLVM backend) ==="
cd /workspace/salt
mkdir -p build && cd build
cmake .. \
    -G Ninja \
    -DCMAKE_BUILD_TYPE=Release \
    -DMLIR_DIR="${MLIR_DIR}" \
    -DLLVM_DIR="${LLVM_DIR}" \
    -DZ3_INCLUDE_DIR=/usr/include \
    -DZ3_LIBRARIES=/usr/lib/x86_64-linux-gnu/libz3.so
ninja salt-opt

echo "=== Build complete ==="
echo "  salt-front: /workspace/salt-front/target/release/salt-front"
echo "  salt-opt:   /workspace/salt/build/salt-opt"
