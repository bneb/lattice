#!/usr/bin/env zsh
set -euo pipefail

SCRIPT_DIR="${0:A:h}"
PROJECT_ROOT="${SCRIPT_DIR:h}"
SALT_FRONT="$PROJECT_ROOT/salt-front"
TMP_DIR="/tmp/salt_build_selectors_diff"
mkdir -p "$TMP_DIR"

LLVM_VERSION="${LLVM_VERSION:-21}"
export PATH="/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin:$PATH"

# Selectors Dependencies
DEPS=(
    "user/browser/alloc/airlock.salt"
    "user/browser/dom.salt"
    "user/browser/selectors.salt"
    "tests/test_selectors_diff.salt"
)

MERGED_SALT="$TMP_DIR/selectors_diff_merged.salt"
echo "// Merged Salt file for selectors_diff" > "$MERGED_SALT"

for mod in "${DEPS[@]}"; do
    cat "$PROJECT_ROOT/$mod" | grep -v "^package " | grep -v "^import " | \
        sed 's/airlock\.//g' | sed 's/c\.//g' | sed 's/dom\.//g' >> "$MERGED_SALT"
done

echo "🔧 [Selectors_Diff] Compiling merged Salt source..."
"$SALT_FRONT/target/release/salt-front" "$MERGED_SALT" --release > "$TMP_DIR/selectors_diff.mlir"

echo "🔧 [Selectors_Diff] Optimizing MLIR..."
mlir-opt "$TMP_DIR/selectors_diff.mlir" --allow-unregistered-dialect \
    --canonicalize --cse --lower-affine --convert-scf-to-cf --convert-vector-to-llvm \
    --convert-cf-to-llvm --convert-arith-to-llvm --convert-math-to-llvm \
    --convert-func-to-llvm --reconcile-unrealized-casts -o "$TMP_DIR/selectors_diff.opt"

echo "🔧 [Selectors_Diff] Translating to LLVM IR..."
mlir-translate --mlir-to-llvmir "$TMP_DIR/selectors_diff.opt" -o "$TMP_DIR/selectors_diff.ll"

echo "🔧 [Selectors_Diff] Linking test_selectors_diff..."
clang -O3 "$TMP_DIR/selectors_diff.ll" \
    "$SALT_FRONT/runtime.c" \
    "$PROJECT_ROOT/tests/bridges/selectors_diff_bridge.c" \
    -D_GNU_SOURCE -lm -o "./test_selectors_diff"

echo "✅ test_selectors_diff built successfully."
