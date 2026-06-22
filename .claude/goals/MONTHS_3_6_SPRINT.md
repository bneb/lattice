# Months 3–6 Sprint — Compiler 1.0 + Ecosystem Seed

**Written:** 2026-06-24
**Previous sprint:** LETTUCE Launch ✅ (benchmarks, 9 commands, blog post, tutorial, design)

---

## Phase A: Documentation (parallel, ~4 hours wall clock)

### A1. "Salt by Example" — 8 chapters
Each chapter is a self-contained Salt program with inline comments. Target: a Rust developer can read one chapter and write that feature in Salt.

| Chapter | Topic | Lines | Agent |
|---------|-------|-------|-------|
| 01 | Variables, types, printing | ~60 | A1a |
| 02 | Functions, requires, ensures | ~80 | A1b |
| 03 | Structs, enums, pattern matching | ~80 | A1c |
| 04 | Generics and monomorphization | ~80 | A1d |
| 05 | Arenas and memory model | ~80 | A1e |
| 06 | Error handling, Result, pipe | ~80 | A1f |
| 07 | FFI and extern functions | ~80 | A1g |
| 08 | Async, yield, state machines | ~80 | A1h |

### A2. Standard library audit
Document every public function in `salt-std`. Each module is independent.

| Module | Agent |
|--------|-------|
| `std.core.ptr` | A2a |
| `std.core.str` | A2b |
| `std.core.result` | A2c |
| `std.collections.string_map` | A2d |
| `std.collections.slab` | A2e |
| `std.net.tcp` | A2f |
| `std.net.poller` | A2g |
| `std.fs` | A2h |

### A3. Blog posts 2 and 3
- **Post 2:** "Microkernel IPC Without the Performance Tax" — SPSC rings, 150-cycle IPC, NetD architecture
- **Post 3:** "Why We Chose Arenas Over Borrow Checking" — scope ladder, escape analysis, comparison with Rust

### A4. Contributor ladder
- `CONTRIBUTING.md`: dev environment setup, test commands, PR process, how to add a language feature, how to add an optimization pass, how to add a Z3 contract. Each section links to a working example.

---

## Phase B: Compiler Stabilization (sequential, ~4 hours)

### B1. Language spec freeze
- Audit `docs/SPEC.md` against compiler behavior
- Document every divergence from Rust syntax with rationale
- Mark experimental features as such
- Freeze the surface: no breaking syntax changes without edition mechanism

### B2. saltc CLI
- Stable `saltc` binary entry point
- `--verify`, `--target`, `--release`, `--lib` flags documented
- Error messages that explain what went wrong and how to fix it
- `--explain E1234` for detailed error docs

### B3. Standard library freeze
- Every public function has a doc comment and usage example
- Every `unsafe` in stdlib has documented justification
- Public API surface documented in `docs/stdlib/`

---

## Phase C: Ecosystem (semi-parallel)

### C1. Package manager v0.1.0
- `sp add <git-url>` — pull dependency, verify contracts
- `sp build` — resolve graph, compile
- `sp test` — run test suite
- Git-based registry (no server infrastructure)
- Version resolution via PubGrub

### C2. First external package
- Identify one compelling external use case
- Help one external contributor build and publish it
- Document the process as a tutorial

### C3. CI completeness
- macOS + Linux builders
- Kernel smoke test in CI
- Benchmark regression tracking
- Code coverage reporting

---

## Success Criteria

- [ ] "Salt by Example": 8 chapters complete, each compiles and runs
- [ ] Standard library: every public function documented
- [ ] Blog posts 2 and 3 published
- [ ] CONTRIBUTING.md: working contributor ladder
- [ ] Language spec frozen
- [ ] saltc CLI shipped
- [ ] Package manager: sp build works on a clean checkout
- [ ] CI: green badge on README
