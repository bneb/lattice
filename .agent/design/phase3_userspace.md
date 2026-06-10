# KeuOS Phase 3 Design: User Space Execution from EXT2

## Overview
Phase 3 establishes the true Ring-3 environment by bridging the Virtual File System (VFS) with the ELF Loader. Instead of loading kernel-embedded binaries, KeuOS will dynamically read ELF executables from the EXT2 root filesystem via the NVMe driver, allocate address spaces, map them, and jump to Ring 3.

## Architectural Components

### 1. VFS System Calls (`kernel/core/syscall.salt`)
- `sys_open(path)`: Traverses the VFS using the root inode's `lookup_op` to resolve a hierarchical path (e.g., `/bin/hello.elf`). It allocates an entry in the current process's `fd_table` and returns a file descriptor.
- `sys_read(fd, buf, count)`: Looks up the `FileDescriptor` in the `fd_table` and calls `inode.ops.read_op`, advancing the file offset.

### 2. EXT2 Directory Traversal (`kernel/sys/ext2.salt`)
- `ext2_lookup(dir_inode, name)`: Reads the data blocks of a directory inode sequentially.
- Parses `ext2_dir_entry_2` structures:
  - `inode_num` (u32)
  - `rec_len` (u16)
  - `name_len` (u8)
  - `file_type` (u8)
  - `name` (variable string)
- Reconstructs a memory-backed `Inode` struct when a string match is found, allowing further traversal or file reading.

### 3. Dynamic ELF Loading (`kernel/core/exec_user.salt`)
- **New API:** `spawn_process_from_inode(inode: Ptr<Inode>, kernel_pml4: u64) -> u64`
- **Workflow:**
  1. Calls `inode.ops.read_op` to fetch the 64-byte `Elf64Header` from offset 0 into a stack-allocated buffer.
  2. Validates the `\x7FELF` signature, 64-bit class, and OS ABI.
  3. Uses `vfs_read` to fetch the Program Headers (`e_phoff`).
  4. Iterates through `PT_LOAD` segments.
  5. For each segment, allocates physical frames via `pmm`, maps them into the user `PML4`, and uses `inode.ops.read_op` to stream data directly from the NVMe disk into the physical frames at `p_offset`.
  6. Zeroes out any remaining `BSS` memory (`p_memsz` - `p_filesz`).
  7. Sets up the kernel stack IRETQ frame and registers the process state.

### 4. Build System Enhancements (`tools/runner_qemu.py`)
- Automatically copies the compiled `.elf` programs (`hello.elf`, `test_memory.elf`, etc.) into the `qemu_build/ext2_disk.img` using `e2fsprogs` (specifically `debugfs` or similar).

## Performance & Security Constraints
- **Unassailable Safety:** `spawn_process_from_inode` must carefully bound checks on ELF segments. A malicious ELF segment size (`p_memsz`) could overflow into kernel space (`0xFFFF...`) or crash the PMM via exhaustion.
- **VFS Path Isolation:** Hardcode basic `strcmp` and `/` delimiter splitting. No `..` or `.` traversal supported in the MVP to prevent directory traversal exploits.
- **Zero-Copy Optimization:** For now, the block layer utilizes bounce buffers. True zero-copy DMA directly to the user pages (mapping the user physical frames directly into the NVMe PRP list) is deferred to Phase 4 for stability.
