# Security Audit Report: Memory Management Subsystem

**Date:** 2024-05-23
**Auditors:** Gemini CLI (Senior Security Researcher / OS Architect)
**Scope:** `kernel/mem/user_paging.salt`, `kernel/mem/pmm_sharded.salt`

---

## 1. Executive Summary
The audit identified critical vulnerabilities in the memory management subsystem. The most severe issues involve a lack of boundary checks in `map_user_page`, which allows modification of kernel page tables, and a double-free vulnerability in the process destruction path that corrupts the physical memory manager.

---

## 2. Findings: `kernel/mem/user_paging.salt`

### [CRITICAL] U-P-01: Double Free and Shared Memory Corruption
- **Line Numbers:** 324-334 (in `destroy_pt`)
- **Description:** The `destroy_user_pml4` function recursively traverses the page table and calls `pmm.free()` on every leaf physical frame. 
- **Impact:**
    1. **Double Free:** If a process maps the same physical page at two different virtual addresses, the page is freed twice, corrupting the PMM's free list.
    2. **Use-After-Free:** If two processes share memory (e.g., via a future SHM implementation or COW), destroying one process will free the pages still in use by the other.
- **Recommendation:** Implement a reference counting mechanism for physical frames or only free page table structures in `destroy_user_pml4`, leaving data page lifecycle management to a higher-level manager.

### [CRITICAL] U-P-02: Kernel Page Table Corruption via `map_user_page`
- **Line Numbers:** 218-225
- **Description:** `map_user_page` does not validate that the `vaddr` is within the user-mode range (typically `< 0x0000800000000000`).
- **Impact:** Since `create_user_pml4` clones the kernel half of the PML4 (indices 256-511), a call to `map_user_page` with a kernel address will cause `walk_or_create` to traverse the shared kernel PDPTs/PDs and overwrite kernel PTEs. Any kernel component or syscall that passes an unvalidated `vaddr` to this function can be exploited to gain arbitrary kernel read/write or code execution.
- **Recommendation:** Add a strict check: `if vaddr >= 0x0000800000000000 { panic("Security violation: attempt to map kernel address"); }`.

### [HIGH] U-P-03: Permission Promotion in Intermediate Tables
- **Line Numbers:** 204-214 (in `walk_or_create`)
- **Description:** `walk_or_create` unconditionally applies `USER_TABLE_FLAGS` (0x7: Present | Write | User) to all newly created PDP, PD, and PT entries.
- **Impact:** While leaf PTEs can still restrict access, having `Write` and `User` bits set at all higher levels increases the attack surface. If a leaf PTE is misconfigured or if a huge page is incorrectly mapped, the hardware-level protection is weakened.
- **Recommendation:** Intermediate table flags should be the logical OR of the required permissions for all child entries, or at least restricted based on the range being mapped.

### [LOW] U-P-04: Predictable PCID Allocation
- **Line Numbers:** 46-56
- **Description:** `allocate_pcid` uses a simple global counter that wraps at 4095.
- **Impact:** Predictable PCIDs can facilitate certain side-channel attacks (e.g., TLB-based timing attacks) between processes.
- **Recommendation:** Consider a randomized PCID allocator or a bitmap to track and reuse PCIDs less predictably.

---

## 3. Findings: `kernel/mem/pmm_sharded.salt`

### [CRITICAL] P-S-01: Treiber Stack Corruption via Double Free
- **Line Numbers:** 128-150 (in `free_frame`)
- **Description:** `free_frame` does not check if a page is already present in the free list. 
- **Impact:** Pushing the same physical address twice creates a cycle in the linked list (`A -> B -> A ...`). Subsequent calls to `alloc_frame` will return the same page multiple times to different kernel components, leading to catastrophic data corruption and unpredictable behavior.
- **Recommendation:** Implement a "Used/Free" bitmask or a metadata byte for each frame to track state before allowing a `free` operation.

### [HIGH] P-S-02: Lack of Bounds Checking on `free_frame`
- **Line Numbers:** 128-131
- **Description:** `free_frame` only checks for page alignment. It does not verify if the address is within the valid physical RAM range or if it is 0.
- **Impact:** `free_frame(0)` will result in `set_next_frame_ptr(0, head)`, which will likely corrupt the IDT or other low-memory structures via the `phys_to_virt` mapping.
- **Recommendation:** Validate that `page` is within the `start` and `end` bounds provided during `init()`.

### [MEDIUM] P-S-03: Potential ABA-like Race in `steal_from_remote`
- **Line Numbers:** 173-177
- **Description:** In `steal_from_remote`, `get_next_frame_ptr(victim_head)` is called before the `atomic_cas_i64`.
- **Impact:** Although mitigated by the per-core sharding (only the owner pushes), if the logic for where pages are freed ever changes (e.g., cross-core frees), this becomes a classic ABA vulnerability. The CAS only ensures the `head` is the same, not that the `next` pointer read previously is still valid.
- **Recommendation:** Ensure that the PMM design continues to strictly enforce that only the owner core can push to its own shard.

---

## 4. Conclusion
The memory management subsystem requires urgent refactoring to enforce address space boundaries and robustly manage physical frame lifecycles. The lack of `vaddr` validation in `map_user_page` is a particularly dangerous oversight that compromises the entire kernel's security model.
