# Kernel Audit Report

This report contains the findings of a rigorous, file-by-file manual audit of the `kernel/` directory in the lattice monorepo. The audit identified multiple critical security vulnerabilities, deep race conditions, memory leaks, and architectural code smells that static analysis tools would miss.

## 1. Security Vulnerabilities

### 1.1 Arbitrary Kernel Read/Write via User-Controlled `MOE_BAR_PTR`
- **File:** `kernel/core/syscall.salt`
- **Location:** `syscall_set_moe_bar_ptr` (Line ~320), `moe_drain_bar` (Line ~355)
- **Description:** The system call `syscall_set_moe_bar_ptr` allows any user process to set the global `MOE_BAR_PTR` to an arbitrary 64-bit address without validation. During `sched_yield`, the kernel calls `moe_drain_bar`, which reads a 64-bit control word from this address and subsequently zeroes it out (`*((MOE_BAR_PTR) as &mut u64) = 0;`). A malicious user process can exploit this to perform arbitrary 8-byte kernel memory corruption (zeroing out page tables, capabilities, or function pointers), leading to a full system compromise.

### 1.2 Out-of-Bounds Kernel Read/Write via Untrusted SPSC Capacity
- **File:** `kernel/lib/ipc_shm.salt`
- **Location:** `spsc_push_bulk` (Line ~100), `spsc_pop_bulk` (Line ~136)
- **Description:** The bulk operations read the `capacity` field directly from the shared memory page, which is writable by user space. While `spsc_push` implements a defense-in-depth clamp (`idx >= DATA_CAPACITY`), the ERMS-accelerated bulk operations do not. If a malicious consumer sets `capacity` to `0xFFFFFFFFFFFFFFFF`, `seg1_len` underflows, allowing `fast_memcpy` to read or write arbitrary kernel memory beyond the bounds of the 4KB shared page.

### 1.3 Buffer Overflow / OOB Array Access in Fastpath IPC
- **File:** `kernel/ipc/fastpath.salt`
- **Location:** `fastpath_handoff_syscall` (Line ~19)
- **Description:** The syscall accepts `cap_id` directly from user space as a `u64` and uses it as an index into 64-element global arrays (`FASTPATH_PHYS_PTR`, `FASTPATH_LEN`, `FASTPATH_STATE`) without any bounds checking. A malicious user can provide a `cap_id >= 64` to overwrite kernel memory sequentially following these arrays, trivially hijacking kernel control flow.

### 1.4 Kernel DoS (Division by Zero) and OOB Read in Terminal SIP
- **File:** `kernel/core/main.salt`
- **Location:** `terminal_tx_poll_thread` (Line ~106)
- **Description:** The kernel thread polls the `KERNEL_TX_RING_VIRT` SPSC ring buffer, which is shared with the user-space Terminal SIP. It computes `let new_tail = (tail + 1) % capacity;` where `capacity` is read directly from the user-writable shared page. Setting `capacity = 0` from user space triggers a `#DE` (Divide Error) exception in Ring 0, immediately crashing the kernel. Furthermore, `tail` is also read unvalidated, allowing an out-of-bounds byte read: `let c = *((KERNEL_TX_RING_VIRT + 192 + tail) as &u8);`.

### 1.5 User Pages Mapped into Kernel Root Page Table
- **File:** `kernel/core/main.salt`
- **Location:** `kmain` (Line ~250)
- **Description:** When mapping the Framebuffer and the SPSC Ring Buffers for the Terminal SIP, `map_user_page_extern` is called with `kernel_pml4` and the `0x7` (PTE_USER) flag. This permanently injects user-accessible leaf entries into the kernel's root page table. This violates KPTI/SMAP boundaries, making these regions accessible while the CPU is executing within the kernel CR3 context, opening vectors for speculative execution attacks.

## 2. Race Conditions & Thread Safety

### 2.1 Non-Atomic Victim Bitmap Modification in Work Stealing
- **File:** `kernel/core/scheduler.salt`
- **Location:** `dispatch_stolen` (Line ~432)
- **Description:** When a core steals a fiber from a victim core, it updates the victim's scheduling bitmap directly: `SCHED_ARRAY[victim_cpu].l1_summary = SCHED_ARRAY[victim_cpu].l1_summary & victim_inv;`. This is a non-atomic read-modify-write across cache lines. If the victim core is concurrently spawning or exiting a fiber, this operation will clobber the victim's `l1_summary`, leading to lost fibers or phantom scheduling events.

### 2.2 Lack of Atomics in Slab Cache Allocator
- **File:** `kernel/mem/slab_cache.salt`
- **Location:** `alloc` (Line ~105), `free` (Line ~132)
- **Description:** The slab cache modifies the `free_list_head` in the `CACHE_REGISTRY` using standard memory loads and stores. Since `CACHE_REGISTRY` is global and cache IDs are shared across cores (e.g., the global `VMA_CACHE_ID`), concurrent allocations/deallocations from multiple cores will race on the Treiber stack head pointer, leading to corrupted linked lists and memory leaks.

### 2.3 Non-Atomic Global IPC Message Staging
- **File:** `kernel/core/syscall.salt` (calling `kernel/core/process.salt`)
- **Location:** `sys_ipc_send` -> `process.set_ipc_msg`
- **Description:** In `sys_ipc_send`, the message payload is written to the target process's PCB non-atomically. If multiple cores attempt to send an IPC message to the same `target_pid` concurrently, their writes to `ipc_msg0/1/2` and `ipc_sender` will interleave. This results in the target receiving a corrupted, frankenstein message payload composed of words from different senders.

### 2.4 Race Condition in PID Allocation
- **File:** `kernel/core/process.salt`
- **Location:** `alloc_pid` (Line ~76)
- **Description:** The function scans `PROC_TABLE[i].state` for `PROC_FREE` and then increments the global `NEXT_PID` and `PROC_COUNT` without atomics or spinlocks. Two cores executing `spawn_process` simultaneously can claim the same process slot and assign the exact same PID to two different address spaces, causing fatal PCB corruption.

## 3. Memory Leaks

### 3.1 Total User Memory Leak on Process Exit
- **File:** `kernel/mem/user_paging.salt`
- **Location:** `destroy_user_pml4` (Line ~285)
- **Description:** The loop responsible for freeing a dying process's page tables and physical memory deliberately starts at index 1: `let mut pml4_i: u64 = 1;`. The comment incorrectly states that `PML4[0]` is borrowed from the kernel identity map. In reality, `PML4[0]` manages the lower canonical half (`0x00000000` to `0x0000007FFFFFFFFF`), which is exactly where all user-mode ELFs, heaps, and stacks are mapped. By skipping index 0, every process exit leaks its entire user-space memory footprint, including the PDP, PD, and PT structures.

## 4. Architectural Code Smells

### 4.1 Missing Cross-Core TLB Shootdown on Guard Page Setup
- **File:** `kernel/core/vmm.salt`
- **Location:** `vmm_clear_present` (Line ~70)
- **Description:** When splitting a 2MB huge page to insert a 4KB guard page (e.g., for `stack_base`), the function only flushes the TLB on the local core (`switch_cr3` or `invlpg_stub`). Because the kernel direct map is shared across all cores, other SMP cores will retain the stale 2MB huge page in their TLBs. If a fiber on another core overflows its stack, it will not hit the guard page, resulting in silent memory corruption instead of a deterministic page fault.

### 4.2 Brittleness in Kernel Stack Contiguity Assumption
- **File:** `kernel/core/process.salt`
- **Location:** `alloc_kernel_stack` (Line ~95)
- **Description:** The kernel stack allocation relies on `pmm.alloc()`, which returns random, non-contiguous physical pages. The code currently assumes contiguity by computing `kernel_stack_top = base + KSTACK_SIZE`. This only works because `KSTACK_PAGES` is hardcoded to 1. If `KSTACK_PAGES` is ever increased, the stack top will point to an invalid or unmapped physical frame, breaking context switching instantly.
