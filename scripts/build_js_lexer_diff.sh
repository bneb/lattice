#!/usr/bin/env zsh
set -euo pipefail

SCRIPT_DIR="${0:A:h}"
PROJECT_ROOT="${SCRIPT_DIR:h}"
SALT_FRONT="$PROJECT_ROOT/salt-front"
TMP_DIR="/tmp/salt_build_js_lexer_diff"
mkdir -p "$TMP_DIR"

LLVM_VERSION="${LLVM_VERSION:-21}"
export PATH="/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin:$PATH"

# JS Lexer Dependencies
DEPS=(
    "user/browser/js_tokens.salt"
    "user/browser/js_lexer.salt"
    "tests/test_js_lexer_diff.salt"
)

MERGED_SALT="$TMP_DIR/js_lexer_diff_merged.salt"
echo "// Merged Salt file for js_lexer_diff" > "$MERGED_SALT"

for mod in "${DEPS[@]}"; do
    # Remove package, import, AND use
    cat "$PROJECT_ROOT/$mod" | grep -v "^package " | grep -v "^import " | grep -v "^use " | \
        sed 's/c\.//g' | sed 's/dom\.//g' >> "$MERGED_SALT"
done

echo "🔧 [JS_LexerDiff] Compiling merged Salt source..."
"$SALT_FRONT/target/release/salt-front" "$MERGED_SALT" --release > "$TMP_DIR/js_lexer_diff.mlir"

echo "🔧 [JS_LexerDiff] Optimizing MLIR..."
mlir-opt "$TMP_DIR/js_lexer_diff.mlir" --allow-unregistered-dialect \
    --canonicalize --cse --lower-affine --convert-scf-to-cf --convert-vector-to-llvm \
    --convert-cf-to-llvm --convert-arith-to-llvm --convert-math-to-llvm \
    --convert-func-to-llvm --reconcile-unrealized-casts -o "$TMP_DIR/js_lexer_diff.opt"

echo "🔧 [JS_LexerDiff] Translating to LLVM IR..."
mlir-translate --mlir-to-llvmir "$TMP_DIR/js_lexer_diff.opt" -o "$TMP_DIR/js_lexer_diff.ll"

echo "🔧 [JS_LexerDiff] Linking test_js_lexer_diff..."
clang -O3 "$TMP_DIR/js_lexer_diff.ll" \
    "$SALT_FRONT/runtime.c" \
    "$PROJECT_ROOT/tests/bridges/js_lexer_diff_bridge.c" \
    -D_GNU_SOURCE -lm -o "./test_js_lexer_diff"

echo "✅ test_js_lexer_diff built successfully."
