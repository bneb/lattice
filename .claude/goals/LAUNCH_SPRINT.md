# Launch Sprint — From Working Kernel to Shippable Product

**Date:** 2026-06-25
**Status:** Active
**Previous:** Six-Month Sprint (all 17 goals complete)

---

## 0. Where We Are

The KeuOS stack is real. The kernel boots. The TCP stack does HTTP fetch end-to-end.
Server-side TCP syscalls work. The loopback queue delivers packets without recursion.
The Salt compiler supports progressive Z3 verification. The LSP ships.

The infrastructure is solid. What remains is **polish, packaging, and proof.**

### Assets We Have

| Asset | Status | Evidence |
|---|---|---|
| Kernel boots to Ring 3 | ✅ | 12/12 tests pass |
| TCP client (connect/send/recv/close) | ✅ | HTTP 200 from Python server |
| TCP server (listen/accept/close) | ✅ | Verified via Process L |
| Loopback queue | ✅ | No recursion, proper RX path |
| ICMP ping | ✅ | Echo request/reply from 10.0.2.2 |
| Salt compiler | ✅ | 1254 tests, clippy clean |
| LSP (language server) | ✅ | VS Code extension ships |
| Z3 contract verification | ✅ | 4/4 Lettuce contracts pass |
| MAX_PROCS=64 | ✅ | Room for growth |
| LAST_DISPATCHED scheduler fix | ✅ | True round-robin |
| kernel_trampoline fix | ✅ | No ud2 crash on thread return |
| ECS entity registration | ✅ | All processes are entities |

### What's 95% Done

| Item | Status | Remaining |
|---|---|---|
| Loopback demo | 95% | Echo response needs daemon to stay alive |
| Ring 0 preemption | 90% | Context save/restore in timer ISR |
| Build system | 90% | First-build-after-clean flakes |

---

## 1. The Goal

**Ship KeuOS as a compelling, demonstrable verified systems stack.**

The unique value proposition: *progressive formal verification in a systems language.*
You write code that looks like Rust. The compiler proves your `requires` and `ensures`
clauses at compile time with Z3. When it can't prove them, it falls back to runtime
assertions. You can add contracts incrementally without rewriting your codebase.

No other systems language offers this. Rust gives memory safety but not functional
correctness. Zig gives comptime but not formal proofs. Salt gives both.

The launch needs:
1. A self-contained demo that works every time
2. A tutorial that gets someone from zero to a working verified program in 15 minutes
3. A technical blog post that explains the unique architecture
4. A benchmark that proves Salt/KeuOS performance is competitive
5. An HN launch that drives attention

---

## 2. The Sprint — 4 Weeks to Launch

### Week 1: The Demo (Fix What's Blocked)

**Goal:** A self-contained loopback demo with echo response. Runs on `make demo`.

#### S1: Fix daemon execution ✅ (architecture done, end-to-end blocked by S3)
- [x] Blocking accept() syscall architecture (commit 2154d6c)
  - Added PROC_BLOCKED_ACCEPT (state 5), accept() sleeps until connection arrives
  - sched_wake_accept() wakes blocked listener from handle_server_syn
  - switch_to_process() preserves blocked state (does not overwrite BLOCKED→READY)
  - Fixed sys_tcp_listen() to store real process slot (was hardcoded 1)
  - Fixed MAX_PROCS 16→64 in scheduler to match process table
  - Added Task 0 fairness cooldown to prevent pulse-driven starvation
  - echo_server.salt: clean blocking accept, no yield/timeout loop
- [ ] End-to-end echo response blocked by S3 (fetch/ping do not execute at high slot numbers)
- [ ] Dual-scheduler integration: ECS fiber scheduler (do_dispatch) coexists with process scheduler (schedule_next); kernel threads use ECS, Ring 3 uses process — they don't yield to each other cleanly

#### S2: Fix build system flake ✅ (fixed — deterministic builds)
- [x] Root cause: HashMap iteration order in salt-front compiler caused
  non-deterministic constant/global emission. `dependency_graph.keys()` and
  `loaded_files.values()` iterated in HashMap order, producing different
  topological sort and different MLIR output each run.
- [x] Fix 1: sort `dependency_graph.keys()` in `get_compilation_order()` (module_loader.rs)
- [x] Fix 2: replace `loaded_files.values()` iterations with sorted-order lookup (mod.rs)
- [x] Fix 3: add `sort_global_decls()` post-processing step that sorts all
  `llvm.mlir.global` declarations by symbol name (mod.rs)
- [x] Fix 4: sort glob results + linker inputs in runner_qemu.py
- [x] Fix 5: binary name corrected (saltc, not salt-front) in runner_qemu.py
- [x] Verification: 20/20 salt-front compilations produce identical MLIR.
  Kernel binaries are byte-identical across clean builds.
  Test results are now deterministic (9/11 pass consistently).

#### S3: Stabilize 8-program test suite ✅ (11/11 — all pass)
- [x] Root cause: pre-scheduler kernel threads at slots 0-1 starved higher-slot
  Ring 3 processes. Timer ISR resets LAST_DISPATCHED to 0 on each pulse-driven
  dispatch, so round-robin never reaches slots 12+ (ping, fetch).
- [x] Fix 1: block pre-scheduler processes (terminal, TX Poll, NetD, echo_server)
  with PROC_IPC_BLOCKED so process scheduler skips them
- [x] Fix 2: kernel PML4 comparison in exit-path dispatch filter. Kernel threads
  have user_pml4 == kernel_pml4 (non-zero). Compare against slot 0's PML4 instead
  of zero to correctly identify Ring 3 processes.
- [x] Bridge ECS/process schedulers: salt_yield_check calls schedule_next()
- [x] Prevent stack overflow: dispatchable-count check in schedule_next
- [x] Fix stale MAX_PROCS=16 in exec_user, spawn_coroutine, spawn_inode, syscall_ipc
- [x] Verified: 11/11 tests pass consistently

### Week 2: The Story (Write What We Built)

#### S4: Blog post 1 — "Zero-Cost Safety" ✅ (published)
- [x] Written and committed: `docs/blog/zero-cost-safety.md`
- [x] Covers Z3 Proof-or-Panic architecture, safe_div/binary_search/kernel examples,
  MLIR emission paths, progressive verification, comparison table
- [x] Added to docs README index
- [ ] Review pass by another person (sprint requirement)
- Post 2 (S11) and Post 3 (S12) remain for Week 4

#### S5: Tutorial — "Your First Verified Salt Program" ✅ (published)
- [x] Written and committed: `docs/tutorial/your-first-verified-program.md`
- [x] 8-step walkthrough: hello world → data structures → insert → lookup →
  verify → Z3 counterexample → postconditions → full picture
- [x] Builds a verified key-value store with requires/ensures contracts
- [x] Shows Z3 contract violation as compile error (not runtime crash)
- [x] Added to tutorial README and docs index

#### S6: Lettuce benchmark ✅ (complete — benchmarks exist, docs indexed)
- [x] Benchmark script: `benchmarks/lettuce_bench.sh` — reproducible harness using redis-benchmark
- [x] Analysis: `benchmarks/LETTUCE_BENCH.md` — command coverage (PING/SET/GET/INCR),
  concurrency sweep (1-100 clients), data size sweep, pipelined throughput,
  verification cost, arena-vs-malloc analysis
- [x] Results: Lettuce leads Redis at every concurrency level (1.1-6.8x).
  314-line server, zero heap allocations per request, arena-backed storage.
- [x] Algorithm benchmarks: `benchmarks/BENCHMARKS.md` — 12 algorithms (Salt vs C/Rust)
- [x] Added to docs README index

### Week 3: The Launch (Package and Ship)

#### S7: Documentation sweep (1 session)
- Update README with current architecture
- Verify all `docs/` files match current code
- Add architecture diagram
- Add "Getting Started" guide

#### S8: HN launch prep (1 session)
- Write the "Show HN" post
- Prepare FAQ for common questions ("Why not Rust?", "Is this production-ready?")
- Set up a landing page (GitHub Pages or similar)

#### S9: Launch (1 session)
- Post to HN
- Post to Reddit (r/rust, r/programming)
- Monitor and respond to comments
- Track metrics (stars, issues, contributors)

### Week 4: Sustain (Follow Through)

#### S10: Respond to feedback (ongoing)
- Triage GitHub issues
- Fix critical bugs reported by early users
- Write responses to blog comments

#### S11: Second blog post — "The ECS Kernel Architecture" (2 sessions)
- **Topic:** How KeuOS uses Entity Component System instead of traditional VFS.
  Columnar storage, cache-friendly queries, zero-allocation TCP pool.
- **Deliverable:** Technical deep-dive.

#### S12: Third blog post — "Progressive Verification" (2 sessions)
- **Topic:** The Z3 integration architecture. How Salt compiles `requires` clauses
  to SMT solver constraints. The SAT/UNSAT polarity. What happens when Z3 can't prove
  something (runtime fallback).
- **Deliverable:** Technical deep-dive with Z3 trace output.

---

## 3. The Technical Debt (Not Blocking Launch)

These are real issues but don't block the demo or the launch:

| Issue | Impact | Fix |
|---|---|---|
| `ip_transmit()` duplication | 4 copies of eth+IP+checksum+VirtIO | Extract to `net_tx.salt` |
| `tcp_dispatch.salt` import crash | Can't add eth/ip imports without linker crash | Root cause in Salt compiler |
| Cross-module const arrays | `kernel_config.MAX_PROCS` can't size arrays | Compiler fix (Expr::Field handling) |
| ECS scheduler not wired | Legacy round-robin runs alongside ECS | Wire `scheduler_system_tick` into dispatcher |
| 4KB kernel stack limit | Deep call chains overflow | Multi-page contiguous allocation |
| `copy_from_user` audit | All sites already use safe functions | ✅ Clean |

---

## 4. The DevEx Improvements (After Launch)

| Item | Impact |
|---|---|
| `make demo` — one command to build + boot + run loopback test | Removes QEMU friction |
| `make benchmark` — one command for Lettuce vs nginx/Redis | Repeatable perf measurement |
| `saltc --test` — run contracts without compiling | Fast verify cycle |
| Docker image with all dependencies | No "works on my machine" |
| GitHub Actions CI for external contributors | PRs get tested automatically |

---

## 5. Measurement Gates

After each sprint item:
- `make test` passes (cargo test + kernel boot)
- `make demo` completes (if applicable)
- All blog posts reviewed by at least one other person
- Benchmarks are reproducible on a clean checkout

---

## 6. The Vision

KeuOS is not trying to be Linux. It's trying to prove that **formal verification can be
a compiler feature, not a separate tool.** The pitch is:

> Write systems code that looks like Rust. Add `requires` and `ensures` clauses.
> The compiler proves them at compile time with Z3. Ship verified software without
> a separate verification toolchain.

This is unique. No other systems language offers progressive verification. If we can
demonstrate this clearly — with a working demo, a tutorial, benchmarks, and technical
depth — the project earns its place in the systems programming conversation.
