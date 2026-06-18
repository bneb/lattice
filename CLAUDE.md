# Salt + KeuOS — Quality Standards & Six-Month Plan

## Build
- `cargo build --release` in salt-front/ for the compiler
- `cargo test` in salt-front/ for unit tests
- `tools/runner_qemu.py` for kernel boot tests
- `tools/run_all_tests.py` for full test suite

## Hard Constraints (enforced by .claude/hooks/check-constraints.sh)
- **Max 32 non-blank lines per function.** Violations are blocked on Write/Edit.
- **Max 500 lines per file.** Violations are blocked on Write/Edit.
- **Max 3 levels of indentation nesting.** Violations are blocked on Write/Edit.
- **No mutants.** TODO, FIXME, HACK, XXX, temp_, workaround in non-test files are blocked. Open a GitHub issue instead.
- Every module must have a corresponding test file.
- Never edit vendor/ or isodir/boot/ — those are dependencies and build outputs.

## Work Standards
- Before declaring any task done: `cargo test` passes, `cargo clippy -- -D warnings` is clean.
- No untested code paths. If you add a branch, add a test case.
- Public API changes must update the corresponding docs/ spec files in the same commit.
- Kernel changes touching the ABI must update `docs/abi/KEUOS_ABI.md` atomically.

## Architecture Invariants
- `kernel/core/`, `kernel/mem/`, `kernel/sched/` never import `arch::x86_64` directly — use the HAL router (`kernel/arch/mod.salt`).
- Arena-allocated references must never escape their region (enforced by ArenaVerifier).
- Z3 contracts on public functions are non-negotiable — every unsafe operation needs a `requires` clause.
- SPSC shared memory accesses must validate `capacity` and `tail` from untrusted userspace.

## The Plan

We are methodically executing a 17-point strategy across 5 phases.
Progress is tracked in `.claude/goals/STATUS.md` — read that file to see what's done and what's next.

### Phase 1: Remove Bus Factor
1. Eliminate all hardcoded developer paths
2. Write architecture decision records (15 ADRs)
3. One-command developer setup (`make setup`)
4. Contributor ladder (labels, CODEOWNERS, good-first-issue)

### Phase 2: Developer Experience
5. "Salt by Example" tutorial (8+ chapters)
6. Standard library API documentation
7. LSP: semantic tokens, Z3 diagnostics, code actions, references, rename
8. Package manager: version resolution, registry, direct compiler invocation

### Phase 3: Kernel Completion
9. NetD moved to Ring 3
10. TCP stack: connect/send/recv/close
11. Stable syscall ABI (frozen numbers, layouts, error codes)
12. Kernel security hardening (KASLR, SMAP, SMEP, SPSC clamping)

### Phase 4: Killer App
13. Verified network service (Lettuce productionization)

### Phase 5: Sustainability
14. CI: macOS build, kernel smoke test, benchmark regression
15. v1.0.0 exit criteria document
16. Blog posts (3+ technical deep-dives)
17. Continue maintenance loop for ongoing improvements
