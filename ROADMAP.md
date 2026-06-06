# KeuOS Roadmap: The Path to 10k Stars

This roadmap outlines the progression of KeuOS from a vulnerable research kernel to an ironclad, formally verified infrastructure for edge AI agents.

---

## 🛡️ Red Team Hardening Statement
*The previous iteration of KeuOS relied heavily on the premise of formal verification. However, a rigorous red-team audit revealed critical failures in the prover integration (Z3 `SAT` inversion) and catastrophic Ring 0/Ring 3 boundary porosity. This roadmap has been hardened. We no longer assume the compiler is infallible. We are introducing defense-in-depth, negative-testing, and adversarial fuzzing as first-class citizens.*

---

## Phase 1: The Formal Crucible *(Months 0-6)*
**Objective:** Eradicate foundational security vulnerabilities. Rebuild trust in the compiler and the kernel boundary.

- **Compiler Integrity:** Fix the Z3 `SAT` vs `UNSAT` logical inversion that renders `@requires` useless. Implement negative-test suites (code that *must* fail to compile).
- **Ironclad Memory Boundaries:** Enforce KASLR, SMAP/SMEP, and strict `vaddr` validation in `map_user_page` to prevent kernel page table corruption.
- **Untrusted IPC Hardening:** Treat SPSC ring buffers as hostile territory. Clamp all `capacity` and `tail` reads from shared memory. Fix wrap-around out-of-bounds reads.
- **Leak Eradication:** Fix the total user memory leak on process exit (`destroy_user_pml4`) and Treiber stack double-frees that cause catastrophic physical memory corruption.

## Phase 1.5: Zero-Cost Temporal Safety *(Months 6-8)*
**Objective:** Eradicate Use-After-Free (UAF) and Double-Free vulnerabilities without introducing Rust's borrow checker friction or sacrificing runtime performance.

- **Tier 1: Intraprocedural State Machine:** Implement basic affine type tracking (`Uninitialized → Valid → Freed`) directly in the MLIR generator. Zero developer annotation burden, zero runtime cost. Catch local UAFs instantly.
- **Tier 2: Interprocedural Z3 Proofs:** Extend `@requires` and `@ensures` decorators to support `valid(ptr)`. Inject memory state tokens into the Z3 context to model temporal transitions across function boundaries without runtime overhead.
- **Tier 3: Epoch-Tagged Dynamic Checking:** For unprovable concurrent/unstructured paths, introduce the `@dynamic_check` decorator. To preserve 8-byte ABI compatibility and avoid fat pointers, implement Software Memory Tagging by embedding allocation Epoch IDs in the top 16 bits of the pointer. Incurs ~2-5% overhead *only* on explicitly decorated functions.

## Phase 2: The KeuOS Network *(Months 6-12)*
**Objective:** Production-grade Networking and SMP. Moving from proof-of-concept to 10M+ packets/sec.

- **SMP Stability:** Implement atomic CAS for slab cache allocations and fix non-atomic Chase-Lev deque victim bitmap modifications.
- **Zero-Trap TCP/IP:** Finalize the NetD bridge. VirtIO RX to Ring 3 SPSC ring pump without system calls.
- **Cross-Core Synchronization:** Implement cross-core TLB shootdowns for guard pages to ensure deterministic page faults on stack overflows across all 16 cores.
- **Adversarial Chaos Testing:** Introduce network fuzzing and connection reset chaos tests against NetD to prove resilience under duress.

## Phase 3: The Edge AI Appliance *(Months 12-18)*
**Objective:** Realizing the end-to-end agent runtime. The OS is the inference engine.

- **Basalt Hardening:** Eliminate OOM leaks in sampling and RoPE rotation. Fix O(N^2) tokenizer pre-scan bottlenecks.
- **Direct Hardware Access:** Expose zero-trap BAR addresses (`nvme_addr`, `rdma_addr`) securely to physical hardware for zero-copy model loading.
- **Cooperative Reactor Scheduling:** O(1) hierarchical bitmap scheduling optimized specifically for low-latency AI inference token generation.
- **Full Pipeline Verification:** Run the complete NetD, Basalt, and Lettuce pipeline entirely as verified, isolated services.

## Phase 4: Open Ecosystem & WasmerOS *(Months 18-24+)*
**Objective:** Scaling to a vibrant 10k-star open-source ecosystem.

- **Standard Library (`salt-std`):** Release a comprehensive, secure-by-default userspace library.
- **WebAssembly Sandboxing:** Introduce WASM support in Ring 3 for running untrusted third-party code with mathematical isolation guarantees.
- **POSIX Compatibility Layer:** Allow porting of existing C/C++ applications via a `musl`-backed shim layer running on top of KeuOS primitives.
- **Interactive Web IDE:** Deploy the Salt LSP in the browser for frictionless onboarding, featuring zero-I/O Z3 hover and real-time verification visualization.

> [!CAUTION]
> Phase 2, 3, and 4 cannot commence until Phase 1 is rigorously verified by a secondary independent audit. The foundation must be structurally sound before we scale.
