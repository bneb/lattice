#!/usr/bin/env zsh
set -euo pipefail

SCRIPT_DIR="${0:A:h}"
PROJECT_ROOT="${SCRIPT_DIR:h}"
SALT_FRONT="$PROJECT_ROOT/salt-front"
TMP_DIR="/tmp/salt_build_layout_diff"
mkdir -p "$TMP_DIR"

LLVM_VERSION="${LLVM_VERSION:-21}"
export PATH="/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin:$PATH"

# Layout Dependencies
DEPS=(
    "user/browser/dom.salt"
    "user/browser/font.salt"
    "user/browser/observers.salt"
    "user/browser/typography.salt"
    "user/browser/alloc/airlock.salt"
    "user/browser/layout.salt"
    "tests/test_layout_diff.salt"
)

MERGED_SALT="$TMP_DIR/layout_diff_merged.salt"
echo "// Merged Salt file for layout_diff" > "$MERGED_SALT"

for mod in "${DEPS[@]}"; do
    cat "$PROJECT_ROOT/$mod" | grep -v "^package " | grep -v "^import " | \
        sed 's/airlock\.//g' | sed 's/c\.//g' | sed 's/dom\.//g' | \
        sed 's/dom\.//g' | sed 's/typography\.//g' | \
        sed 's/font\.//g' | sed 's/observers\.//g' >> "$MERGED_SALT"
done

echo "🔧 [Layout_Diff] Compiling merged Salt source..."
"$SALT_FRONT/target/release/salt-front" "$MERGED_SALT" --release > "$TMP_DIR/layout_diff.mlir"

echo "🔧 [Layout_Diff] Optimizing MLIR..."
mlir-opt "$TMP_DIR/layout_diff.mlir" --allow-unregistered-dialect \
    --canonicalize --cse --lower-affine --convert-scf-to-cf --convert-vector-to-llvm \
    --convert-cf-to-llvm --convert-arith-to-llvm --convert-math-to-llvm \
    --convert-func-to-llvm --reconcile-unrealized-casts -o "$TMP_DIR/layout_diff.opt"

echo "🔧 [Layout_Diff] Translating to LLVM IR..."
mlir-translate --mlir-to-llvmir "$TMP_DIR/layout_diff.opt" -o "$TMP_DIR/layout_diff.ll"

echo "🔧 [Layout_Diff] Linking test_layout_diff..."
clang -O3 "$TMP_DIR/layout_diff.ll" \
    "$SALT_FRONT/runtime.c" \
    "$PROJECT_ROOT/tests/bridges/layout_diff_bridge.c" \
    -D_GNU_SOURCE -lm -o "./test_layout_diff"

echo "✅ test_layout_diff built successfully."
