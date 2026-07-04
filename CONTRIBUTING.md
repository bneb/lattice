# Contributing to Lattice / KeuOS

This is a contributor reference. Each section links to a concrete example in the codebase.

---

## 1. Dev Environment Setup

```bash
git clone https://github.com/bneb/keuos.git && cd keuos

make setup        # install LLVM 21, Z3 4.12+, build the Salt compiler
make build        # compile salt-front (Rust) in release mode
make test         # run all compiler unit tests
```

Prerequisites (installed by `make setup`):

| Dependency | Role |
|:-----------|:-----|
| LLVM 21 | `mlir-opt`, `mlir-translate`, `clang` |
| Rust 1.75+ | Salt compiler (`salt-front/`) |
| Z3 4.12+ | Formal verification of memory-safety contracts |
| Python 3 | Build scripts, fuzzing, integration tests |

See [`scripts/bootstrap.sh`](scripts/bootstrap.sh) for the full install logic.

---

## 2. Project Structure Overview

```
keuos/
├── salt-front/           Salt compiler (Rust)
│   └── src/
│       ├── grammar/         AST definitions, parser (syn-based)
│       ├── hir/             High-level IR (typeck, lowering, scope)
│       ├── codegen/         MLIR emission, SIR, passes, verification
│       │   ├── passes/          Compiler passes (liveness, pulse injection, async→state)
│       │   ├── verification/    Z3 contract verification engine
│       │   ├── sir/             Salt IR (mid-level representation)
│       │   └── context/         Lowering context and codegen driver
│       ├── passes/          Comptime evaluation
│       └── common/          Mangling, shared utilities
├── kernel/               KeuOS kernel (Salt source)
│   ├── arch/              HAL router (x86_64, aarch64, riscv)
│   ├── core/              Scheduler, process lifecycle
│   ├── mem/               Physical/virtual memory manager
│   ├── sched/             Scheduling policies and topology
│   ├── ipc/               Inter-process communication
│   ├── net/               Network stack
│   ├── drivers/           Device drivers
│   └── sys/               System call dispatch
├── std/                  Standard library modules (Salt source)
├── lettuce/              Verified HTTP/Redis server (flagship app)
├── basalt/               Baseline ML framework
├── tools/
│   ├── salt-lsp/         Language server (completion, diagnostics, SIR index)
│   ├── salt-build/       Build orchestrator
│   ├── sp/               Package manager
│   └── runner_qemu.py    Kernel boot test harness
├── tests/                Integration tests (Salt source + Python helpers)
├── benchmarks/           Performance benchmark scripts
├── docs/                 Architecture docs, ADRs, tutorials, ABI specs
├── scripts/              Bootstrap, QEMU, build helpers
└── tools/run_all_tests.py  Full test suite runner
```

Key reference files:
- Kernel architecture: [`kernel/README.md`](kernel/README.md)
- Compiler architecture: [`salt-front/src/codegen/README.md`](salt-front/src/codegen/README.md)
- Standard library: [`std/README.md`](std/README.md)
- Design decisions: [`docs/adr/`](docs/adr/)
- System call ABI: [`docs/abi/`](docs/abi/)

---

## 3. How to Run Tests

```bash
make test                          # compiler unit tests only (fast)
cargo test --lib                   # same, from salt-front/

make lettuce-verify                # compile Lettuce with Z3 verification enabled
make lettuce                       # build + verify Lettuce

make bench                         # run benchmarks (shorter suite)
make bench-long                    # full benchmark suite

tools/run_all_tests.py             # full suite: unit + integration + QEMU boot tests
tools/runner_qemu.py               # kernel smoke tests in QEMU (requires keuos.iso)

cd salt-front && cargo clippy -- -D warnings   # lint check (must pass before PR)
```

Test file locations:
- Compiler unit tests are co-located with source: [`salt-front/src/codegen/tests_*.rs`](salt-front/src/codegen/tests_postcondition.rs)
- Integration tests: [`tests/`](tests/README.md)
- Kernel QEMU tests: [`tools/runner_qemu.py`](tools/runner_qemu.py)

---

## 4. How to Add a Language Feature

The typical file-touch order for a new Salt language feature:

| Step | File(s) | What to do |
|------|---------|------------|
| 1 | `grammar/attr.rs`, `grammar/pattern.rs` | Add AST nodes / attributes for the new syntax |
| 2 | `hir/lower.rs` | Lower new AST nodes to HIR |
| 3 | `hir/typeck.rs` | Add type-checking rules |
| 4 | `codegen/emit_hir.rs` | Emit MLIR operations for the new construct |
| 5 | `codegen/tests_*.rs` | Add compile-and-verify tests |
| 6 | `tests/test_example.salt` | Add integration test |
| 7 | `docs/tutorial/` or `docs/SPEC.md` | Document the feature |

**Working example — iterators:**
- AST / grammar: [`salt-front/src/grammar/pattern.rs`](salt-front/src/grammar/pattern.rs)
- Lowering + codegen: [`salt-front/src/codegen/tests_iterator_protocol.rs`](salt-front/src/codegen/tests_iterator_protocol.rs)
- Integration test: [`tests/test_iterator.salt`](tests/test_iterator.salt)

For each step, add the corresponding test in the same commit. See [`CLAUDE.md`](CLAUDE.md) for the zero-mutant and coverage policies.

---

## 5. How to Add an Optimization Pass

Optimization passes live in [`salt-front/src/codegen/passes/`](salt-front/src/codegen/passes/).

**Pass structure:**

```rust
//! docs: what the pass does, algorithm summary
//! Uses: which analysis results it consumes

pub struct MyPass {
    // per-function state
}

impl MyPass {
    pub fn run(&mut self, ctx: &mut LoweringContext, func: &SaltFn) -> Result<(), String> {
        // 1. Gather required info (call graph, liveness, etc.)
        // 2. Transform the SIR / MLIR
        // 3. Return Ok(()) or Err with diagnostics
    }
}
```

**Test pattern (co-located or in `tests_*.rs`):**

```rust
#[test]
fn test_my_pass_basic() {
    let mlir = compile_to_mlir(r#"
        package main
        fn test_fn(x: i32) -> i32 {
            return x;
        }
    "#);
    assert!(mlir.contains("expected_operation"));
}
```

**Working examples:**
- Liveness analysis: [`salt-front/src/codegen/passes/liveness.rs`](salt-front/src/codegen/passes/liveness.rs)
- Async-to-state-machine transform: [`salt-front/src/codegen/passes/async_to_state.rs`](salt-front/src/codegen/passes/async_to_state.rs)
- Sync verifier (Z3-backed): [`salt-front/src/codegen/passes/sync_verifier.rs`](salt-front/src/codegen/passes/sync_verifier.rs)
- Pulse injection: [`salt-front/src/codegen/passes/pulse_injection.rs`](salt-front/src/codegen/passes/pulse_injection.rs)

Register the new pass in [`salt-front/src/codegen/passes/mod.rs`](salt-front/src/codegen/passes/mod.rs).

---

## 6. How to Add a Z3 Contract

Contracts use `requires` (precondition) and `ensures` (postcondition) clauses on function signatures.

**Syntax:**

```salt
fn safe_divide(a: i32, b: i32) -> i32
    requires(b > 0)
    ensures(result >= 0)
{
    return a / b;
}
```

The verification engine is in [`salt-front/src/codegen/verification/`](salt-front/src/codegen/verification/).

**Files to touch:**

| What | File |
|------|------|
| Contract parsing | `grammar/attr.rs` — `requires` / `ensures` AST |
| Verification engine | `codegen/verification/mod.rs` — `VerificationEngine` |
| Pointer safety | `codegen/verification/ptr_bounds_verifier.rs` |
| Arena safety | `codegen/verification/arena_verifier.rs` |
| Exhaustiveness | `codegen/verification/exhaustiveness.rs` |
| Proof hints | `codegen/verification/proof_hint.rs` |

**Test example:**
[`salt-front/src/codegen/tests_postcondition.rs`](salt-front/src/codegen/tests_postcondition.rs) — see `test_postcondition_basic_absolute_value` for a complete `ensures` test.

Each unsafe operation needs a `requires` clause. The engine uses Weakest Precondition (WP) generation to verify that every execution path satisfies the contract. Violations produce Z3 counterexamples at compile time.

---

## 7. PR Process

1. **Branch:** Fork the repo and create a feature branch. Name it descriptively (e.g., `feat/spmc-channel`, `fix/oom-slab`).

2. **Commit format:**
   ```
   type: short description

   Body explaining motivation and approach. Max 72 chars per line.
   ```
   Types: `feat`, `fix`, `refactor`, `chore`, `docs`, `test`, `bench`.

3. **Before submitting:**
   ```bash
   cargo test                              # all tests pass
   cargo clippy -- -D warnings             # zero warnings
   tools/run_all_tests.py                  # full integration suite (CI gate)
   ```

4. **CI checks** (`.github/workflows/ci.yml`):
   - `cargo build --release`
   - `cargo test --lib`
   - `cargo clippy -- -D warnings`
   - Kernel smoke test (QEMU boot)
   - Benchmark regression gate

5. **Atomic changes:** Kernel ABI changes must also update `docs/abi/KEUOS_ABI.md` in the same PR. Cross-component API changes (e.g., kernel NetD interface) must update both sides atomically.

6. **For small fixes,** no issue is required. For new features or breaking changes, open an issue or start a [GitHub Discussion](https://github.com/bneb/keuos/discussions) first.

---

## 8. Code Standards

These are enforced by pre-commit hooks and CI:

| Rule | Threshold |
|------|-----------|
| Lines per file | Max 500 |
| Non-blank lines per function | Max 32 |
| Indentation nesting | Max 3 levels |
| Mutants (TODO, FIXME, HACK, XXX, temp_, workaround) | Zero in non-test files |
| Clippy | `-D warnings` — zero warnings, zero allows |

**Architecture invariants:**
- `kernel/core/`, `kernel/mem/`, `kernel/sched/` must **never** import `arch::x86_64` directly — use the HAL router (`kernel/arch/`). See [HAL mandate](docs/keuos_driver_model.md).
- Arena-allocated references must never escape their region (enforced by `ArenaVerifier`).
- SPSC shared memory accesses must validate `capacity` and `tail` from untrusted userspace.

**Testing:**
- Every module must have a corresponding test file.
- Every branch in new code must have a test case.
- Public API changes must update the corresponding `docs/` spec files.

**Refactoring policy:**
- Never modify kernel scheduler core logic, interrupt handlers, or page table code during refactoring — extract helpers only.
- Never edit `vendor/`, `isodir/boot/`, or `.claude/hooks/`.
- Never boot QEMU interactively — use `tools/runner_qemu.py`.
- Every extracted function that touches unsafe memory must have a Z3 `requires` clause.
