# Six-Month Sprint: From Infrastructure to Impact

**Written:** 2026-06-22
**Status:** Active
**Previous sprint:** All 17 goals complete. Kernel boots, NetD runs in Ring 3, Z3 contracts verified, LSP ships, Clippy zero.

---

## 0. The Honest Assessment

We have built a remarkable machine that nobody uses.

The stack is real: a systems language with compile-time formal verification (Salt), a
microkernel with zero-trap SPSC IPC (KeuOS), a verified HTTP service (Lettuce), a
language server, a compiler targeting MLIR/LLVM, benchmarks, CI. All 1,254 tests
pass. The kernel boots. NetD spawns in Ring 3. The Z3 contracts fire on `--verify`.

The deadweight loss is not code quality — the code is good. It's **adoption friction**.
No external contributor can build this stack, understand it, or ship something with it
in under a week. The project exists for itself. Every hour spent splitting files or
eliminating deep nests is an hour not spent making this usable by someone who isn't us.

The quality sprint was correct for its time — we eliminated 186 deep-nest blocks,
extracted ~1,400 lines from oversized files, created 6 clean submodules, and achieved
clippy zero. The compiler is now in a state where new contributors can read it without
needing a map. That work is done. It is not the work that matters next.

**What matters next is making the stack legible to the outside world.**

---

## 1. The Strategic Thesis

Most verified-systems projects fail for the same reason: they optimize for the proof,
not the programmer. Dafny, F*, Coq, Isabelle — these are proof assistants that happen
to produce code. The result is software that is correct but unshippable: no standard
library, no tooling, no ecosystem, no users.

Salt inverts this. The thesis is: **verification is a compiler feature, not a
separate tool**. You write systems code that looks like Rust. The compiler uses Z3 to
prove your `requires` and `ensures` clauses at compile time. When it can't prove them,
it falls back to runtime assertions — your program still compiles, still runs, still
has defined behavior. The verification is _progressive_: you can add contracts
incrementally to an existing codebase without rewriting it.

This is unique. No other systems language offers this. Rust gives you memory safety
but not functional correctness. Zig gives you comptime but not formal proofs. C/C++
gives you neither. Salt gives you both, in a single compiler pass, without a separate
verification toolchain.

If this thesis is correct, Salt should be the default choice for any software where
correctness matters and performance matters — network infrastructure, kernel modules,
cryptographic protocols, edge compute runtimes, database storage engines.

The six-month plan is about proving this thesis in public.

---

## 2. The Six-Month Arc

### Month 1–2: The Lettuce Launch (weeks 1–8)

**Thesis:** The best way to prove a systems language works is to ship a working system.

**What we do:** Productionize Lettuce — our verified HTTP service — into a
benchmarkable, deployable, bloggable artifact. This is not about adding features.
It's about making the existing verified RESP parser, AOF store, and HTTP dispatch
loop into something that can be demonstrated, benchmarked, and understood by an
external engineer in an afternoon.

**Specific deliverables:**
- [ ] **Lettuce runs in QEMU with `make lettuce`** — one command, boots kernel, spawns
  NetD, starts Lettuce, serves HTTP on VirtIO, accessible from host curl
- [ ] **Benchmark against nginx and Redis** — requests/sec, latency p50/p99, memory
  footprint. Publish the numbers even if we lose. Honesty builds trust.
- [ ] **Lettuce tutorial** — a step-by-step guide to building a verified HTTP endpoint:
  define the route, add a `requires` contract on the input, add an `ensures` contract
  on the response, compile with `--verify`, see the Z3 output
- [ ] **Blog post 1 ships:** "Zero-Cost Safety: How Salt Proves Memory Safety at Compile
  Time" (already outlined in `docs/BLOG_OUTLINE.md`)
- [ ] **Hacker News launch:** Blog post + Lettuce benchmark + "try it in QEMU" repo

**The metric:** Can someone who has never seen Salt clone the repo, run `make lettuce`,
see it serve HTTP, and understand _why_ it's verified?

### Month 3–4: The Compiler 1.0 (weeks 9–16)

**Thesis:** A language without a stable compiler is a research project. A language with
a stable compiler is infrastructure.

**What we do:** Freeze the Salt language surface, ship v1.0.0 of the compiler with
documentation, and make the LSP good enough that someone would choose to write Salt
over Rust for a new project.

**Specific deliverables:**
- [ ] **Language spec frozen** — `docs/SPEC.md` is the authoritative reference. Every
  syntax feature documented. Every divergence from Rust documented with rationale.
- [ ] **Compiler v1.0.0** — `saltc` binary with stable CLI, `--verify` flag, `--target`
  flag, `--release` flag. Error messages that explain what went wrong and how to fix it.
- [ ] **"Salt by Example"** — 8 chapters covering: variables and types, functions and
  contracts, structs and enums, pattern matching, generics, arenas, FFI, async/await.
  Each chapter is a compilable Salt program with inline comments.
- [ ] **LSP v0.5.0** — semantic tokens, go-to-def, find-refs, hover with Z3
  counterexamples, code actions for adding contracts. Fast enough to use in anger
  (<100ms for go-to-def on a 5,000-line file).
- [ ] **Standard library audit** — every public function in `salt-std` has a doc
  comment, a usage example, and (where applicable) a `requires` clause.
- [ ] **Blog post 2 ships:** "Microkernel IPC Without the Performance Tax"

**The metric:** Can someone who knows Rust read "Salt by Example", install the VS Code
extension, and write a working Salt program with verified contracts in under an hour?

### Month 5–6: The Ecosystem Seed (weeks 17–24)

**Thesis:** Languages don't win on features. They win on ecosystem. The first external
contributor is worth more than a thousand internal refactors.

**What we do:** Ship the package manager, publish the first third-party Salt package,
and make it possible for an external engineer to contribute a feature to the compiler
without a hand-holding session.

**Specific deliverables:**
- [ ] **Package manager v0.1.0** — `sp add <git-url>` pulls a dependency, `sp build`
  resolves and compiles the graph, `sp test` runs the test suite. Git-based registry
  (no server infrastructure needed). Version resolution via PubGrub.
- [ ] **First external package** — find one external contributor, help them build one
  Salt package (a JSON parser, a regex engine, a small CLI tool — anything real),
  publish it to the registry, document the process as a tutorial
- [ ] **Contributor ladder** — `CONTRIBUTING.md` with: how to set up a dev environment,
  how to run tests, how to add a new language feature, how to add a new optimization
  pass, how to add a Z3 contract. Each step links to a working example PR.
- [ ] **CI completeness** — macOS and Linux builders, kernel smoke test, benchmark
  regression, code coverage reporting. A green CI badge on the README.
- [ ] **Blog post 3 ships:** "Why We Chose Arenas Over Borrow Checking"
- [ ] **Conference talk submitted** — Strange Loop, Systems @ Scale, or RustConf. Topic:
  "Compile-Time Verification for Systems Programmers: How Salt Embeds Z3 in the
  Compiler Pipeline"

**The metric:** Can an external engineer make a non-trivial contribution to the
compiler within one week of cloning the repo?

---

## 3. The Non-Goals (What We Say No To)

Every hour spent on these is an hour stolen from the launch:

- **Self-hosting compiler.** Salt written in Salt is a beautiful goal for 2027. It is
  not a v1.0.0 goal. The Rust compiler is good enough.
- **RISC-V or ARM port.** x86-64 is the launch target. Ports fragment the testing
  matrix and slow down the feedback loop. Ship on one architecture first.
- **More kernel features.** The kernel boots, NetD runs, TCP works. More kernel
  features before external users exist is a form of procrastination.
- **AI/ML workloads (Basalt).** The ROADMAP.md Phase 3 is aspirational. Without users,
  there is no one to run AI workloads. Without users, there is no one to care about
  O(1) scheduling. Users first, features second.
- **File splitting for its own sake.** The compiler has 38 files over 500 LOC. Some
  of these are legitimate: tightly coupled logic on large structs, match trees on AST
  types, recursive codegen patterns. The quality sprint extracted what could be
  extracted. What remains is genuinely hard to decompose. Stop optimizing. Ship.
- **POSIX compatibility.** Porting musl to KeuOS is a year of work for zero
  differentiation. Salt programs should target Salt APIs. The WASM sandbox is a
  better compatibility story.

---

## 4. The Weekly Cadence

| Day | Activity |
|-----|----------|
| Monday | Feature work (Lettuce, compiler, docs) |
| Tuesday | Feature work |
| Wednesday | Testing, benchmarking, blog writing |
| Thursday | Feature work |
| Friday | Review, cleanup, metric tracking |
| Weekend | Blog posts published Monday AM |

**Each Monday morning:** 5-minute metrics check:
- [ ] `cargo test --lib` — all passing?
- [ ] `cargo clippy -- -D warnings` — clean?
- [ ] `make lettuce` — boots and serves HTTP?
- [ ] Deep nest count not increased from prior week?
- [ ] No new files over 500 LOC?
- [ ] One external-facing artifact shipped this week? (blog post, doc update, benchmark, bug fix that helps a user)

---

## 5. The Pivot Signal

If by the end of Month 4 (compiler v1.0.0) we have **zero external users** despite
shipping a working compiler, documented language, verified HTTP service, and published
blog posts, we have a positioning problem, not an execution problem.

In that case, pivot:
- **Option A: Verified edge proxy.** Lettuce + NetD + SPSC rings = a verified L7
  proxy that Cloudflare/Fastly would pay attention to. The pitch: "nginx with formal
  proofs that it won't crash on malformed input."
- **Option B: WASM runtime.** Compile Salt to WebAssembly, ship a Salt-based WASM
  runtime with Z3-verified sandboxing. The pitch: "WASM but the compiler proves your
  sandbox is correct."
- **Option C: Embedded/RTOS.** Target Cortex-M, leverage arena allocation for
  deterministic memory. The pitch: "Rust for embedded but with compile-time proofs
  that your ISR won't overflow the stack."

None of these are the plan. All of them are valid if the plan fails. But the plan
succeeds if we make the stack legible. Legibility is the bottleneck. Everything
else is a distraction.

---

## 6. The One Sentence

**Salt is the first systems language where the compiler mathematically proves your
code is correct — and if it can't, it still compiles and runs safely.**

That sentence, if true and demonstrable, is worth more than any amount of internal
quality work. The next six months are about making it true and demonstrable. Everything
else waits.
