#!/usr/bin/env zsh
set -euo pipefail

SCRIPT_DIR="${0:A:h}"
PROJECT_ROOT="${SCRIPT_DIR:h}"
SALT_FRONT="$PROJECT_ROOT/salt-front"
TMP_DIR="/tmp/salt_build_worker"
mkdir -p "$TMP_DIR"

LLVM_VERSION="${LLVM_VERSION:-21}"
export PATH="/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin:$PATH"

# Headless Worker Dependencies
DEPS=(
    "std/core/str.salt"
    "std/time.salt"
    "user/os/process.salt"
    "user/browser/ipc_shared.salt"
    "user/browser/net.salt"
    "user/browser/worker_main.salt"
)

MERGED_SALT="$TMP_DIR/worker_merged.salt"
echo "// Merged Salt file for prisimi_worker" > "$MERGED_SALT"

for mod in "${DEPS[@]}"; do
    # Remove package and import lines, and also strip 'ipc_shared.' and 'net.' prefixes
    cat "$PROJECT_ROOT/$mod" | grep -v "^package " | grep -v "^import " | sed 's/ipc_shared\.//g' | sed 's/net\.//g' >> "$MERGED_SALT"
done

echo "🔧 [Worker] Compiling merged Salt source..."
"$SALT_FRONT/target/release/salt-front" "$MERGED_SALT" --release > "$TMP_DIR/worker_full.mlir"

mlir-opt "$TMP_DIR/worker_full.mlir" --allow-unregistered-dialect \
    --canonicalize --cse --lower-affine --convert-scf-to-cf --convert-vector-to-llvm \
    --convert-cf-to-llvm --convert-arith-to-llvm --convert-math-to-llvm \
    --convert-func-to-llvm --reconcile-unrealized-casts -o "$TMP_DIR/worker_full.opt"
mlir-translate --mlir-to-llvmir "$TMP_DIR/worker_full.opt" -o "$TMP_DIR/worker_merged.ll"

echo "🔧 [Worker] Linking prisimi_worker..."
# Link with Headless Worker Bridge (JavaScriptCore)
clang -O3 "$TMP_DIR/worker_merged.ll" \
    "$SALT_FRONT/runtime.c" \
    "$PROJECT_ROOT/user/os/facet_os.c" \
    "$PROJECT_ROOT/user/browser/jsc_sw_bridge.m" \
    "$PROJECT_ROOT/tests/bridges/ipc_bridge.c" \
    -DCONFIG_VERSION=\"2024-01-13\" -DCONFIG_BIGNUM -D_GNU_SOURCE \
    -lm -framework JavaScriptCore -fobjc-arc -o "./prisimi_worker"

echo "✅ prisimi_worker built successfully."
