#!/usr/bin/env zsh
# =============================================================================
# test_nvme.sh — Build and Boot the NVMe Test Kernel on QEMU
# =============================================================================

set -euo pipefail

SCRIPT_DIR="${0:A:h}"
PROJECT_ROOT="${SCRIPT_DIR:h}"
SALT_FRONT="$PROJECT_ROOT/salt-front"

LLVM_VERSION="${LLVM_VERSION:-21}"
export PATH="/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin:$PATH"

BUILD_DIR="$PROJECT_ROOT/build_out"
TMP_DIR="/tmp/salt_build_nvme"
rm -rf "$TMP_DIR"
mkdir -p "$BUILD_DIR" "$TMP_DIR"

KERNEL_BIN="$BUILD_DIR/nvme_test.bin"
LINKER_SCRIPT="$PROJECT_ROOT/user/kernel/linker.ld"

echo "🔨 Building NVMe Test Kernel"
echo "============================================="

KERNEL_SALT_FILES=(
    "$PROJECT_ROOT/kernel/tests/nvme_test.salt"
    "$PROJECT_ROOT/kernel/keuos/hw/pcie_enum.salt"
    "$PROJECT_ROOT/kernel/drivers/serial.salt"
    "$PROJECT_ROOT/kernel/drivers/nvme.salt"
)

echo "  [1/5] Compiling Salt → MLIR..."
cd "$SALT_FRONT"
for SALT_FILE in "${KERNEL_SALT_FILES[@]}"; do
    BASENAME=$(basename "$SALT_FILE" .salt)
    
    LIB_FLAG=""
    if [[ "$BASENAME" != "nvme_test" ]]; then
        LIB_FLAG="--lib"
    fi

    "$PROJECT_ROOT/salt-front/target/release/saltc" "$SALT_FILE" $LIB_FLAG \
        -o "$TMP_DIR/${BASENAME}.mlir" 2>"$TMP_DIR/${BASENAME}_mlir.log" 2>/dev/null || {
        echo "  ✗ MLIR generation failed for $BASENAME"
        exit 1
    }
done
echo "  ✓ MLIR generated"

echo "  [2/5] Optimizing MLIR..."
for MLIR_FILE in "$TMP_DIR"/*.mlir; do
    if [[ "$MLIR_FILE" == *".opt."* ]]; then continue; fi
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

echo "  [3/5] Generating LLVM IR..."
for OPT_FILE in "$TMP_DIR"/*.opt.mlir; do
    BASENAME=$(basename "$OPT_FILE" .opt.mlir)
    mlir-translate --mlir-to-llvmir \
        "$OPT_FILE" > "$TMP_DIR/${BASENAME}.ll"
done
echo "  ✓ LLVM IR generated"

echo "  [4/5] Assembling startup..."
nasm -f elf64 \
    "$PROJECT_ROOT/kernel/tests/start_nvme.S" \
    -o "$TMP_DIR/start.o"
echo "  ✓ Assembly built"

# =============================================================================
# Step 5: Link the freestanding binary
# =============================================================================
echo "  [5/5] Linking freestanding kernel..."

# Compile LLVM IR to object files
for LL_FILE in "$TMP_DIR"/*.ll; do
    BASENAME=$(basename "$LL_FILE" .ll)
    clang -c -target x86_64-unknown-none-elf \
        -fno-stack-protector \
        -ffreestanding \
        -nostdlib \
        "$LL_FILE" -o "$TMP_DIR/${BASENAME}.o" 2>/dev/null
done

# Link with custom linker script
OBJ_FILES=()
if [[ -f "$TMP_DIR/start.o" ]]; then
    OBJ_FILES+=("$TMP_DIR/start.o")
fi
# Then add the rest
for OBJ in "$TMP_DIR"/*.o; do
    if [[ "$OBJ" == *"/start.o" ]]; then continue; fi
    OBJ_FILES+=("$OBJ")
done

ld.lld \
    -T "$LINKER_SCRIPT" \
    --no-dynamic-linker \
    -static \
    -nostdlib \
    -o "$KERNEL_BIN" \
    "${OBJ_FILES[@]}"

echo "  ✓ Kernel binary: $KERNEL_BIN"
echo ""

# Launch using run_qemu_nvme.sh
cd "$PROJECT_ROOT"
./scripts/run_qemu_nvme.sh "$KERNEL_BIN"
