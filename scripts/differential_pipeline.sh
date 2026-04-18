#!/usr/bin/env zsh
set -euo pipefail

echo "🚀 Starting Differential Testing Pipeline..."

# 1. Build test binaries
echo "📦 Building differential test binaries..."
./scripts/build_lexer_diff.sh > /dev/null
./scripts/build_css_lexer_diff.sh > /dev/null
./scripts/build_http_lexer_diff.sh > /dev/null
./scripts/build_hpack_diff.sh > /dev/null
./scripts/build_js_lexer_diff.sh > /dev/null
./scripts/build_selectors_diff.sh > /dev/null
./scripts/build_layout_diff.sh > /dev/null

# 2. Build Reference Tools
if [[ ! -f ./tools/rust_css_parser/target/release/rust_css_parser ]]; then
    echo "📦 Building Rust CSS reference parser..."
    (cd tools/rust_css_parser && cargo build --release > /dev/null 2>&1)
fi
if [[ ! -f ./tools/rust_layout/target/release/rust_layout ]]; then
    echo "📦 Building Rust Layout reference (Taffy)..."
    (cd tools/rust_layout && cargo build --release > /dev/null 2>&1)
fi

# ============================================================================
# Test Suite Execution
# ============================================================================

echo ""
echo "🔍 HTML Lexer Differential Tests:"
for fixture in tests/fixtures/*.html; do
    python3 scripts/compare_html_lexer.py "$fixture"
done

echo ""
echo "🔍 CSS Lexer Differential Tests:"
for fixture in tests/fixtures/*.css; do
    python3 scripts/compare_css_lexer.py "$fixture"
done

echo ""
echo "🔍 HTTP Lexer Differential Tests:"
python3 scripts/compare_http_lexer.py tests/fixtures/http_identity.raw

echo ""
echo "🔍 HPACK Decoder Differential Tests:"
python3 scripts/compare_hpack.py tests/fixtures/hpack_indexed_basic.raw

echo ""
echo "🔍 JS Lexer Differential Tests:"
python3 scripts/compare_js_lexer.py tests/fixtures/lexer_basic.js

echo ""
echo "🔍 Selectors Engine Differential Tests:"
python3 scripts/compare_selectors.py "p" tests/fixtures/selectors_basic.html

echo ""
echo "🔍 Layout Solver Differential Tests:"
python3 scripts/compare_layout.py tests/fixtures/layout_basic.json

echo ""
echo "🔍 Structural Layout Evaluation (DOMA):"
./scripts/run_test.sh tests/test_e2e_render_pipeline.salt
python3 scripts/compare_geometry.py

echo ""
echo "✨ All differential tests PASSED!"
