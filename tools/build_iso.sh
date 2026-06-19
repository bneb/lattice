#!/bin/bash
set -e

# Build the kernel ELF locally
echo "==> Building KeuOS..."
# python3 tools/runner_qemu.py build

# Setup the ISO directory structure
echo "==> Setting up ISO directory..."
rm -rf isodir
mkdir -p isodir/boot/grub

# Copy the kernel
cp qemu_build/kernel.elf isodir/boot/keuos.elf

# Create the GRUB configuration
cat > isodir/boot/grub/grub.cfg << EOF
set timeout=0
set default=0
menuentry "KeuOS" {
    multiboot /boot/keuos.elf
    boot
}
EOF

# Build the ISO using Docker to avoid local dependency issues on macOS
echo "==> Building keuos.iso using Docker..."
docker run --rm --platform linux/amd64 -v "$PWD:/work" ubuntu:22.04 bash -c "
    apt-get update -qq &&
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq grub-pc-bin grub-efi-amd64-bin xorriso mtools > /dev/null &&
    grub-mkrescue -o /work/keuos.iso /work/isodir
"

echo "==> Success! Bootable USB image created at keuos.iso"
echo "You can flash this to a USB drive using BalenaEtcher, Rufus, or dd:"
echo "    sudo dd if=keuos.iso of=/dev/diskX bs=4m status=progress"
