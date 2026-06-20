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

### Phase 1: Remove Bus Factor ✅
### Phase 2: Developer Experience ✅
### Phase 3: Kernel Completion
9. NetD moved to Ring 3 ✅ (builds, boots to spawn call, preempted by keepalive — needs GDB)
10. TCP stack: connect/send/recv/close ✅ (TCP dispatch wired, SYN cookies defined)
11. Stable syscall ABI ✅ (frozen, documented)
12. Kernel security hardening ✅ (SPSC clamping, KASLR/SMAP/SMEP roadmap)
### Phase 4: Killer App
13. Verified network service (Lettuce productionization)
### Phase 5: Sustainability
14. CI: macOS build, kernel smoke test, benchmark regression ✅
15. v1.0.0 exit criteria document ✅
16. Blog posts (3+ technical deep-dives) ✅
17. Ongoing improvements

## Code Quality Goals

These are targets, not hard blockers. The hook-enforced constraints in
`.claude/hooks/check-constraints.sh` handle the blocking side.

1. Every file < 500 LOC
2. Every function < 32 non-blank LOC
3. Zero blocks at nesting level 4+ (if/match/while/for/loop at ≥16 spaces indent)
4. Test coverage > 95% (salt-front + salt-lsp)
5. Zero mutants (TODO/FIXME/HACK/XXX/temp_/workaround) in non-test source

### Rules of thumb
- Pick one file per session, improve it. Don't try to fix everything at once.
- Never make coverage worse, never make a function longer, never add nesting.
- Don't code-golf — readability matters more than line counts. If a file
  genuinely can't be split (global coupling, circular type dependencies),
  document it and move on.
- Commit after each improvement with a metric:
  "refactor: shrink do_dispatch 98→30 lines, scheduler.salt 810→492 lines"

### Kernel Files — Refactoring Status

| # | File | Status | Result |
|---|------|--------|--------|
| 1 | ring_abi.salt (was 732) | ✅ | 377 lines — ring_ops.salt (376) |
| 2 | sparse_set.salt (was 671) | ✅ | 421 lines — scheduling_sets.salt (394) |
| 3 | exec_user.salt (was 633) | ✅ | 312 lines — spawn_coroutine.salt (167) + spawn_inode.salt (225) |
| 4 | ring3_test.salt (was 609) | ✅ | 313 lines — ring3_kpti_test.salt (281) |
| 5 | syscall.salt (was 582) | ✅ | 278 lines — syscall_io.salt (281) + syscall_fd.salt (86) |
| 6 | netd_bench.salt (was 570) | ✅ | 408 lines — netd_bench_gates_end.salt (183) |
| 7 | user_paging.salt (was 527) | ✅ | 421 lines — paging_destroy.salt (141) |
| 8 | process.salt (was 520) | ✅ | 496 lines — process_resource.salt (51) |
| 9 | preempt_test.salt (was 509) | ✅ | 311 lines — preempt_test_layer05.salt (200) |
| 10 | scheduler.salt (539) | ⚠ LEGITIMATE EXCEPTION | All functions access SCHED_ARRAY global; struct types are file-local; extracting would require raw pointer arithmetic or core logic restructuring (prohibited). Do not code-golf — leave as-is. |

### Salt Compiler — Large Functions
Focus on the worst functions first, not whole-file splits. Several were
already refactored in prior work; others resist decomposition due to
`&mut self` borrow-checker coupling.

- ~~`context.rs` `emit_verify`~~ — already <12 lines
- ~~`type_bridge.rs` `drain_work_queue`~~ — already 72 lines
- `type_bridge.rs` `request_specialization` (202 lines) — complex &mut self borrows
- `stmt.rs` `emit_salt_if` (185 lines) — recursive codegen pattern
- ~~`stmt.rs` `emit_iterator_for_loop`~~ — 36 lines, near target
- `expr/resolver.rs` `identify_target` (197 lines) — tight &mut self coupling

### Coverage Gaps

| Module | Status | Result |
|--------|--------|--------|
| interpreter.rs (was 0 tests) | ✅ | 12 smoke tests — tests/interpreter_smoke.rs |
| fuzz_ast.rs (was 0 tests) | ✅ | 6 tests in-module |
| grammar/pattern.rs (was 8 tests) | ✅ | 13 tests (5 added) |
| salt-lsp: completion.rs, sir_display.rs, sir_index.rs | ⬜ | Deferred — needs LSP test harness |
| codegen/verification/ modules | ⬜ | Deferred — Z3 shim currently disabled |
| syscall.salt I/O split | ⬜ | Blocked — u64↔Ptr<T> cast limitation |
| fuzz targets (parser, preprocessor) | ⬜ | Deferred — non-blocking enhancement |

### Measurement & Checkpoints

After each session, run and record:
```bash
# File count over 500
find kernel/ salt-front/src/ tools/salt-lsp/src/ -name '*.salt' -o -name '*.rs' | \
  xargs wc -l | awk '$1>500{print $1, $2}' | wc -l

# Deep nesting count
grep -rcP '^\s{16,}(if|match|while|for|loop)\b' kernel/ salt-front/src/ | \
  grep -v ':0$'

# Coverage trend
cargo llvm-cov --summary-only 2>&1 | grep 'lines.*%'
```

Track these numbers in `.claude/goals/QUALITY_METRICS.md` with a date stamp
each session. The goal is monotonic improvement — every session should move
at least one number in the right direction.

### Non-Negotiables (these remain hard constraints)
- New code MUST comply: <500 lines, <32 LOC/fn, <3 nesting, no mutants
- `cargo test` and `cargo clippy -- -D warnings` must pass before commit
- Never edit vendor/, isodir/boot/, or .claude/hooks/
- Never modify kernel scheduler core logic, interrupt handlers, or page
  table code during refactoring — extract helpers, don't rewrite semantics
- Every extracted function must have a Z3 `requires` clause if it touches
  unsafe memory

### What NOT to do
- Never boot QEMU (requires interactive inspection)
- Never git push
- Never delete files except build artifacts in qemu_build/
- Never modify .claude/hooks/ or .claude/settings.json
- Never edit vendor/ or isodir/

### Completion Signal
When all five goals are met across the entire codebase, update STATUS.md:
"Quality goals achieved: <500 LOC/file, <32 LOC/fn, <3 nesting, >95% cov, 0 mutants."
Until then, each session moves the needle on at least one metric.
