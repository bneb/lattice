# KeuOS & Salt - Hardened Sprint Plan

*This sprint plan has undergone a rigorous Red Team analysis. We are no longer simply "fixing bugs"; we are closing critical exploitation vectors and hardening the architecture against adversarial input.*

## Sprint 1: Compiler Soundness & Negative Testing
**Goal:** The compiler must be mathematically trustworthy. Stop trusting Z3 blindly.
- [ ] **SEC-01:** Fix Z3 `SAT` vs `UNSAT` inversion in `@requires` verification (`salt-front/src/codegen/verification/mod.rs`). The verifier MUST assert the negation and check for `UNSAT`.
- [ ] **SEC-02:** Integrate `ptr_bounds_verifier.rs` into `emit_index` and `emit_lvalue` to enforce mandatory bounds checking on `Ptr<T>` indexing.
- [ ] **SEC-03:** Fix generic unification type confusion in `generic_resolver.rs`. Enforce structural equivalence checks for multiple usages of the same generic parameter.
- [ ] **SEC-04:** Establish a "Negative Compilation" test suite. Prove that out-of-bounds indices and type confusion now physically fail to compile.

## Sprint 2: Kernel Privilege Escalation & Memory Corruption
**Goal:** Seal the Ring 0 / Ring 3 boundary. User processes must not dictate kernel state.
- [ ] **SEC-05:** Remove `syscall_set_moe_bar_ptr`. The kernel must NEVER allow a user process to define arbitrary 64-bit physical memory targets.
- [ ] **SEC-06:** Fix `map_user_page` in `user_paging.salt` to violently `panic()` if `vaddr >= 0x0000800000000000`. Prevent user manipulation of the kernel PML4 half.
- [ ] **SEC-07:** Restrict `USER_TABLE_FLAGS` in intermediate tables (PDP, PD, PT) so that `Write` and `User` bits are only granted if explicitly required by the leaf mapping.
- [ ] **SEC-08:** Mitigate Fastpath IPC Array OOB: Add strict `if cap_id >= 64 { return ERR; }` bounds checks before accessing `FASTPATH_PHYS_PTR`.

## Sprint 3: The Untrusted SPSC Threat Vector
**Goal:** Assume the data plane is actively trying to compromise the system.
- [ ] **SEC-09:** Clamp `capacity` reads in `spsc_push_bulk`/`spsc_pop_bulk` (`ipc_shm.salt`). Never trust the capacity field written by the consumer/producer.
- [ ] **SEC-10:** Fix `terminal_tx_poll_thread` (#DE) division-by-zero crash by validating that the shared ring `capacity > 0`.
- [ ] **SEC-11:** Redesign `ipc_ring_push_bytes` to handle partial write failure correctly instead of dropping bytes mid-frame.
- [ ] **SEC-12:** Fix SPSC wrap-around linear copy logic in `user/lib/ring.salt`. Split operations into two distinct memory segments if crossing the physical boundary.

## Sprint 4: PMM Corruption & Resource Exhaustion (DoS)
**Goal:** Eradicate leaks and data races that allow trivial Denial of Service.
- [ ] **SEC-13:** Fix the catastrophic double-free in `destroy_user_pml4`. The loop MUST start at `pml4_i = 0` (or correctly handle the lower canonical half) to prevent total user memory leak.
- [ ] **SEC-14:** Implement a Used/Free bitmask in `pmm_sharded.salt` to detect and prevent Treiber stack cycle corruption (Double Free).
- [ ] **SEC-15:** Validate that the address passed to `free_frame` is within physical RAM bounds and non-zero.
- [ ] **SEC-16:** Overhaul FD generation in `user/lib/socket.salt` to prevent predictable `port % 256` FD guessing attacks.

## Sprint 5: Concurrency, SMP & Basalt Hardening
**Goal:** Eliminate race conditions and secure the AI inference workload.
- [ ] **SEC-17:** Replace non-atomic reads/writes in `dispatch_stolen` (Scheduler) and Slab Cache Allocator with proper hardware CAS instructions.
- [ ] **SEC-18:** Implement cross-core TLB shootdowns in `vmm_clear_present` to guarantee guard page effectiveness across all CPUs.
- [ ] **SEC-19:** Patch Basalt Memory Leaks: Free `prob_buf` and `idx_buf` in `sample_topp`, and `freq_cis` in `basalt_engine_free`.
- [ ] **SEC-20:** Fix the RoPE rotation offset bug in `model_loader.salt` to ensure $w_{ptr}$ advances correctly for imaginary components.
- [ ] **SEC-21:** Fix GPU Command Buffer overflow in `compositor.salt` by adding explicit bounds checking against `GPU_RECT_BUF` size.
