#!/usr/bin/env zsh
set -e

echo "==========================================================="
echo "        THE SOVEREIGN MULTIVERSE (EPIC 68)                 "
echo "==========================================================="

rm -rf /tmp/salt_build
mkdir -p /tmp/salt_build

SCRIPT_DIR="${0:A:h}"
PROJECT_ROOT="${SCRIPT_DIR:h}"
SALT_FRONT="$PROJECT_ROOT/salt-front/target/release/salt-front"
declare -a MODULES=(
    "std/time.salt"
    "std/thread/thread.salt"
    "user/os/process.salt"
    "user/os/ipc_ring.salt"
    "user/os/worker_ring.salt"
    "user/netd/virtio_bridge.salt"
    "user/browser/alloc/airlock.salt"
    "user/browser/font.salt"
    "user/browser/css_utils.salt"
    "user/browser/css.salt"
    "user/browser/css_lexer.salt"
    "user/browser/http_lexer.salt"
    "user/browser/dom.salt"
    "user/browser/lexer.salt"
    "user/browser/html_serializer.salt"
    "user/browser/paint.salt"
    "user/browser/events.salt"
    "user/browser/layout.salt"
    "user/browser/timers.salt"
    "user/browser/history.salt"
    "user/browser/js_jsc.salt"
    "user/browser/websocket.salt"
    "user/browser/worker.salt"
    "user/browser/animations.salt"
    "user/browser/compositor.salt"
    "user/browser/chrome.salt"
    "user/browser/media.salt"
    "user/browser/observers.salt"
    "user/browser/typography.salt"
    "user/browser/app_main.salt"
    "user/browser/hit_test.salt"
    "user/browser/telemetry.salt"
    "user/browser/transpiler.salt"
    "user/browser/hpack.salt"
    "user/browser/net.salt"
    "user/browser/storage.salt"
    "user/browser/custom_elements.salt"
    "user/browser/selectors.salt"
    "user/browser/ipc_shared.salt"
    "tests/test_e2e_multiprocess.salt"
)

for mod in "${MODULES[@]}"; do
    if [ -f "$PROJECT_ROOT/$mod" ]; then
        dep_base=$(basename "$mod" .salt)
        echo "🔧 [LLVM] Compiling ${mod}..."
        (cd "$PROJECT_ROOT" && "$SALT_FRONT" -c --lib --release "${mod}" -o "/tmp/salt_build/${dep_base}.o")
    fi
done

echo "🔧 [LLVM] Compiling Engine Genesis (user/browser/main.salt)..."
"$SALT_FRONT" -c --lib --release user/browser/main.salt -o "/tmp/salt_build/main.o"

echo "🔧 [Clang] Assembling Renderer Binary..."
clang -O3 \
    /tmp/salt_build/*.o \
    tests/bridges/ipc_bridge.c \
    user/browser/jsc_bridge.m \
    user/browser/jsc_classes.m \
    user/browser/jsc_bindings.m \
    user/browser/jsc_media.m \
    user/browser/jsc_animations.m \
    user/browser/jsc_events.m \
    user/browser/jsc_websocket.m \
    user/browser/text_mac.m \
    vendor/openlibm/libopenlibm.a \
    std/net/http_bridge.c \
    user/browser/font_bridge.c \
    user/facet/gpu/facet_gpu.m \
    user/facet/gpu/facet_window.m \
    user/facet/gpu/facet_image.c \
    user/browser/canvas_mac.m \
    user/browser/media_decoder.m \
    tests/bridges/spsc_bridge.c \
    tests/bridges/main_bridge.c \
    user/browser/base64_decode.c \
    user/facet/window/facet_window.m \
    user/browser/metal.m \
    user/browser/tls_mac.m \
    tests/bridges/mac_stubs.c \
    salt-front/runtime.c \
    user/os/facet_os.c \
    -Ivendor/openlibm/include -Ivendor/openlibm/src -I/opt/homebrew/include/harfbuzz -DCONFIG_VERSION=\"2024-01-13\" -DCONFIG_BIGNUM -Wno-implicit-fallthrough -Wno-int-conversion -D_GNU_SOURCE \
    -framework Cocoa -framework Metal -framework MetalKit -framework IOSurface -framework QuartzCore -framework VideoToolbox -framework CoreMedia -framework CoreVideo -framework JavaScriptCore -framework CoreText -framework Foundation -framework Security -framework Network -L/opt/homebrew/lib -lharfbuzz -fobjc-arc -lm \
    -o /tmp/salt_build/prisimi_renderer

echo "  ✓ Renderer Built Natively: /tmp/salt_build/prisimi_renderer"

# Link Main Process
echo "🔧 [Clang] Assembling Cocoa Main Process Sandbox..."
clang -O3 \
    tests/bridges/ipc_bridge.c \
    user/browser/mac_app.m \
    -framework Cocoa -framework Metal -framework MetalKit -framework IOSurface -framework QuartzCore -fobjc-arc \
    -o /tmp/salt_build/mac_app

echo "  ✓ Main Process Built Natively: /tmp/salt_build/mac_app"

echo ""
echo "================= INITIATING MULTIPROCESS BOOT ================="

# Let's run the test directly through the renderer executable
/tmp/salt_build/prisimi_renderer -e _tests__test_e2e_multiprocess__tests_e2e_multiprocess_run

exit_code=$?

if [ $exit_code -eq 0 ]; then
    echo "--- Multiverse Validation TDD: PASS (Exit code: $exit_code) ---"
else
    echo "--- Multiverse Validation TDD: FAIL (Exit code: $exit_code) ---"
fi
exit $exit_code
