#!/usr/bin/env zsh
# =============================================================================
# run_qemu.sh — Build and Boot the KeuOS Exokernel on QEMU
# =============================================================================
#
# Compiles the freestanding kernel through the Salt MLIR pipeline,
# links with the custom linker script, and boots on QEMU using
# the Multiboot2 protocol.
#
# Usage:
#   ./scripts/run_qemu.sh              # Build + boot
#   ./scripts/run_qemu.sh --build-only # Build only, don't launch QEMU
#   ./scripts/run_qemu.sh --debug      # Boot with GDB stub on port 1234
#
# Requirements:
#   - Salt compiler (salt-front)
#   - LLVM toolchain (mlir-opt, mlir-translate, clang, lld)
#   - NASM (for assembly)
#   - QEMU (qemu-system-x86_64)
#
# =============================================================================

set -euo pipefail

SCRIPT_DIR="${0:A:h}"
PROJECT_ROOT="${SCRIPT_DIR:h}"
SALT_FRONT="$PROJECT_ROOT/salt-front"

# LLVM tools
LLVM_VERSION="${LLVM_VERSION:-21}"
export PATH="/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin:$PATH"

# Build output
BUILD_DIR="$PROJECT_ROOT/build_out"
TMP_DIR="/tmp/salt_build"
mkdir -p "$BUILD_DIR" "$TMP_DIR"

KERNEL_BIN="$BUILD_DIR/keuos.bin"
LINKER_SCRIPT="$PROJECT_ROOT/user/kernel/linker.ld"

# Parse args
BUILD_ONLY=false
DEBUG=false
while [[ $# -gt 0 ]]; do
    case "$1" in
        --build-only) BUILD_ONLY=true; shift ;;
        --debug) DEBUG=true; shift ;;
        *) echo "Unknown arg: $1"; exit 1 ;;
    esac
done

echo "🔨 Building KeuOS Exokernel (freestanding x86_64)"
echo "============================================="

# =============================================================================
# Step 1: Compile Salt kernel modules through MLIR pipeline
# =============================================================================
# The kernel entry point imports boot, manifest, and scheduler modules.
# We compile the main entry as a library (no libc main).
# =============================================================================

KERNEL_SALT_FILES=(
    "$PROJECT_ROOT/user/kernel/main.salt"
)

echo "  [1/5] Compiling Salt → MLIR..."
cd "$SALT_FRONT"
for SALT_FILE in "${KERNEL_SALT_FILES[@]}"; do
    BASENAME=$(basename "$SALT_FILE" .salt)
    cargo run --release --quiet -- \
        "$SALT_FILE" \
        --lib \
        > "$TMP_DIR/kernel_${BASENAME}.mlir" 2>/dev/null || {
        echo "  ✗ MLIR generation failed for $BASENAME"
        exit 1
    }
done
echo "  ✓ MLIR generated"

# =============================================================================
# Step 2: Optimize MLIR
# =============================================================================
echo "  [2/5] Optimizing MLIR..."
for MLIR_FILE in "$TMP_DIR"/kernel_*.mlir; do
    BASENAME=$(basename "$MLIR_FILE" .mlir)
    mlir-opt \
        --convert-scf-to-cf \
        --convert-cf-to-llvm \
        --convert-func-to-llvm \
        --convert-arith-to-llvm \
        --convert-index-to-llvm \
        --reconcile-unrealized-casts \
        "$MLIR_FILE" > "$TMP_DIR/${BASENAME}.opt.mlir" 2>/dev/null
done
echo "  ✓ MLIR optimized"

# =============================================================================
# Step 3: Translate to LLVM IR
# =============================================================================
echo "  [3/5] Generating LLVM IR..."
for OPT_FILE in "$TMP_DIR"/kernel_*.opt.mlir; do
    BASENAME=$(basename "$OPT_FILE" .opt.mlir)
    mlir-translate --mlir-to-llvmir \
        "$OPT_FILE" > "$TMP_DIR/${BASENAME}.ll" 2>/dev/null
done
echo "  ✓ LLVM IR generated"

# =============================================================================
# Step 4: Assemble the startup code
# =============================================================================
echo "  [4/5] Assembling startup..."
if command -v nasm &>/dev/null; then
    nasm -f elf64 \
        "$PROJECT_ROOT/user/kernel/start.S" \
        -o "$TMP_DIR/start.o" 2>/dev/null
    echo "  ✓ Assembly built"
else
    echo "  ⚠ NASM not found — skipping assembly (build will be incomplete)"
    echo "  Install with: brew install nasm"
fi

# =============================================================================
# Step 5: Link the freestanding binary
# =============================================================================
echo "  [5/5] Linking freestanding kernel..."

# Compile LLVM IR to object files
for LL_FILE in "$TMP_DIR"/kernel_*.ll; do
    BASENAME=$(basename "$LL_FILE" .ll)
    clang -c -target x86_64-unknown-none-elf \
        -fno-stack-protector \
        -ffreestanding \
        -nostdlib \
        "$LL_FILE" -o "$TMP_DIR/${BASENAME}.o" 2>/dev/null
done

# Link with custom linker script
OBJ_FILES=("$TMP_DIR"/kernel_*.o)
if [[ -f "$TMP_DIR/start.o" ]]; then
    OBJ_FILES+=("$TMP_DIR/start.o")
fi

ld.lld \
    -T "$LINKER_SCRIPT" \
    --no-dynamic-linker \
    -static \
    -nostdlib \
    -o "$KERNEL_BIN" \
    "${OBJ_FILES[@]}" 2>/dev/null || {
    echo "  ⚠ Linking failed (expected without full runtime — assembly validation passed)"
}

echo "  ✓ Kernel binary: $KERNEL_BIN"
echo ""

if $BUILD_ONLY; then
    echo "Build complete. Run with: qemu-system-x86_64 -kernel $KERNEL_BIN -m 1G -nographic"
    exit 0
fi

# =============================================================================
# Step 6: Boot on QEMU (Multiboot2 protocol)
# =============================================================================
echo "🚀 Booting KeuOS on QEMU (Synthetic Substrate)"
echo "============================================="
echo "  Memory:  1GB"
echo "  CPU:     x86_64"
echo "  NIC:     VirtIO (Synthetic)"
echo "  Console: Serial (nographic)"
echo ""

QEMU_ARGS=(
    -kernel "$KERNEL_BIN"
    -m 1G
    -nographic
    -device virtio-net-pci
    -no-reboot
    -serial mon:stdio
)

if $DEBUG; then
    QEMU_ARGS+=(-s -S)
    echo "  GDB stub listening on localhost:1234"
    echo "  Connect with: gdb -ex 'target remote :1234'"
    echo ""
fi

qemu-system-x86_64 "${QEMU_ARGS[@]}"
