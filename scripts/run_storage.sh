#!/usr/bin/env bash
# =============================================================================
# scripts/run_storage.sh
# VirtIO Block Storage — QEMU with Raw Tensor Disk
# =============================================================================
#
# Creates a 16MB raw disk image for tensor storage and boots
# the KeuOS exokernel with both VirtIO-net and VirtIO-blk.
#
# Usage:
#   ./scripts/run_storage.sh [kernel_path]
#
# =============================================================================

set -euo pipefail

KERNEL="${1:-build/keuos.bin}"
DISK="build/tensors.img"
DISK_SIZE="16M"

# Create tensor disk if it doesn't exist
if [ ! -f "$DISK" ]; then
    echo "=== Creating raw tensor disk: $DISK ($DISK_SIZE) ==="
    mkdir -p build
    dd if=/dev/zero of="$DISK" bs=1M count=16 status=progress
fi

# Verify kernel exists
if [ ! -f "$KERNEL" ]; then
    echo "ERROR: Kernel binary not found: $KERNEL"
    echo "Run './scripts/run_qemu.sh' to build first."
    exit 1
fi

echo "=== Booting KeuOS with VirtIO-blk ==="
echo "  Kernel: $KERNEL"
echo "  Disk:   $DISK"

qemu-system-x86_64 \
    -kernel "$KERNEL" \
    -nographic \
    -no-reboot \
    -serial mon:stdio \
    -netdev user,id=net0 \
    -device virtio-net-pci,netdev=net0,mac=52:54:00:12:34:AA \
    -drive file="$DISK",format=raw,if=virtio \
    -m 128M

echo "=== Exited ==="
