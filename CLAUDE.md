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

## Autonomous Operating Mode

When running autonomously (via /loop), follow these instructions.

### Priority Order
Work through remaining items in this order. Only advance to the next item
when the current one is verifiably complete or blocked on kernel boot testing.

1. **Add @no_mangle wrappers** for remaining kernel extern symbols
   (syscall_configure_msrs, pcid_init, ist_install_gates, run_async_fiber_tests,
   run_vfs_tests, run_preemptive_tests, nvme_init). Each wrapper goes at the
   bottom of the defining module in the "// @no_mangle ABI Wrappers" section.
   Verify with: `python3 tools/runner_qemu.py build` reports BUILD SUCCESS.

2. **Implement sys_ipc_reg_send (syscall 14)** — currently returns ENOSYS.
   This is the fast-path register IPC that NetD needs for Ring 3 operation.
   Define the function in kernel/ipc/fastpath.salt or kernel/core/syscall.salt.
   Wire it into the syscall dispatch table. No kernel boot test required;
   verify with `cargo check` + kernel link.

3. **Z3 contract regression tests** — create salt-front/tests/z3_contracts/
   with .salt files that encode expected verification results:
   - `test_contract_proved.salt`: requires(x > 0) with concrete x=5 → Z3 must prove
   - `test_contract_rejected.salt`: requires(x > 0) with x=0 → Z3 must reject  
   - `test_contract_timeout.salt`: complex contract → Z3 must time out at 100ms
   Run with: `./salt-front/target/release/salt-front --verify <file>`
   Document each expected result.

4. **VS Code extension** — update tools/salt-lsp/editors/vscode/ with:
   - Package.json version bump to 0.3.0
   - Updated syntax grammar for new LSP features (semantic tokens)
   - README with install instructions for the .vsix

5. **Code coverage** — add `cargo tarpaulin` or `cargo llvm-cov` to CI
   for salt-front and salt-lsp. Set baseline coverage percentage.

6. **Lettuce AOF persistence** — lettuce/aof.salt has an append-only file stub.
   Implement write-ahead logging with arena-allocated buffers and Z3-verified
   bounds on every write.

7. **Salt compiler warning cleanup** — salt-front has 3 pre-existing warnings.
   Fix them: unused_mut in cli.rs, dead_code in method_resolution.rs,
   unreachable_patterns in special_methods.rs.

8. **Fuzz targets** — salt-front has libfuzzer-sys as a dev dependency.
   Create fuzz targets for the parser (salt-front/fuzz/fuzz_parser.rs) and
   the preprocessor (fuzz_preprocess.rs).

### What NOT to do autonomously
- Never modify kernel core scheduler, interrupt handlers, or page table code
- Never boot QEMU (requires interactive inspection)
- Never git push
- Never delete files except build artifacts in qemu_build/
- Never modify .claude/hooks/ or .claude/settings.json
- Never edit vendor/ or isodir/

### Completion signal
When ALL items are done or blocked, update STATUS.md and report:
"Autonomous work complete. Remaining: [list blocked items and why]."

### Between sessions
On each /loop iteration:
1. Open `.claude/goals/STATUS.md`
2. Find the first unchecked item in the Priority Order above
3. Work on it
4. When done (compiles, tests pass), check it off and commit
5. If blocked, note why and move to next item
