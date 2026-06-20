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

## Quality Goals (Aspirational — work toward these incrementally)

We are pursuing five quality goals. These are NOT hard blockers on new code
(the hook-enforced constraints in `.claude/hooks/check-constraints.sh` already
handle that). These are long-term targets to reach through systematic,
incremental refactoring.

### The Five Goals
1. Every file < 500 LOC
2. Every function < 32 non-blank LOC
3. Zero blocks at nesting level 4+ (no `if`/`match`/`while`/`for`/`loop`
   at 16+ spaces of indent)
4. Test coverage > 95% (salt-front + salt-lsp)
5. Zero mutant markers (TODO/FIXME/HACK/XXX/temp_/workaround) in source

### Strategy: Ratchet, Don't Boil the Ocean
- Each session, pick ONE file or module and improve it against ALL five goals.
- Never make coverage worse. Never make a function longer. Never add nesting.
- If you touch a function to fix something else, leave it shorter/flatter.
- Commit after each successful improvement with a metric in the message:
  "refactor: shrink do_dispatch 98→30 lines, scheduler.salt 810→492 lines"

### Priority Queue (work top-to-bottom)

**Tier 1 — Kernel (highest impact, smallest files):**
1. `kernel/core/scheduler.salt` (810 lines, 8 deep-nest blocks, functions
   up to 98 lines) — split into sched_dispatch, sched_spawn, sched_steal
2. `kernel/core/ring_abi.salt` (732 lines) — split by operation family
   (ring_init, ring_transfer, ring_spawn/mmap)
3. `kernel/core/exec_user.salt` (633 lines, functions up to 161 lines) —
   extract spawn variants into separate helpers
4. `kernel/core/ring3_test.salt` (609 lines) — split by test scenario
5. `kernel/core/syscall.salt` (582 lines) — extract I/O syscalls
   (sys_write/read/open/spawn/exit) to syscall_io.salt (blocked: u64↔Ptr<T>
   cast limitation — track this, revisit when compiler supports it)
6. `kernel/benchmarks/netd_bench.salt` (570 lines) — split by test case
7. `kernel/mem/user_paging.salt` (527 lines) — extract walk/create/unmap
8. `kernel/core/process.salt` (520 lines) — extract PID alloc/free helpers
9. `kernel/core/preempt_test.salt` (509 lines) — split by test layer
10. `kernel/ecs/sparse_set.salt` (671 lines) — split sparse set operations

**Tier 2 — Salt compiler (large files, architectural risk):**
Focus on the worst functions first, not whole-file splits:
- `context.rs` `emit_verify` (368 lines) — extract verification phases
- `type_bridge.rs` `drain_work_queue` (325 lines) — extract queue stages
- `type_bridge.rs` `request_specialization` (171 lines) — split by case
- `stmt.rs` `emit_salt_if` (157 lines) — extract branch emission helpers
- `stmt.rs` `emit_iterator_for_loop` (148 lines) — extract loop patterns
- `expr/resolver.rs` `identify_target` (174 lines) — split by target kind

**Tier 3 — Coverage gaps:**
- `interpreter.rs` (0 tests) — add smoke tests for eval_expr, exec_stmt
- `fuzz_ast.rs` (0 tests) — verify round-trip for basic AST nodes
- `grammar/pattern.rs` (0 dedicated tests) — test each pattern variant
- `codegen/verification/` modules — add edge-case Z3 contract tests
- salt-lsp: `completion.rs`, `sir_display.rs`, `sir_index.rs` — add unit tests

**Tier 4 — Fuzz targets (new coverage tool):**
- `salt-front/fuzz/fuzz_parser.rs` — round-trip fuzzing
- `salt-front/fuzz/fuzz_preprocess.rs` — preprocessor stress

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
