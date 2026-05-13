# Possible Bugs and Issues - High-Fidelity Audit

This document lists critical logical, architectural, and security bugs identified during an exhaustive, line-by-line manual audit of the core codebase. These issues go beyond structural linting and represent actual threats to system stability and security.

## 1. Kernel Scheduler
- **[CRITICAL] Recursive Stack Overflow in `sched_yield`**: `kernel/core/scheduler.salt:505` calls `invoke_task` inline without switching to a dedicated scheduler stack or performing a proper context switch. Frequent yielding between fibers will lead to infinite kernel stack growth and a crash.
- **[BUG] Duplicate Syscall Numbers**: `kernel/core/syscall.salt:88` and `:94` both define syscall number 12. The second definition (`SYS_CORE_ACQUIRE`) is unreachable.

## 2. Memory Management
- **[CRITICAL] ABA Race Condition in PMM**: `kernel/core/pmm.salt:166-177` implements a lock-free Treiber stack for cross-core stealing without hazard pointers or generational counters. A core can read a `next` pointer from a page that is concurrently re-allocated and modified, leading to total free-list corruption.
- **[BUG] Lack of SMP Synchronization**: Globals such as `CURRENT_PID`, `NEXT_PID`, `PROC_TABLE`, and the `slab_cache` registry are accessed and modified across cores without atomics or spinlocks, leading to inevitable state corruption on multi-core systems.
- **[FRAGILE] Non-Contiguous Stack Allocation**: `kernel/core/process.salt:98` loops to allocate `KSTACK_PAGES` but assumes the resulting pages are physically contiguous when calculating `stack_top`. While currently `KSTACK_PAGES=1`, this is a latent bug that will trigger if stack size is increased.

## 3. Hardware Events & IPC
- **[CRITICAL] Non-Functional Event Queue**: `kernel/core/pulse.salt:72` has the `push` function disabled with a TODO. Consequently, ALL hardware interrupts (keyboard, timer, etc.) are dropped, rendering the kernel's event-driven logic dead.
- **[SECURITY] Unvalidated Pointer Dereference in `moe_drain_bar`**: `kernel/core/syscall.salt:392` dereferences a `payload_ptr` obtained from a BAR mailbox without validating it against `is_valid_user_ptr`. This allows a process with access to the mailbox to trigger arbitrary kernel memory reads.
- **[SECURITY] Insecure `sys_shm_grant`**: `kernel/core/syscall.salt:764` allows any process to inject memory mappings into any other process's address space without the target's consent or a handshake mechanism. This allows a malicious process to overwrite another's stack or data.
- **[SECURITY] Permission Escalation**: `sys_shm_grant` maps pages with `PTE_WRITE` (`kernel/core/syscall.salt:847`) regardless of the sender's original permissions, allowing a process to grant "Write" access to any page it can "Read".

## 4. Userspace / Basalt
- **[BUG] Segfault on OOM in Transformer**: `basalt/src/transformer.salt:117` returns a "null" `RunState` on allocation failure, but `forward` (line 144) dereferences its members without checking, causing an immediate crash on memory pressure.
- **[BUG] Memory Leak in `alloc_run_state`**: If only some buffers fail to allocate in `basalt/src/transformer.salt`, the successfully allocated ones are leaked before returning.
- **[BUG] Buffer Overflow in Tokenizer**: `basalt/src/tokenizer.salt:100` copies token text based on a length read from the file without verifying it stays within the pre-allocated `arena_size`. Corrupted files can cause a heap overflow.
- **[PERF] O(N) Token Lookup**: `basalt/src/tokenizer.salt:173` uses linear search for token IDs across 32,000+ entries, making tokenization extremely slow.

---
*Note: This list is in addition to the structural issues found in the previous automated scan.*
