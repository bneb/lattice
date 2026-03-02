#!/bin/bash
set -euo pipefail

# =============================================================================
# Basalt WASM Build Script
# =============================================================================
# Builds basalt.wasm from Salt sources through the full MLIR pipeline.
# Output: basalt/wasm/dist/basalt.wasm (22KB, 6 exports)
#
# Requirements: LLVM 21 (clang + wasm-ld), salt-front (release build)
# =============================================================================

LLVM_VERSION="${LLVM_VERSION:-21}"
export PATH="/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin:$PATH"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SALT_FRONT="$PROJECT_ROOT/salt-front"

OUT_DIR="/tmp/salt_build"
mkdir -p "$OUT_DIR"

# 1. Concatenate Salt sources (same as build_basalt.sh)
COMBINED_SRC="$OUT_DIR/basalt_combined.salt"
echo "// Auto-generated build file for Basalt" > "$COMBINED_SRC"
echo "package main" >> "$COMBINED_SRC"
echo "use std.core.ptr.Ptr" >> "$COMBINED_SRC"
echo "" >> "$COMBINED_SRC"

MODULES=(
    "$PROJECT_ROOT/basalt/src/kernels.salt"
    "$PROJECT_ROOT/basalt/src/sampler.salt"
    "$PROJECT_ROOT/basalt/src/quant.salt"
    "$PROJECT_ROOT/basalt/src/transformer.salt"
    "$PROJECT_ROOT/basalt/src/model_loader.salt"
    "$PROJECT_ROOT/basalt/src/tokenizer.salt"
    "$PROJECT_ROOT/basalt/src/main.salt"
)

for file in "${MODULES[@]}"; do
    echo "// ---- Module: $(basename $file) ----" >> "$COMBINED_SRC"
    grep -v "^package " "$file" | \
    grep -v "^use basalt\." >> "$COMBINED_SRC"
    echo "" >> "$COMBINED_SRC"
done

echo "Built source: $COMBINED_SRC"

# 2. salt-front → MLIR
echo "Running salt-front..."
"$SALT_FRONT/target/release/salt-front" "$COMBINED_SRC" > "$OUT_DIR/basalt.mlir"

# 3. mlir-opt (optimization & lowering)
echo "Running mlir-opt..."
mlir-opt "$OUT_DIR/basalt.mlir" \
    --allow-unregistered-dialect \
    --canonicalize \
    --cse \
    --loop-invariant-code-motion \
    --sccp \
    --canonicalize \
    --cse \
    --lower-affine \
    --convert-scf-to-cf \
    --convert-vector-to-llvm \
    --convert-cf-to-llvm \
    --convert-arith-to-llvm \
    --convert-math-to-llvm \
    --convert-func-to-llvm \
    --reconcile-unrealized-casts \
    -o "$OUT_DIR/basalt.opt.mlir"

# 4. Strip verify ops
sed -i '' '/"salt.verify"/d' "$OUT_DIR/basalt.opt.mlir"

# 5. mlir-translate → LLVM IR
echo "Running mlir-translate..."
mlir-translate --mlir-to-llvmir "$OUT_DIR/basalt.opt.mlir" -o "$OUT_DIR/basalt.ll"

# 6. Compile to WASM objects
echo "Compiling to WASM..."
clang --target=wasm32-unknown-unknown -O3 -msimd128 -c "$OUT_DIR/basalt.ll" \
    -o "$OUT_DIR/basalt_engine.o" -Wno-override-module

clang --target=wasm32-unknown-unknown -O3 -msimd128 -fno-builtin -D__wasm__ -c \
    "$PROJECT_ROOT/basalt/wasm/basalt_wasm.c" \
    -o "$OUT_DIR/basalt_bridge.o"

# 7. Link WASM binary
echo "Linking WASM..."
wasm-ld --no-entry --export-dynamic \
    --import-memory \
    "$OUT_DIR/basalt_engine.o" \
    "$OUT_DIR/basalt_bridge.o" \
    -o "$OUT_DIR/basalt.wasm"

# 8. Copy to dist
DIST_DIR="$PROJECT_ROOT/basalt/wasm/dist"
mkdir -p "$DIST_DIR"
cp "$OUT_DIR/basalt.wasm" "$DIST_DIR/basalt.wasm"

SIZE=$(wc -c < "$DIST_DIR/basalt.wasm" | tr -d ' ')
echo "Build complete: $DIST_DIR/basalt.wasm (${SIZE} bytes)"
