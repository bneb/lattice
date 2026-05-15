# Possible Bugs and Security Vulnerabilities

This document is the result of a multi-phase, rigorous, line-by-line manual audit of the entire repository by dedicated sub-agents. It outlines deep logical bugs, severe security vulnerabilities, race conditions, and memory leaks that automated static analysis tools missed.

## 1. Salt Compiler (Soundness & Codegen)
- **[CRITICAL SECURITY] Unsound Precondition Verification in `@requires`**: `salt-front/src/codegen/verification/mod.rs`. The compiler elides runtime checks if Z3 returns `SAT`. This only proves a case *can* be satisfied, not that it is *always* satisfied. It should check for `UNSAT` on the negation of the requirement.
- **[CRITICAL SECURITY] Unchecked Pointer Indexing**: `salt-front/src/codegen/expr/memory.rs`. `Ptr<T>` indexing (`ptr[i]`) lowers to raw, unchecked MLIR `getelementptr` instructions. Z3 bounds verification exists in the source but is disconnected from the codegen path, enabling trivial buffer overflows.
- **[CRITICAL SECURITY] Generic Monomorphization Type Confusion**: `salt-front/src/codegen/generic_resolver.rs`. Unification logic does not validate consistency for subsequent encounters of the same generic parameter. This allows `fn swap<T>(a: T, b: T)` to be called with mismatched types (e.g., `Int` and `Ptr`), causing raw memory corruption during execution.

## 2. Kernel Assembly (Arch-Specific)
- **[CRITICAL SECURITY] GS-Base Race Condition in NMI Handler**: `kernel/arch/x86_64/nmi_handler.S`. If an NMI hits between `SYSCALL` and the kernel's first `swapgs`, the handler sees a kernel `CS` and skips its own `swapgs`. The kernel then uses the User GS base, leading to catastrophic information leaks or crashes.
- **[BUG] System V ABI Violation (Stack Misalignment)**: `kernel/arch/x86/syscall_entry.S` and `syscall_entry_fast.S`. The stack is misaligned by 8 bytes before calling C/Salt functions. This violates the 16-byte alignment requirement and can cause `#GP` faults on SSE instructions.
- **[SECURITY] Register State Leaks during Context Switch**: `kernel/arch/x86_64/context_switch_asm.S`. Scratch registers (RAX, RCX, RDX, R8-R11) are not cleared when switching between fibers or processes, allowing sensitive data to bleed across contexts.

## 3. Kernel Memory Management (Ring 0)
- **[CRITICAL SECURITY] Arbitrary Kernel Page Table Corruption**: `kernel/mem/user_paging.salt` (`map_user_page`). The function lacks a user-range check for `vaddr`. A process can map a physical frame over a kernel PDPT or PD, gaining arbitrary kernel read/write access.
- **[CRITICAL BUG] PMM Treiber Stack Corruption via Double Free**: `kernel/mem/pmm_sharded.salt` (`free_frame`). The PMM does not verify if a page is already free. Pushing the same physical address twice creates a cycle in the Treiber stack, leading to the same frame being allocated to multiple conflicting components.
- **[CRITICAL SECURITY] Arbitrary Kernel R/W via User-Controlled MOE_BAR_PTR**: `kernel/core/syscall.salt` (`syscall_set_moe_bar_ptr`). A syscall allows any user process to set the global `MOE_BAR_PTR` to an arbitrary 64-bit address without validation. During `sched_yield`, the kernel zeros out this address, enabling arbitrary kernel memory corruption.
- **[BUG] Total User Memory Leak on Process Exit**: `kernel/mem/user_paging.salt` (`destroy_user_pml4`). The function explicitly skips index 0. Because all user-mode ELFs and heaps exist in index 0, every process exit leaks its entire memory footprint and page tables.
- **[CRITICAL SECURITY] Out-of-Bounds Kernel R/W via Untrusted SPSC Capacity**: `kernel/lib/ipc_shm.salt`. Bulk operations read `capacity` from user-writable shared pages without bounds checking.
- **[CRITICAL SECURITY] Fastpath IPC OOB Array Access**: `kernel/ipc/fastpath.salt`. Unvalidated `cap_id` used as a direct index into global arrays.
- **[CRITICAL SECURITY] Division by Zero Kernel DoS**: `kernel/core/main.salt`. Computes modulo using a `capacity` value read from a user-shared ring buffer.

## 4. Userspace (Ring 3)
- **[CRITICAL BUG] SPSC Ring Buffer Overflow**: `user/lib/ring.salt`. Copy loops fail to handle wrap-around, causing OOB reads/writes at ring boundaries.
- **[BUG] GPU Command Buffer Overflow**: `user/browser/compositor.salt`. `trigger_hardware_compositor` writes to `GPU_RECT_BUF` without bounds checking.
- **[BUG] WebSocket Message Truncation**: `user/browser/main.salt`. Fixed-size buffer silently truncates large payloads.
- **[RACE CONDITION] Global State in NetD**: `user/netd/router.salt`. `bind_stream` lacks mutexes on global port arrays.

## 5. Basalt (Llama 2 Engine)
- **[CRITICAL LEAK] Per-Token Scratch Buffer Leak**: `basalt/src/sampler.salt`. `prob_buf` and `idx_buf` are never freed, leaking ~384MB per 1000 tokens.
- **[BUG] Top-P Truncation Disabled**: `basalt/src/sampler.salt`. A dead loop (`for skip in 0..0 {}`) prevents truncation.
- **[BUG] RoPE Rotation Corruption**: `basalt/src/model_loader.salt`. Identical real/imaginary pointers passed for RoPE rotation, breaking attention.
- **[LEAK] Engine Lifecycle RoPE Leak**: `basalt/src/main.salt`. `basalt_engine_free` fails to free frequency buffers.
