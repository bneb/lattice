#!/usr/bin/env bash
# =============================================================================
# scripts/run_qemu_nvme.sh
# NVMe Storage — QEMU with Raw Disk
# =============================================================================

set -euo pipefail

KERNEL="${1:-build/keuos.bin}"
DISK="build/nvme_test.img"
DISK_SIZE="64M"

# Create NVMe disk if it doesn't exist
if [ ! -f "$DISK" ]; then
    echo "=== Creating raw NVMe disk: $DISK ($DISK_SIZE) ==="
    mkdir -p build
    dd if=/dev/zero of="$DISK" bs=1M count=64 status=progress
fi

# Verify kernel exists
if [ ! -f "$KERNEL" ]; then
    echo "ERROR: Kernel binary not found: $KERNEL"
    echo "Run './scripts/run_qemu.sh' to build first."
    exit 1
fi

echo "=== Booting KeuOS with NVMe ==="
echo "  Kernel: $KERNEL"
echo "  Disk:   $DISK"

qemu-system-x86_64 \
    -kernel "$KERNEL" \
    -M q35 \
    -nographic \
    -no-reboot \
    -serial mon:stdio \
    -drive file="$DISK",format=raw,if=none,id=nvme_drive \
    -device nvme,serial=1234,drive=nvme_drive \
    -m 256M \
    -trace "pci_nvme*"

echo "=== Exited ==="
