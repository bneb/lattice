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

#### S1: Fix pre-scheduler daemon execution (1 session)
- **Problem:** Daemon spawned via `exec_spawn_process` in `boot_helpers` never executes.
  Process dump shows slot 3 exists with state=PROC_READY but never makes a syscall.
- **Root cause:** `schedule_next` is called from dispatcher (slot 0) which always
  searches from `(current+1)`. Kernel threads at slots 1-2 are found first and
  monopolize CPU. With LAST_DISPATCHED, the cursor advances but the timer ISR only
  preempts Ring 3, so kernel threads never yield control back.
- **Fix options:**
  a. Run daemon as Ring 3 user program (Process L) — works but 8 programs cause crash
  b. Add `schedule_next()` call to kernel thread loops
  c. Fix timer ISR to preempt Ring 0 (context save/restore needed)
  d. Move daemon spawn to post-scheduler (main.salt) where user programs run

#### S2: Fix build system flake (1 session)
- **Problem:** First build after `rm -rf qemu_build` produces a kernel where user
  programs don't execute. Second build (cached) works.
- **Fix:** Investigate build cache interaction with embedded_user.S incbin.
  May need to ensure user programs are built before kernel compilation.

#### S3: Stabilize 8-program test suite (1 session)
- **Problem:** Adding Process L (8th user program) causes `AB#DF!` crash at RIP=0.
- **Root cause:** Likely process table or stack exhaustion. MAX_PROCS=64 should be
  sufficient, but something else overflows.
- **Fix:** Debug with GDB to find the exact crash site.

### Week 2: The Story (Write What We Built)

#### S4: Blog post 1 — "Zero-Cost Safety" (2 sessions)
- **Topic:** How Salt proves memory safety at compile time without runtime overhead.
  Walk through a `requires`/`ensures` example with Z3. Show the MLIR output.
  Contrast with Rust's borrow checker and Zig's comptime.
- **Deliverable:** 2000-word post with code snippets, diagrams, MLIR output.

#### S5: Tutorial — "Your First Verified Salt Program" (2 sessions)
- **Topic:** Install Salt, write a key-value store with `requires` and `ensures`,
  compile with `--verify`, see Z3 prove the contracts.
- **Deliverable:** Step-by-step tutorial with copy-pasteable code. Should work
  in under 15 minutes on a fresh macOS/Linux machine.

#### S6: Lettuce benchmark (2 sessions)
- **Topic:** Benchmark Lettuce against nginx and Redis on equivalent workloads.
  Measure latency, throughput, and memory. Salt's zero-cost abstractions should
  be competitive with C/Rust.
- **Deliverable:** Benchmark script + graphs + analysis.

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
