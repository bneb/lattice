#!/usr/bin/env zsh
set -euo pipefail

SCRIPT_DIR="${0:A:h}"
PROJECT_ROOT="${SCRIPT_DIR:h}"
SALT_FRONT="$PROJECT_ROOT/salt-front"
TMP_DIR="/tmp/salt_build_http_lexer_diff"
mkdir -p "$TMP_DIR"

LLVM_VERSION="${LLVM_VERSION:-21}"
export PATH="/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin:$PATH"

# HTTP Lexer Dependencies
DEPS=(
    "user/browser/constants.salt"
    "user/browser/http_lexer.salt"
    "tests/test_http_lexer_diff.salt"
)

MERGED_SALT="$TMP_DIR/http_lexer_diff_merged.salt"
echo "// Merged Salt file for http_lexer_diff" > "$MERGED_SALT"

for mod in "${DEPS[@]}"; do
    cat "$PROJECT_ROOT/$mod" | grep -v "^package " | grep -v "^import " | \
        sed 's/c\.//g' | sed 's/dom\.//g' >> "$MERGED_SALT"
done

echo "🔧 [HTTP_LexerDiff] Compiling merged Salt source..."
"$SALT_FRONT/target/release/salt-front" "$MERGED_SALT" --release > "$TMP_DIR/http_lexer_diff.mlir"

echo "🔧 [HTTP_LexerDiff] Optimizing MLIR..."
mlir-opt "$TMP_DIR/http_lexer_diff.mlir" --allow-unregistered-dialect \
    --canonicalize --cse --lower-affine --convert-scf-to-cf --convert-vector-to-llvm \
    --convert-cf-to-llvm --convert-arith-to-llvm --convert-math-to-llvm \
    --convert-func-to-llvm --reconcile-unrealized-casts -o "$TMP_DIR/http_lexer_diff.opt"

echo "🔧 [HTTP_LexerDiff] Translating to LLVM IR..."
mlir-translate --mlir-to-llvmir "$TMP_DIR/http_lexer_diff.opt" -o "$TMP_DIR/http_lexer_diff.ll"

echo "🔧 [HTTP_LexerDiff] Linking test_http_lexer_diff..."
clang -O3 "$TMP_DIR/http_lexer_diff.ll" \
    "$SALT_FRONT/runtime.c" \
    "$PROJECT_ROOT/tests/bridges/http_lexer_diff_bridge.c" \
    -D_GNU_SOURCE -lm -o "./test_http_lexer_diff"

echo "✅ test_http_lexer_diff built successfully."
