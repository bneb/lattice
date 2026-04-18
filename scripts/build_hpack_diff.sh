#!/usr/bin/env zsh
set -euo pipefail

SCRIPT_DIR="${0:A:h}"
PROJECT_ROOT="${SCRIPT_DIR:h}"
SALT_FRONT="$PROJECT_ROOT/salt-front"
TMP_DIR="/tmp/salt_build_hpack_diff"
mkdir -p "$TMP_DIR"

LLVM_VERSION="${LLVM_VERSION:-21}"
export PATH="/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin:$PATH"

# HPACK Decoder Dependencies
DEPS=(
    "user/browser/hpack.salt"
    "tests/test_hpack_diff.salt"
)

MERGED_SALT="$TMP_DIR/hpack_diff_merged.salt"
echo "// Merged Salt file for hpack_diff" > "$MERGED_SALT"

for mod in "${DEPS[@]}"; do
    # Remove package/import
    # AND remove the extern declaration for the function we are defining in the test driver
    cat "$PROJECT_ROOT/$mod" | grep -v "^package " | grep -v "^import " | \
        grep -v "extern fn ext_net_route_header_to_stream" | \
        sed 's/airlock\.//g' | sed 's/c\.//g' | sed 's/dom\.//g' >> "$MERGED_SALT"
done

echo "🔧 [HPACK_Diff] Compiling merged Salt source..."
"$SALT_FRONT/target/release/salt-front" "$MERGED_SALT" --release > "$TMP_DIR/hpack_diff.mlir"

echo "🔧 [HPACK_Diff] Optimizing MLIR..."
mlir-opt "$TMP_DIR/hpack_diff.mlir" --allow-unregistered-dialect \
    --canonicalize --cse --lower-affine --convert-scf-to-cf --convert-vector-to-llvm \
    --convert-cf-to-llvm --convert-arith-to-llvm --convert-math-to-llvm \
    --convert-func-to-llvm --reconcile-unrealized-casts -o "$TMP_DIR/hpack_diff.opt"

echo "🔧 [HPACK_Diff] Translating to LLVM IR..."
mlir-translate --mlir-to-llvmir "$TMP_DIR/hpack_diff.opt" -o "$TMP_DIR/hpack_diff.ll"

echo "🔧 [HPACK_Diff] Linking test_hpack_diff..."
clang -O3 "$TMP_DIR/hpack_diff.ll" \
    "$SALT_FRONT/runtime.c" \
    "$PROJECT_ROOT/tests/bridges/hpack_diff_bridge.c" \
    -D_GNU_SOURCE -lm -o "./test_hpack_diff"

echo "✅ test_hpack_diff built successfully."
