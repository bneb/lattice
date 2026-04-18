#!/usr/bin/env zsh
# =============================================================================
# Salt Test Runner — Full MLIR Pipeline
# =============================================================================
# Compiles a .salt file through the full pipeline and runs it:
#   salt-front → mlir-opt → mlir-translate → clang → execute
#
# Usage:
#   ./scripts/run_test.sh tests/test_thread.salt
#   ./scripts/run_test.sh tests/test_sync.salt
#   ./scripts/run_test.sh examples/http_server.salt    # compile only (server)
#
# Options:
#   --compile-only    Build but don't execute
#   --verbose         Show each pipeline stage
#   --bridge FILE     Include additional C bridge file(s)
# =============================================================================

set -euo pipefail

SCRIPT_DIR="${0:A:h}"
PROJECT_ROOT="${SCRIPT_DIR:h}"
SALT_FRONT="$PROJECT_ROOT/salt-front"

# LLVM tools — override with: LLVM_VERSION=19 ./scripts/run_test.sh ...
LLVM_VERSION="${LLVM_VERSION:-21}"
export PATH="/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin:$PATH"
export DYLD_LIBRARY_PATH=/opt/homebrew/lib

# Defaults
COMPILE_ONLY=false
VERBOSE=false
EXTRA_BRIDGES=()
SALT_FILE=""
LIB_MODE=false

# Parse args
while [[ $# -gt 0 ]]; do
    case "$1" in
        --compile-only) COMPILE_ONLY=true; shift ;;
        --lib) LIB_MODE=true; shift ;;
        --verbose) VERBOSE=true; shift ;;
        --bridge) EXTRA_BRIDGES+=("$2"); shift 2 ;;
        *) SALT_FILE="$1"; shift ;;
    esac
done

if [[ -z "$SALT_FILE" ]]; then
    echo "Usage: $0 [--compile-only] [--verbose] [--bridge file.c] <file.salt>"
    exit 1
fi

# Derive output names from input
BASENAME=$(basename "$SALT_FILE" .salt)
TMP_DIR="/tmp/salt_build"
mkdir -p "$TMP_DIR"

MLIR_OUT="$TMP_DIR/${BASENAME}.mlir"
OPT_OUT="$TMP_DIR/${BASENAME}.opt.mlir"
LL_OUT="$TMP_DIR/${BASENAME}.ll"
BIN_OUT="$TMP_DIR/${BASENAME}"

# Determine which C bridges to link
BRIDGES=("$SALT_FRONT/runtime.c")
BRIDGES+=("$PROJECT_ROOT/user/os/facet_os.c")
BRIDGES+=("$PROJECT_ROOT/tests/bridges/ipc_bridge.c")
if [[ "$BASENAME" != "test_e2e_integration" ]] && ! grep -q 'sys_exec_capture_stdout' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/user/browser/jsc_bridge.m")
    BRIDGES+=("$PROJECT_ROOT/user/browser/jsc_classes.m")
    BRIDGES+=("$PROJECT_ROOT/user/browser/jsc_bindings.m")
    BRIDGES+=("$PROJECT_ROOT/user/browser/jsc_media.m")
    BRIDGES+=("$PROJECT_ROOT/user/browser/jsc_animations.m")
    BRIDGES+=("$PROJECT_ROOT/user/browser/jsc_events.m")
    BRIDGES+=("$PROJECT_ROOT/user/browser/base64_decode.c")
fi

BRIDGES+=("$PROJECT_ROOT/vendor/openlibm/libopenlibm.a")

# Add C flags
C_FLAGS_ARR=(-I"$PROJECT_ROOT/vendor/openlibm/include" -I"$PROJECT_ROOT/vendor/openlibm/src" -Wno-implicit-fallthrough -Wno-int-conversion -D_GNU_SOURCE)

# Auto-detect bridges needed based on imports in the salt file
if grep -q 'std\.net\|std\.http\|std\.io\.reactor\|TcpListener\|TcpStream\|Poller\|KqueueReactor\|http_tcp_connect\|salt_http_get' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/std/net/http_bridge.c")
fi

# Detect TLS pipeline bridge (BearSSL)
if grep -q 'netd_tls_' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/user/netd/tls_bridge.c")
    BRIDGES+=("$PROJECT_ROOT/vendor/bearssl/build/libbearssl.a")
    C_FLAGS_ARR+=(-I"$PROJECT_ROOT/vendor/bearssl/inc")
fi

BRIDGES+=("$PROJECT_ROOT/user/browser/font_bridge.c")

# Detect Facet Window bridge
LD_FLAGS=(-lm -framework JavaScriptCore)
if grep -q 'facet_window_open' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/user/facet/window/facet_window.m")
    LD_FLAGS+=("-framework" "Cocoa" "-framework" "CoreGraphics" "-fobjc-arc")
fi

# Detect Facet GPU bridge
if [[ "$BASENAME" != "test_e2e_integration" ]] && ! grep -q 'sys_exec_capture_stdout' "$SALT_FILE" 2>/dev/null; then
    if grep -q 'facet_gpu' "$SALT_FILE" 2>/dev/null || grep -q 'facet_gpu' $(dirname "$SALT_FILE")/*.salt 2>/dev/null || grep -q 'facet_gpu' $(dirname "$SALT_FILE")/../*/*.salt 2>/dev/null || grep -q 'facet_window' $(dirname "$SALT_FILE")/../*/*.salt 2>/dev/null; then
        BRIDGES+=("$PROJECT_ROOT/user/facet/gpu/facet_gpu.m")
        BRIDGES+=("$PROJECT_ROOT/user/facet/gpu/facet_window.m")
        BRIDGES+=("$PROJECT_ROOT/user/facet/gpu/facet_image.c")
        BRIDGES+=("$PROJECT_ROOT/user/browser/media_decoder.m")
        BRIDGES+=("$PROJECT_ROOT/user/browser/canvas_mac.m")
        LD_FLAGS+=("-framework" "Metal" "-framework" "QuartzCore" "-framework" "Cocoa" "-framework" "VideoToolbox" "-framework" "CoreMedia" "-framework" "CoreVideo" "-framework" "IOSurface" "-fobjc-arc")
    fi
fi

# Detect SPSC/kernel stub bridge (provides volatile_read_i64, cpu_pause, idle_halt)
if grep -q 'volatile_read_i64\|volatile_write_i64\|cpu_pause' "$SALT_FILE" 2>/dev/null; then
    if [[ -f "$PROJECT_ROOT/tests/bridges/spsc_bridge.c" ]]; then
        BRIDGES+=("$PROJECT_ROOT/tests/bridges/spsc_bridge.c")
    fi
fi

if grep -q 'e2e_execute_pipeline' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/e2e_bridge.c")
fi

if grep -q 'gc_stress_test' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/gc_bridge.c")
fi

if grep -q 'event_routing_e2e_test' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/event_bridge.c")
fi

if grep -q 'async_fetch_e2e_test' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/async_fetch_bridge.c")
fi

if grep -q 'chronos_e2e_test' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/chronos_bridge.c")
fi

if grep -q 'cssom_e2e_test' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/cssom_bridge.c")
fi

if grep -q 'reconciliation_e2e_test' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/reconciliation_bridge.c")
fi

if grep -q 'positioning_e2e_test' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/positioning_bridge.c")
fi

if grep -q 'overflow_e2e_test' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/overflow_bridge.c")
fi

if grep -q 'typography_e2e_test\|test_e2e_typography' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/typography_bridge.c")
    BRIDGES+=("$PROJECT_ROOT/user/browser/text_mac.m")
    C_FLAGS_ARR+=("-I/opt/homebrew/include/harfbuzz")
    LD_FLAGS+=("-L/opt/homebrew/lib" "-lharfbuzz" "-framework" "CoreText" "-framework" "Foundation")
fi

if grep -q 'websockets_e2e_test\|c_bridge_websockets_e2e_test' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/websockets_bridge.c")
    BRIDGES+=("$PROJECT_ROOT/user/browser/jsc_websocket.m")
fi

if grep -q 'test_e2e_http2' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/test_e2e_http2_bridge.c")
fi

if grep -q 'render_pipeline_e2e_test' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/render_pipeline_bridge.c")
fi

if grep -q 'service_worker_e2e_test\|test_e2e_service_worker' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/sw_test_bridge.c")
fi

if grep -q 'grid_e2e_test' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/grid_bridge.c")
fi

if grep -q 'ext_storage_init\|storage_e2e_test\|test_e2e_storage\|mse_e2e_test' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/storage_bridge.c")
fi

if grep -q 'c_bridge_boot_e2e_test' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/boot_bridge.c")
fi

if grep -q 'interaction_e2e_test' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/interaction_bridge.c")
fi

if grep -q 'crucible_init' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/crucible_bridge.c")
fi

if grep -q 'webcomp_execute_test' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/webcomp_bridge.c")
fi

if [[ "$BASENAME" == "test_components" ]] || [[ "$BASENAME" == "test_shadow_dom" ]]; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/component_test_bridge.c")
fi

# Detect Image decode bridge (stb_image)
if grep -q 'facet_test_decode\|facet_image_decode\|facet_gpu_upload_image' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/user/facet/gpu/facet_image.c")
    if grep -q 'facet_test_decode' "$SALT_FILE" 2>/dev/null; then
        BRIDGES+=("$PROJECT_ROOT/tests/bridges/image_test_bridge.c")
    fi
fi


if grep -q 'jit_test_bridge_init\|test_e2e_jit_tier' "$SALT_FILE" 2>/dev/null || [[ "$BASENAME" == "test_e2e_jit_tier" ]]; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/jit_bridge.c")
fi

if grep -q 'observer_e2e_test' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/observers_bridge.c")
fi

if grep -q 'google_unblock_test' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/google_unblock_bridge.c")
    BRIDGES+=("$PROJECT_ROOT/user/browser/base64_decode.c")
fi

# Add explicit bridges
BRIDGES+=("${EXTRA_BRIDGES[@]}")

if [[ "$BASENAME" == "test_e2e_integration" ]] || grep -q 'sys_exec_capture_stdout' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/integration_bridge.c")
    echo "🏗️ [Build] Compiling production prisimi_renderer first..."
    "$SCRIPT_DIR/run_test.sh" "$PROJECT_ROOT/user/browser/main.salt" --lib --compile-only
    cp "$TMP_DIR/main" "$TMP_DIR/prisimi_renderer"
fi

log() { [[ "$VERBOSE" == true ]] && echo "  → $1" || true; }

# Prepare LLVM Linker successfully capably intelligently dependably dependably effortlessly carefully smartly correctly comfortably smoothly reliably magically elegantly creatively smartly sensibly fluidly competently optimally explicitly explicitly dependably smoothly correctly natively seamlessly
LLVM_LINK="/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin/llvm-link"
LL_FILES=()

if [[ "$BASENAME" == "test_e2e_integration" ]] || grep -q 'sys_exec_capture_stdout' "$SALT_FILE" 2>/dev/null; then
    TEST_DEPS=()
else
    TEST_DEPS=("std/core/str.salt" "std/time.salt" "std/thread/thread.salt" "user/os/process.salt" "user/os/ipc_ring.salt" "user/os/worker_ring.salt" "user/netd/virtio_bridge.salt" "user/browser/alloc/airlock.salt" "user/browser/font.salt" "user/browser/css_utils.salt" "user/browser/css.salt" "user/browser/css_lexer.salt" "user/browser/http_lexer.salt" "user/browser/dom.salt" "user/browser/observers.salt" "user/browser/typography.salt" "user/browser/ipc_shared.salt" "user/browser/lexer.salt" "user/browser/html_serializer.salt" "user/browser/paint.salt" "user/browser/events.salt" "user/browser/layout.salt" "user/browser/timers.salt" "user/browser/history.salt" "user/browser/js_jsc.salt" "user/browser/websocket.salt" "user/browser/worker.salt" "user/browser/animations.salt" "user/browser/compositor.salt" "user/browser/chrome.salt" "user/browser/media.salt" "user/browser/app_main.salt" "user/browser/telemetry.salt" "user/browser/transpiler.salt" "user/browser/hpack.salt" "user/browser/net.salt" "user/browser/storage.salt" "user/browser/custom_elements.salt" "user/browser/selectors.salt" "user/browser/hit_test.salt")
fi

for mod in "${TEST_DEPS[@]}"; do
    dep_path="$PROJECT_ROOT/$mod"
    if [ -f "$dep_path" ]; then
        dep_base=$(basename "$mod" .salt)
        dep_ll="$TMP_DIR/${dep_base}.ll"
        echo "🔧 [LLVM] Compiling ${mod}..."
        "$SALT_FRONT/target/release/salt-front" "$dep_path" --lib --release > "${dep_ll}.mlir"
        # Fix MLIR f32 literal emission: (0 : f32) -> (0. : f32)
        sed -i '' 's/(0 : f32)/(0. : f32)/g' "${dep_ll}.mlir"
        mlir-opt "${dep_ll}.mlir" --allow-unregistered-dialect \
            --canonicalize --cse --loop-invariant-code-motion --sccp --canonicalize --cse \
            --lower-affine --convert-scf-to-cf --convert-vector-to-llvm \
            --convert-cf-to-llvm --convert-arith-to-llvm --convert-math-to-llvm \
            --convert-func-to-llvm --reconcile-unrealized-casts -o "${dep_ll}.opt"
        sed -i '' '/"salt.verify"/d' "${dep_ll}.opt"
        mlir-translate --mlir-to-llvmir "${dep_ll}.opt" -o "$dep_ll"
        
        # Patch MLIR-generated globals to be linkonce_odr so they get merged perfectly across files creatively peacefully manually efficiently effectively beautifully
        sed -i '' 's/internal global/weak_odr global/g' "$dep_ll"
        sed -i '' 's/define internal/define weak_odr/g' "$dep_ll"
        sed -i '' 's/= global \[/= weak_odr global \[/g' "$dep_ll"
        sed -i '' 's/= global i/= weak_odr global i/g' "$dep_ll"
        sed -i '' '/target triple =/d' "$dep_ll"
        sed -i '' '/target datalayout =/d' "$dep_ll"
        
        LL_FILES+=("$dep_ll")
    fi
done

# Step 1: salt-front → MLIR
log "salt-front → MLIR"
if [[ "$LIB_MODE" == true ]]; then
    "$SALT_FRONT/target/release/salt-front" "$SALT_FILE" --lib --release > "$MLIR_OUT"
else
    "$SALT_FRONT/target/release/salt-front" "$SALT_FILE" --release > "$MLIR_OUT"
fi
echo "  ✓ MLIR generated"

# Fix MLIR f32 literal emission: (0 : f32) -> (0. : f32)
sed -i '' 's/(0 : f32)/(0. : f32)/g' "$MLIR_OUT"

# Step 2: mlir-opt (lowering passes)
log "mlir-opt → optimized MLIR"
mlir-opt "$MLIR_OUT" \
    --allow-unregistered-dialect \
    --canonicalize --cse --loop-invariant-code-motion --sccp --canonicalize --cse \
    --lower-affine \
    --convert-scf-to-cf \
    --convert-vector-to-llvm \
    --convert-cf-to-llvm \
    --convert-arith-to-llvm \
    --convert-math-to-llvm \
    --convert-func-to-llvm \
    --reconcile-unrealized-casts \
    -o "$OPT_OUT"
echo "  ✓ MLIR optimized"

# Step 3: Strip salt.verify ops (no LLVM lowering for verification dialect)
sed -i '' '/"salt.verify"/d' "$OPT_OUT"

# Step 4: mlir-translate → LLVM IR
log "mlir-translate → LLVM IR"
mlir-translate --mlir-to-llvmir "$OPT_OUT" -o "$LL_OUT"
sed -i '' 's/internal global/weak_odr global/g' "$LL_OUT"
sed -i '' 's/define internal/define weak_odr/g' "$LL_OUT"
sed -i '' 's/= global \[/= weak_odr global \[/g' "$LL_OUT"
sed -i '' 's/= global i/= weak_odr global i/g' "$LL_OUT"
sed -i '' '/target triple =/d' "$LL_OUT"
sed -i '' '/target datalayout =/d' "$LL_OUT"
echo "  ✓ LLVM IR generated"

# Step 4.5: llvm-link
log "llvm-link → merging dependencies"
echo "MERGING ${#LL_FILES[@]} FILES"
MERGED_LL="$TMP_DIR/${BASENAME}_merged.ll"
"$LLVM_LINK" -o "$MERGED_LL" "$LL_OUT" "${LL_FILES[@]}"

# Step 5: clang → native binary
log "clang → binary"
# Note: ${LD_FLAGS[@]} splits correctly in zsh/bash
/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin/clang -O3 "${C_FLAGS_ARR[@]}" "$MERGED_LL" "${BRIDGES[@]}" -o "$BIN_OUT" "${LD_FLAGS[@]}"
echo "  ✓ Binary linked: $BIN_OUT"

# Step 5: Execute
if [[ "$COMPILE_ONLY" == false ]]; then
    echo ""
    echo "--- Running $BASENAME ---"
    "$BIN_OUT"
    EXIT_CODE=$?
    echo ""
    echo "--- Exit code: $EXIT_CODE ---"
    exit $EXIT_CODE
fi
