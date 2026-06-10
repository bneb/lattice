# Phase 2: NVMe BlockDevice and EXT2 Integration Design

## 1. Objectives
- Implement the `BlockDevice` interface for the NVMe driver, enabling DMA block I/O.
- Implement a Read-Only EXT2 file system driver to parse partitions and directories from NVMe.
- Mount the EXT2 filesystem as the root (`/`) VFS mount.
- Enable user-space processes to read files from the physical disk via `sys_open` and `sys_read`.

## 2. NVMe BlockDevice Implementation

### 2.1. NVMe Request Queueing and DMA Safety
To avoid DMA boundary violations and Use-After-Free (UAF) vulnerabilities upon user process termination, we will use **Pinned Kernel Bounce Buffers**. 

```salt
pub struct NvmeRequest {
    pub req_id: u32,
    pub state: u32, // 0 = FREE, 1 = PENDING, 2 = COMPLETE
    pub bounce_phys: u64, // Pinned 4KB page in kernel PMM
    pub bounce_virt: Ptr<u8>,
}

// Global request table, statically allocated
// max 32 concurrent requests matching CQ size
```
*Red Team Fix:* The NVMe controller DMA will target `bounce_phys`. If the requesting fiber is killed, the kernel still safely owns the bounce buffer. The CQ polling loop will simply clear the request state.

### 2.2. BlockDevice Ops Mapping
We will implement `kernel/sys/nvme_block.salt` which maps `BlockDeviceOps` to the NVMe driver.

```salt
fn nvme_submit_request(req: Ptr<BlockRequest>) -> u32 {
    let req_id = allocate_nvme_req_slot_atomic(); // Must use atomic CAS for SMP safety
    
    // Copy user data to kernel bounce buffer (if write)
    if req.is_write {
        copy_to_bounce_buffer(req_id, req.virt_addr, req.count);
    }
    
    // Hit SQ doorbell
    submit_io_cmd(...)
    
    // Explicit Polling Loop (avoids Lost Wakeup race condition)
    while is_req_pending(req_id) {
        scheduler.yield_now(); 
    }
    
    // Copy from kernel bounce buffer to user data (if read)
    if not req.is_write {
        copy_from_bounce_buffer(req_id, req.virt_addr, req.count);
    }
    
    free_nvme_req_slot_atomic(req_id);
    return req_id;
}
```

## 3. EXT2 File System (Read-Only)

We will implement `kernel/sys/ext2.salt` to parse standard EXT2 structures.
To keep the implementation manageable and highly secure against malformed images, it will be strictly **Read-Only**.

### 3.1. EXT2 Structures
We define standard EXT2 data structures matching the on-disk layout.
*Red Team Fix:* All structs must be strictly validated. We will read fields using explicit byte-offset accessors (e.g., `read_u32(buf + 4)`) instead of relying on Salt struct layouts to guarantee there is no compiler padding misalignment.

### 3.2. Vulnerability Mitigations
- **Directory Entry Overflows:** We will strictly validate that `rec_len > 0` and `name_len <= rec_len` to prevent infinite loops and OOB reads during directory traversal.
- **Indirect Block Cycles:** We will enforce a strict maximum depth counter when traversing singly/doubly/triply indirect blocks to prevent infinite loops.
- **Integer Overflows:** We will use saturating math or explicit bounds checking on `offset + count` against `inode.size`.

## 4. Boot Integration
In `kernel.core.main.salt`:
1. Initialize the NVMe Controller.
2. Register the NVMe Block Device.
3. Call `ext2.mount(nvme_block_dev)` to read the superblock and instantiate the Root Inode.
4. Set the Root Inode in the VFS layer (`vfs.set_root(root_inode)`).

## 5. Security & Red Team Considerations
- **DMA Buffer Bounds:** When submitting DMA physical addresses, we must ensure the `buf_phys` is page-aligned and does not cross page boundaries unless using a PRP List (Page Replacement Page List). For MVP, we will limit single block requests to contiguous physical pages.
- **Async UAF:** Ensure that the fiber making the `BlockRequest` does not terminate or free its stack/buffer before the NVMe controller completes the DMA. The NVMe polling loop must handle abandoned requests safely.
- **EXT2 OOB Reads:** Bounds checking on indirect block reads. Maliciously crafted EXT2 images must not crash the kernel (validate block bounds against total blocks).
