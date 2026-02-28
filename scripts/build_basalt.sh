#!/bin/bash
set -euo pipefail

# =============================================================================
# Basalt Build Script
# =============================================================================
# Concatenates modules into a single unit and compiles via full MLIR pipeline.
# Mirrors logic from scripts/run_test.sh for toolchain setup.
# =============================================================================

# Setup Paths (LLVM 18)
export PATH="/opt/homebrew/opt/llvm@18/bin:$PATH"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SALT_FRONT="$PROJECT_ROOT/salt-front"

OUT_DIR="/tmp/salt_build"
mkdir -p "$OUT_DIR"
COMBINED_SRC="$OUT_DIR/basalt_combined.salt"
BASENAME="basalt"

MLIR_OUT="$OUT_DIR/${BASENAME}.mlir"
OPT_OUT="$OUT_DIR/${BASENAME}.opt.mlir"
LL_OUT="$OUT_DIR/${BASENAME}.ll"
BIN_OUT="$OUT_DIR/${BASENAME}"

# 1. Concatenate Source Files
echo "// Auto-generated build file for Basalt" > "$COMBINED_SRC"
echo "package main" >> "$COMBINED_SRC"
# Explicitly import std.core.ptr.Ptr so .offset() works globally
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
    # Strip package decls and local imports
    grep -v "^package " "$file" | \
    grep -v "^use basalt\." >> "$COMBINED_SRC"
    echo "" >> "$COMBINED_SRC"
done

echo "Built source: $COMBINED_SRC"

# 2. salt-front → MLIR
echo "Running salt-front..."
"$SALT_FRONT/target/release/salt-front" "$COMBINED_SRC" > "$MLIR_OUT"

# 3. mlir-opt (optimization & lowering)
echo "Running mlir-opt..."
mlir-opt "$MLIR_OUT" \
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
    -o "$OPT_OUT"

# 4. Strip verify ops
sed -i '' '/"salt.verify"/d' "$OPT_OUT"

# 5. mlir-translate → LLVM IR
echo "Running mlir-translate..."
mlir-translate --mlir-to-llvmir "$OPT_OUT" -o "$LL_OUT"

# 6. clang → native binary
echo "Running clang..."
# Link with runtime.c (for panic handler etc)
BRIDGES=("$SALT_FRONT/runtime.c")
clang -O3 -ffast-math -march=native "$LL_OUT" "${BRIDGES[@]}" -o "$BIN_OUT" -lm -Wno-override-module

echo "Build complete: $BIN_OUT"
echo "Running Basalt..."
echo "----------------------------------------------------------------"
"$BIN_OUT"
