#!/bin/bash
set -e
mkdir -p qemu_build
IMG="qemu_build/ext2_disk.img"
echo "Creating raw NVMe disk: $IMG (64M)"
dd if=/dev/zero of=$IMG bs=1M count=64 status=progress
echo "Formatting as ext2"
mkfs.ext2 -F $IMG

echo "Mounting and copying ELFs"
mkdir -p /tmp/keuos_mnt
# On macOS we might need ext4fuse or something, wait! I'm on mac!
