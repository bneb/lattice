# KeuOS Roadmap

This roadmap outlines the planned development and hardening phases for the KeuOS kernel and surrounding infrastructure.

---

## 🛡️ Hardening Statement
*Early versions of KeuOS relied heavily on the premise of formal verification, but security audits revealed issues in the prover integration (Z3 `SAT` inversion) and Ring 0/Ring 3 boundary enforcement. This roadmap incorporates defense-in-depth, negative-testing, and fuzzing to complement the formal verification pipeline.*

---

## Phase 1: Verification and Boundary Hardening *(Months 0-6)*
**Objective:** Address foundational security issues and improve the reliability of the compiler and kernel boundary.

- **Compiler Integrity:** Fix the Z3 `SAT` vs `UNSAT` logical inversion that affected `@requires`. Implement negative-test suites (code that must fail to compile).
- **Memory Boundaries:** Enforce KASLR, SMAP/SMEP, and strict `vaddr` validation in `map_user_page` to protect kernel page tables.
- **IPC Hardening:** Clamp `capacity` and `tail` reads from SPSC shared memory rings. Prevent wrap-around out-of-bounds reads.
- **Resource Management:** Fix user memory leaks on process exit (`destroy_user_pml4`) and Treiber stack double-frees.

## Phase 1.5: Temporal Safety *(Months 6-8)*
**Objective:** Prevent Use-After-Free (UAF) and Double-Free vulnerabilities while maintaining runtime performance.

- **Tier 1: Intraprocedural State Machine:** Implement basic affine type tracking (`Uninitialized → Valid → Freed`) in the MLIR generator to catch local UAFs statically.
- **Tier 2: Interprocedural Z3 Proofs:** Extend `@requires` and `@ensures` decorators to support `valid(ptr)`. Inject memory state tokens into the Z3 context to model temporal transitions across function boundaries.
- **Tier 3: Epoch-Tagged Dynamic Checking:** For concurrent or unprovable paths, introduce the `@dynamic_check` decorator. Implement Software Memory Tagging by embedding allocation Epoch IDs in the top 16 bits of the pointer. 

## Phase 2: Networking and SMP *(Months 6-12)*
**Objective:** Scale networking and multi-processing capabilities.

- **SMP Stability:** Implement atomic CAS for slab cache allocations and fix non-atomic Chase-Lev deque victim bitmap modifications.
- **TCP/IP:** Finalize the NetD bridge. Enable VirtIO RX to Ring 3 SPSC ring communication without system calls.
- **Cross-Core Synchronization:** Implement cross-core TLB shootdowns for guard pages to ensure consistent page faults on stack overflows across all cores.
- **Chaos Testing:** Introduce network fuzzing and connection reset tests against NetD to validate resilience.
- **Interactive TUI:** Build the `grit` shell and establish VirtIO console communication to achieve bare-metal interactive boot capability.

## Phase 3: AI Workloads *(Months 12-18)*
**Objective:** Optimize the operating system for edge AI inference.

- **Basalt Hardening:** Address OOM leaks in sampling and RoPE rotation. Optimize tokenizer pre-scan performance.
- **Hardware Access:** Expose BAR addresses (`nvme_addr`, `rdma_addr`) securely to physical hardware for direct model loading.
- **Scheduling:** Implement an O(1) hierarchical bitmap scheduler optimized for low-latency inference token generation.
- **Pipeline Verification:** Run the NetD, Basalt, and Lettuce pipeline as verified, isolated services.

## Phase 4: Open Ecosystem *(Months 18-24+)*
**Objective:** Expand the software ecosystem and developer tooling.

- **Standard Library (`salt-std`):** Release a userspace library.
- **WebAssembly Sandboxing:** Introduce WASM support in Ring 3 for running untrusted third-party code.
- **POSIX Compatibility Layer:** Allow porting of existing C/C++ applications via a `musl`-backed shim layer.
- **Interactive Web IDE:** Deploy the Salt LSP in the browser, featuring Z3 hover and real-time verification visualization.

> [!CAUTION]
> Phase 2, 3, and 4 depend on the successful implementation and audit of Phase 1.
