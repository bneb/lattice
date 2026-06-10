# Phase 2: Virtual File System (VFS) and Block Storage Design

## 1. Objectives
- Introduce a high-performance **Virtual File System (VFS)** for KeuOS.
- Create a generic **BlockDevice** interface supporting async/DMA.
- Connect the existing NVMe driver (`nvme.salt`) to the BlockDevice layer.
- Plumb file descriptors (`fd`) through to user-space via syscalls (`sys_open`, `sys_read`, `sys_write`, `sys_close`, `sys_lseek`, `sys_fstat`).

## 2. Architecture

### 2.1. The Block Device Layer (`kernel/sys/block.salt`)
To support asynchronous DMA, the block device interface will use physical memory boundaries and yield fibers.
```salt
pub struct BlockRequest {
    pub id: u32,
    pub is_write: bool,
    pub lba: u64,
    pub count: u32,
    pub phys_addr: u64, // DMA target
    pub fiber_waker: u64, // Scheduler task ID to wake
}

pub struct BlockDeviceOps {
    pub submit_request: fn(req: Ptr<BlockRequest>) -> u32, // Returns req_id
}
```

### 2.2. The VFS Layer (`kernel/sys/vfs.salt`)
Inodes will use static operation tables (`InodeOps`) to preserve cache locality, and atomic sizes for concurrency.
```salt
pub struct InodeOps {
    pub read: fn(inode: Ptr<Inode>, offset: u64, buf: Ptr<u8>, count: u64) -> i64,
    pub write: fn(inode: Ptr<Inode>, offset: u64, buf: Ptr<u8>, count: u64) -> i64,
    pub lookup: fn(inode: Ptr<Inode>, name: Ptr<u8>) -> Ptr<Inode>,
    pub mkdir: fn(inode: Ptr<Inode>, name: Ptr<u8>) -> i32,
}

pub struct Inode {
    pub id: u64,
    pub size: u64, // Needs atomic updates
    pub is_dir: bool,
    pub ops: Ptr<InodeOps>,
    pub refcount: u32, // Atomic
}

pub struct FileDescriptor {
    pub inode: Ptr<Inode>, // Reference counted
    pub offset: u64, // Must be protected by a lock for multithreaded read/write
    pub flags: u32,
    pub refcount: u32, // UAF protection
}
```

### 2.3. RamFS (Initial File System) (`kernel/sys/ramfs.salt`)
We will implement RamFS to prove the VFS and syscalls.

### 2.4. Syscall Integration (`kernel/core/syscall.salt`)
- `sys_open(path: Ptr<u8>, flags: u32)` -> `fd: i32`. Must use `copy_from_user` for `path`.
- `sys_read(fd: i32, buf: Ptr<u8>, count: u64)` -> `bytes: i64`
- `sys_write(fd: i32, buf: Ptr<u8>, count: u64)` -> `bytes: i64`
- `sys_close(fd: i32)` -> `status: i32`
- `sys_lseek(fd: i32, offset: i64, whence: i32)` -> `offset: i64`
- `sys_fstat(fd: i32, stat_buf: Ptr<u8>)` -> `status: i32`

## 3. Implementation Steps
1. **Define VFS & Block Structs**: Add `kernel/sys/vfs.salt` and `kernel/sys/block.salt`.
2. **Implement RamFS**: Add `kernel/sys/ramfs.salt` to support basic in-memory files.
3. **Process FD Table**: Modify `Process` in `kernel/core/process.salt` to hold `Ptr<FileDescriptor>`.
4. **Syscalls**: Implement `sys_open`, `sys_read`, `sys_write`, `sys_close` using `copy_from_user` security patterns.
5. **Testing**: Write a TDD test `kernel/sys/vfs_test.salt`.
