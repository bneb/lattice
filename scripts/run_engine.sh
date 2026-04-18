#!/usr/bin/env zsh
set -e

echo "==========================================================="
echo "        THE PRISIMI BROWSER ENGINE (EPIC 7)                "
echo "==========================================================="

rm -rf /tmp/salt_build
mkdir -p /tmp/salt_build

# 1. Compile engine dependencies natively
export PATH="/opt/homebrew/opt/llvm@21/bin:/opt/homebrew/bin:$PATH"

declare -a MODULES=()
for mod in std/time.salt std/thread/thread.salt user/os/*.salt user/netd/virtio_bridge.salt user/browser/alloc/airlock.salt user/browser/*.salt; do
    if [[ "$mod" == "user/browser/main.salt" ]] || [[ "$mod" == *"cdm_main"* ]] || [[ "$mod" == *"worker_main"* ]]; then continue; fi
    if [[ "$mod" == *"js_quickjs.salt"* ]]; then continue; fi
    MODULES+=("$mod")
done

SCRIPT_DIR="${0:A:h}"
PROJECT_ROOT="${SCRIPT_DIR:h}"
SALT_FRONT="$PROJECT_ROOT/salt-front/target/release/salt-front"

for mod in "${MODULES[@]}"; do
    echo "🔧 [LLVM] Compiling $mod..."
    fname=$(basename "$mod" .salt)
    "$SALT_FRONT" --lib "$mod" > "/tmp/salt_build/$fname.mlir"
    mlir-opt "/tmp/salt_build/$fname.mlir" --allow-unregistered-dialect \
        --canonicalize --cse --lower-affine --convert-scf-to-cf --convert-vector-to-llvm \
        --convert-cf-to-llvm --convert-arith-to-llvm --convert-math-to-llvm \
        --convert-func-to-llvm --reconcile-unrealized-casts -o "/tmp/salt_build/$fname.opt"
    mlir-translate --mlir-to-llvmir "/tmp/salt_build/$fname.opt" -o "/tmp/salt_build/$fname.ll"
done

echo "🔧 [LLVM] Compiling Engine Genesis (user/browser/main.salt)..."
"$SALT_FRONT" --lib user/browser/main.salt > "/tmp/salt_build/main.mlir"
mlir-opt "/tmp/salt_build/main.mlir" --allow-unregistered-dialect \
    --canonicalize --cse --lower-affine --convert-scf-to-cf --convert-vector-to-llvm \
    --convert-cf-to-llvm --convert-arith-to-llvm --convert-math-to-llvm \
    --convert-func-to-llvm --reconcile-unrealized-casts -o "/tmp/salt_build/main.opt"
mlir-translate --mlir-to-llvmir "/tmp/salt_build/main.opt" -o "/tmp/salt_build/main.ll"

# 2. Patch Linkonce ODR across ALL files to prevent identical SoA arrays from colliding
# MacOS static linking crashes on identically named Struct-of-Arrays buffers.
echo "🔧 [LLVM-Link] Merging SoA Matrix dependencies cleanly..."
for f in /tmp/salt_build/*.ll; do
    sed -i '' 's/internal global /linkonce_odr global /g' "$f"
    sed -i '' 's/private global /linkonce_odr global /g' "$f"
done

# 3. Native Merging 
MERGED_LL="/tmp/salt_build/engine_merged.ll"
llvm-link -S /tmp/salt_build/*.ll -o "$MERGED_LL"

# 4. Compile Merged Engine Native
MERGED_OBJ="/tmp/salt_build/engine_merged.o"
clang -O3 -c "$MERGED_LL" -o "$MERGED_OBJ"

# Enable debugging of the symbol table if the linker fails natively
# nm -m "$MERGED_OBJ" > /tmp/salt_build/engine_symbols.txt

# 5. Link with runtime and bridges
echo "🔧 [Clang] Assembling Execute Artifact..."
clang -O3 \
    "$MERGED_OBJ" \
    tests/bridges/spsc_bridge.c \
    tests/bridges/ipc_bridge.c \
    tests/bridges/mac_stubs.c \
    salt-front/runtime.c \
    user/browser/text_mac.m \
    user/browser/tls_mac.m \
    user/browser/canvas_mac.m \
    user/browser/media_decoder.m \
    user/browser/font_bridge.c \
    user/browser/cdm_bridge.c \
    user/browser/jsc_bridge.m \
    user/browser/jsc_bindings.m \
    user/browser/jsc_classes.m \
    user/browser/jsc_events.m \
    user/browser/jsc_media.m \
    user/browser/jsc_websocket.m \
    user/browser/jsc_animations.m \
    user/browser/metal.m \
    user/os/facet_os.c \
    user/facet/gpu/facet_window.m \
    user/facet/gpu/facet_gpu.m \
    user/facet/gpu/facet_image.c \
    -I/opt/homebrew/include/harfbuzz \
    -L/opt/homebrew/lib -lharfbuzz \
    -framework Cocoa -framework Metal -framework MetalKit -framework IOKit -framework Security -framework CoreMedia -framework VideoToolbox -framework CoreVideo -framework Network -framework IOSurface -framework JavaScriptCore -framework QuartzCore \
    -o /tmp/salt_build/prisimi_engine \
    -e _salt_browser_main

echo "  ✓ Prisimi Sovereign Built Natively: /tmp/salt_build/prisimi_engine"

echo ""
echo "================= INITIATING ENGINE HEARTBEAT ================="
/tmp/salt_build/prisimi_engine

exit_code=$?
echo ""
echo "--- Engine Halt (Exit code: $exit_code) ---"
