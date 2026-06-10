# KeuOS & Salt - Hardened Sprint Plan

*This sprint plan has undergone a rigorous Red Team analysis. We are no longer simply "fixing bugs"; we are closing critical exploitation vectors and hardening the architecture against adversarial input.*

## Sprint 1: Formal Verification & Soundness
**Goal:** Prove the absence of vulnerabilities in the kernel boundaries using Z3 constraint solving.
- [x] **SEC-01:** Fix Z3 `SAT` vs `UNSAT` inversion in `@requires`.
- [x] **SEC-02:** Integrate `ptr_bounds_verifier.rs` into `emit_index` and `emit_lvalue` for mandatory bounds checking.
- [x] **SEC-03:** Fix generic unification type confusion in `generic_resolver.rs`.
- [x] **SEC-04:** Establish a "Negative Compilation" test suite. Prove that out-of-bounds indices and type confusion now physically fail to compile.

## Sprint 2: Kernel Privilege Escalation & Memory Corruption
**Goal:** Seal the Ring 0 / Ring 3 boundary. User processes must not dictate kernel state.
- [x] **SEC-05:** Remove `syscall_set_moe_bar_ptr`. The kernel must NEVER allow a user process to define arbitrary 64-bit physical memory targets.
- [x] **SEC-06:** Fix `map_user_page` in `user_paging.salt` to violently `panic()` if `vaddr >= 0x0000800000000000`. Prevent user manipulation of the kernel PML4 half.
- [x] **SEC-07:** Restrict `USER_TABLE_FLAGS` in intermediate tables (PDP, PD, PT) so that `Write` and `User` bits are only granted if explicitly required by the leaf mapping.
- [x] **SEC-08:** Mitigate Fastpath IPC Array OOB: Add strict `if cap_id >= 64 { return ERR; }` bounds checks before accessing `FASTPATH_PHYS_PTR`.

## Sprint 3: The Untrusted SPSC Threat Vector
**Goal:** Assume the data plane is actively trying to compromise the system.
- [x] **SEC-09:** Clamp `capacity` reads in `spsc_push_bulk`/`spsc_pop_bulk` (`ipc_shm.salt`). Never trust the capacity field written by the consumer/producer.
- [x] **SEC-10:** Fix `terminal_tx_poll_thread` (#DE) division-by-zero crash by validating that the shared ring `capacity > 0`.
- [x] **SEC-11:** Redesign `ipc_ring_push_bytes` to handle partial write failure correctly instead of dropping bytes mid-frame.
- [x] **SEC-12:** Fix SPSC wrap-around linear copy logic in `user/lib/ring.salt`. Split operations into two distinct memory segments if crossing the physical boundary.

## Sprint 4: PMM Corruption & Resource Exhaustion (DoS)
**Goal:** Eradicate leaks and data races that allow trivial Denial of Service.
- [x] **SEC-13:** Fix the catastrophic double-free in `destroy_user_pml4`. The loop MUST start at `pml4_i = 0` (or correctly handle the lower canonical half) to prevent total user memory leak.
- [x] **SEC-14:** Implement a Used/Free bitmask in `pmm_sharded.salt` to detect and prevent Treiber stack cycle corruption (Double Free).
- [x] **SEC-15:** Validate that the address passed to `free_frame` is within physical RAM bounds and non-zero.
- [x] **SEC-16:** Overhaul FD generation in `user/lib/socket.salt` to prevent predictable `port % 256` FD guessing attacks.

## Sprint 5: Concurrency, SMP & Basalt Hardening
**Goal:** Eliminate race conditions and secure the AI inference workload.
- [x] **SEC-17:** Replace non-atomic reads/writes in `dispatch_stolen` (Scheduler) and Slab Cache Allocator with proper hardware CAS instructions.
- [x] **SEC-18:** Implement cross-core TLB shootdowns in `vmm_clear_present` to guarantee guard page effectiveness across all CPUs.
- [x] **SEC-19:** Patch Basalt Memory Leaks: Free `prob_buf` and `idx_buf` in `sample_topp`, and `freq_cis` in `basalt_engine_free`.

## Sprint 6: The TUI & Shell Milestone
**Goal:** Boot KeuOS on a standard Linux hypervisor and drop into an interactive REPL.
- [x] **TUI-01:** Implement VirtIO console/serial RX interrupt handling for keyboard input.
- [x] **TUI-02:** Establish the `grit` shell architecture (event loop reading from VirtIO rings).
- [x] **TUI-03:** Implement basic `stdio` standard library wrappers to write to the VGA text buffer or serial out.
- [x] **TUI-04:** Achieve a successful interactive boot sequence ("Hello, KeuOS").

## Sprint 7: Technical Debt & Phase Transition
**Goal:** Address deferred structural technical debt to clear the runway for Phase 1.5 (Temporal Safety) and Phase 2 (Networking & SMP).
- [x] **TD-01 (Compiler):** Enhance yield injection with full AST-based cost modeling and expression-level I/O detection for precise jitter budgets.
- [x] **TD-02 (Compiler):** Support comptime `match` statements and non-i32 return value fuzzer generation.
- [x] **TD-03 (Kernel):** Gate System Interface Program (SIP) execution behind interprocedural Z3 signature verification.
- [x] **TD-04 (Kernel):** Implement robust lock-free `DelayedFreeMailbox` draining (Treiber stack & lists) using cross-core atomics.
- [x] **TD-05 (Kernel):** Expand `keuos_reclaim` to aggressively tear down multi-ring IPC tables and disable VirtIO NIC rings.
- [x] **TD-06 (Stdlib):** Finalize reactor event loop syscalls (128-131) for KeuOS and `epoll` shims for Linux port.
- [x] **TD-07 (Stdlib):** Implement Small String Optimization (SSO) in `std::string`.

## Sprint 8: Temporal Safety (Phase 1.5)
**Goal:** Prevent Use-After-Free (UAF) and Double-Free vulnerabilities while maintaining runtime performance.
- [x] **TS-01:** Implement basic affine type tracking (`Uninitialized -> Valid -> Freed`) in the MLIR generator to catch local UAFs statically.
- [x] **TS-02: Contract Enforcements (`requires` / `ensures`)**
  - Extend `@requires` and `@ensures` decorators to support `valid(ptr)`.
  - Inject memory state tokens into the Z3 context.
  - Assert pointer states at function entry and return boundaries.
- [x] **TS-03 (Compiler/Kernel):** Implement Software Memory Tagging (Epoch IDs in top 16 bits of pointers) and introduce `@dynamic_check` for unprovable paths.
