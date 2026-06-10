# Phase 2 Proposal: Virtual File System (VFS) and Storage
Since Networking (VirtIO, UDP/TCP, NetD) and SMP (APIC, Chase-Lev Stealing) have been successfully brought up, Phase 2 will focus on the Storage layer.

## Objectives
1. **Virtual File System (VFS):** Implement a hierarchical VFS supporting `open()`, `read()`, `write()`, and `close()` multiplexed across mount points.
2. **NVMe Driver Integration:** Connect the existing `kernel/drivers/nvme.salt` driver to a Block Device abstraction layer.
3. **EXT2 / FAT32 Driver:** Implement a minimal file system driver to parse partitions and directory structures from the NVMe block device.
4. **Ring 3 Syscalls:** Plumb VFS operations through the syscall interface (`syscall_entry_fast.S`) so user-space applications can read and write files.
