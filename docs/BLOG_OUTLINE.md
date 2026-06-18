# Blog Post Series — Salt + KeuOS Technical Deep-Dives

Three posts designed for an audience of systems programmers, language designers,
and kernel hackers. Target: ~2,000 words each.

---

## Post 1: "Zero-Cost Safety: How Salt Proves Memory Safety at Compile Time"

**Hook:** We replaced runtime bounds checks with mathematical proofs. Here's how.

**Outline:**
1. The Problem: Every CVE is a memory bug. Bounds checks, ASAN, and borrow
   checking all have costs — runtime overhead, annotation burden, or both.
2. The Idea: What if the compiler could prove safety mathematically, and
   simply not emit the check?
3. How It Works: Z3 Proof-or-Panic architecture
   - Translate `requires` expression to Z3 formula
   - Negate the condition, check satisfiability
   - UNSAT → check elided (zero instructions emitted)
   - SAT → compile error with counterexample
   - UNKNOWN → runtime assertion as safe fallback
4. Concrete Example: Binary search with `requires(arr.len() > 0)`
   - Show the Salt code, show the generated MLIR (or lack thereof)
   - Contrast with Rust's `unwrap()` panic path and C's silent UB
5. The Limits: What Z3 can and can't prove
   - 100ms timeout, path explosion
   - Why the fallback is still safe (it's standard MLIR, not custom ops)
6. What This Enables: Verified kernel operations, bounds-checked matmul with
   zero overhead

**Call to Action:** Try it — the compiler is open source.

---

## Post 2: "Microkernel IPC Without the Performance Tax"

**Hook:** Mach was 2x slower than monolithic Unix. We got within 150 cycles.

**Outline:**
1. The Microkernel Performance Problem
   - Context switch cost (~1,000 cycles)
   - Data copy cost (kernel buffer → userspace buffer)
   - Lock contention on shared IPC channels
2. The Three-Optimization Stack
   - **No trap:** SPSC rings in shared memory — data plane is regular load/store
   - **No copy:** DMA writes directly into the ring page; consumer reads same page
   - **No lock:** Single-producer, single-consumer; head/tail on separate cache lines
3. Proof-Carrying IPC: How we prevent descriptor forgery
   - SipHash-2-4 proof hints embedded at compile time
   - Alignment gate (64-byte cache line guard)
   - MMU gate (Ring 3 can't touch Ring 0, period)
4. Benchmark: 150-cycle IPC vs. ~1,000+ for traditional kernel-mediated IPC
5. NetD: The networking daemon that runs entirely in userspace

**Call to Action:** The kernel boots in QEMU — `make setup && make run-qemu`.

---

## Post 3: "Why We Chose Arenas Over Borrow Checking"

**Hook:** Salt has no lifetime annotations. Here's how we still guarantee safety.

**Outline:**
1. The Systems Memory Trilemma
   - Manual (C): fast, unsafe
   - GC (Go/Java): safe, unpredictable latency
   - Borrow checking (Rust): safe + fast, complex lifetime annotations
2. The Arena Model
   - Fixed-size regions, bump-pointer allocation, O(1) bulk free
   - No per-object deallocation = no fragmentation, no free list
3. The Scope Ladder: Compile-time escape analysis
   - Depth-based assignment: deeper values can't be stored in shallower containers
   - Three laws: Return Rule, Assignment Rule, Transitivity Rule
   - Z3 verification of arena mark/reset safety
4. What It Catches
   - Return escape (returning a pointer to a local arena)
   - Store escape (storing a local arena pointer in a long-lived struct field)
   - Use-after-reset (Z3 epoch tracking in debug builds)
5. When Arenas Don't Work
   - Arbitrary graph structures with independent lifetimes
   - Long-lived caches that span request boundaries
   - How we handle these cases (separate allocators, `unsafe` with contracts)
6. Comparison with Rust
   - Equivalent safety, zero annotations
   - The trade: arena discipline vs. lifetime discipline

**Call to Action:** Read the tutorial (`docs/tutorial/`) and open a PR.
