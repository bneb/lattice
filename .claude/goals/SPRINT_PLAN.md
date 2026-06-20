# Sprint Plan — Quality Goals

Goal: every kernel `.salt` file < 500 lines, then worst salt-front functions < 32 LOC.
Each task = one thematic extraction. One task per session. Work top-to-bottom.

## Sprint 1: Kernel Files to < 500 Lines (10 tasks)

### [x] T-001: Extract ring_ops from ring_abi.salt
- **Target:** `kernel/core/ring_abi.salt` (732 → ~450 lines)
- **Strategy:** Extract `ring_write`, `ring_brk`, `ring_mmap`, `ring_ipc_send`, `ring_shm_grant`, `ring_spawn` into `kernel/core/ring_ops.salt`
- **Acceptance:** `ring_ops.salt` exists, `ring_abi.salt` < 500 lines, `cargo test` passes, clippy clean
- **Estimate:** 1 session

### [x] T-002: Extract scheduling sets from sparse_set.salt
- **Target:** `kernel/ecs/sparse_set.salt` (671 → ~370 lines)
- **Strategy:** Extract thread, priority, affinity, epoch_info, and perf_counters sets into `kernel/ecs/scheduling_sets.salt`. Leave ipc_cap, socket, and memmap sets in sparse_set.salt.
- **Acceptance:** `scheduling_sets.salt` exists, `sparse_set.salt` < 500 lines, `cargo test` passes, clippy clean
- **Estimate:** 1 session

### [x] T-003: Extract spawn_coroutine from exec_user.salt
- **Target:** `kernel/core/exec_user.salt` (633 → ~440 lines)
- **Strategy:** Extract `spawn_ring3_coroutine`, `setup_kernel_stack`, `map_kernel_stack`, `exec_spawn_ring3_coroutine` into `kernel/core/spawn_coroutine.salt`
- **Acceptance:** `spawn_coroutine.salt` exists, `exec_user.salt` < 500 lines, `cargo test` passes, clippy clean
- **Estimate:** 1 session

### [x] T-004: Extract KPTI tests from ring3_test.salt
- **Target:** `kernel/core/ring3_test.salt` (609 → ~370 lines)
- **Strategy:** Extract `test_ring3_kpti`, `test_pcid_allocation`, `test_pcid_cr3_noflush`, `test_kpti_user_pml4_isolated` into `kernel/core/ring3_kpti_test.salt`
- **Acceptance:** `ring3_kpti_test.salt` exists, `ring3_test.salt` < 500 lines, `cargo test` passes, clippy clean
- **Estimate:** 1 session

### [x] T-005: Extract paging_destroy from user_paging.salt
- **Target:** `kernel/mem/user_paging.salt` (527 → ~410 lines)
- **Strategy:** Extract `destroy_user_pml4`, `destroy_pdp`, `destroy_pd`, `destroy_pt`, `user_paging_destroy_user_pml4`, `user_paging_unmap_user_page`, `unmap_user_page` into `kernel/mem/paging_destroy.salt`
- **Acceptance:** `paging_destroy.salt` exists, `user_paging.salt` < 500 lines, `cargo test` passes, clippy clean
- **Estimate:** 1 session

### [x] T-006: Extract gates_10_to_18 from netd_bench.salt
- **Target:** `kernel/benchmarks/netd_bench.salt` (570 → ~405 lines)
- **Strategy:** Extract `test_gates_10_to_18` (largest test, ~167 lines) into `kernel/benchmarks/netd_bench_gates_end.salt`
- **Acceptance:** `netd_bench_gates_end.salt` exists, `netd_bench.salt` < 500 lines, `cargo test` passes, clippy clean
- **Estimate:** 1 session

### [x] T-007: Extract PID/stack alloc from process.salt
- **Target:** `kernel/core/process.salt` (520 → ~420 lines)
- **Strategy:** Extract `alloc_pid`, `alloc_kernel_stack`, `sys_alloc_kernel_stack`, `free_kernel_stack`, `init_slot` into `kernel/core/process_resource.salt`
- **Acceptance:** `process_resource.salt` exists, `process.salt` < 500 lines, `cargo test` passes, clippy clean
- **Estimate:** 1 session

### [x] T-008: Extract layer_05 tests from preempt_test.salt
- **Target:** `kernel/core/preempt_test.salt` (509 → ~360 lines)
- **Strategy:** Extract `test_layer_05`, `test_layer_05a`, `test_layer_05b` into `kernel/core/preempt_test_layer05.salt`
- **Acceptance:** `preempt_test_layer05.salt` exists, `preempt_test.salt` < 500 lines, `cargo test` passes, clippy clean
- **Estimate:** 1 session

### T-009: Extract bitmap ops from scheduler.salt
- **Target:** `kernel/core/scheduler.salt` (539 → ~390 lines)
- **Strategy:** Extract `bitmap_set`, `bitmap_clear`, `mask_above_bit`, `mask_below_bit`, `find_free_slot`, `find_next_fiber` into `kernel/core/sched_bitmap.salt`
- **Acceptance:** `sched_bitmap.salt` exists, `scheduler.salt` < 500 lines, `cargo test` passes, clippy clean
- **Estimate:** 1 session

### T-010: Extract I/O syscalls from syscall.salt (BLOCKED)
- **Target:** `kernel/core/syscall.salt` (582 → ~400 lines)
- **Strategy:** Extract `sys_write`, `sys_read`, `sys_open`, `copy_from_user`, `copy_to_user` into `kernel/core/syscall_io.salt`
- **Blocker:** u64↔Ptr<T> cast limitation in Salt compiler prevents cross-package function references for these signatures
- **Action:** Track compiler progress. Do not attempt until blocker resolved.
- **Estimate:** 1 session (once unblocked)

## Sprint 2: Salt-Front Worst Functions (6 tasks)

For these, extract one mega-function per session — don't split the whole file.

### T-011: Extract verification phases from context.rs emit_verify
- **Target:** `salt-front/src/codegen/context.rs` `emit_verify` (368 lines → target < 100)
- **Strategy:** Identify 4-5 distinct phases within emit_verify. Extract each into a private helper function in the same file. Goal: emit_verify becomes a 10-20 line orchestrator calling named phases.
- **Acceptance:** `emit_verify` < 100 lines, no new deep nesting, `cargo test` passes, clippy clean
- **Estimate:** 1 session

### T-012: Extract queue stages from type_bridge.rs drain_work_queue
- **Target:** `salt-front/src/codegen/type_bridge.rs` `drain_work_queue` (325 lines → target < 100)
- **Strategy:** Identify distinct queue processing stages. Extract each into a private helper. drain_work_queue becomes a loop calling helpers.
- **Acceptance:** `drain_work_queue` < 100 lines, no new deep nesting, `cargo test` passes, clippy clean
- **Estimate:** 1 session

### T-013: Split request_specialization by case
- **Target:** `salt-front/src/codegen/type_bridge.rs` `request_specialization` (171 lines → target < 50)
- **Strategy:** Extract each major match arm body into a named helper function. request_specialization becomes a match dispatcher.
- **Acceptance:** `request_specialization` < 50 lines, no new deep nesting, `cargo test` passes, clippy clean
- **Estimate:** 1 session

### T-014: Extract branch helpers from stmt.rs emit_salt_if
- **Target:** `salt-front/src/codegen/stmt.rs` `emit_salt_if` (157 lines → target < 50)
- **Strategy:** Extract then-block emission, else-block emission, and condition evaluation into helpers.
- **Acceptance:** `emit_salt_if` < 50 lines, no new deep nesting, `cargo test` passes, clippy clean
- **Estimate:** 1 session

### T-015: Extract loop patterns from stmt.rs emit_iterator_for_loop
- **Target:** `salt-front/src/codegen/stmt.rs` `emit_iterator_for_loop` (148 lines → target < 50)
- **Strategy:** Extract iteration setup, body emission, and teardown into helpers.
- **Acceptance:** `emit_iterator_for_loop` < 50 lines, no new deep nesting, `cargo test` passes, clippy clean
- **Estimate:** 1 session

### T-016: Split identify_target by target kind
- **Target:** `salt-front/src/codegen/expr/resolver.rs` `identify_target` (174 lines → target < 50)
- **Strategy:** Extract each target-kind resolution (local, upvalue, global, field, method) into a named helper.
- **Acceptance:** `identify_target` < 50 lines, no new deep nesting, `cargo test` passes, clippy clean
- **Estimate:** 1 session

## Sprint 3: Coverage Gaps (5 tasks)

### T-017: Add smoke tests for interpreter.rs
- **Target:** `salt-front/src/interpreter.rs` (0 tests → target > 60% coverage)
- **Strategy:** Add tests for `eval_expr` and `exec_stmt` with basic types (integers, bools, simple control flow). Create `salt-front/tests/interpreter_smoke.rs` if no inline test module.
- **Acceptance:** > 5 new test functions, all pass, coverage of interpreter.rs > 60%
- **Estimate:** 1 session

### T-018: Add round-trip tests for fuzz_ast.rs
- **Target:** `salt-front/src/fuzz_ast.rs` (0 tests → target > 60% coverage)
- **Strategy:** Add round-trip tests: parse → serialize → parse, verify AST equivalence for basic node types.
- **Acceptance:** > 3 new test functions, all pass, coverage of fuzz_ast.rs > 60%
- **Estimate:** 1 session

### T-019: Add pattern variant tests for grammar/pattern.rs
- **Target:** `salt-front/src/grammar/pattern.rs` (0 dedicated tests)
- **Strategy:** Test each pattern variant: literal, variable, wildcard, struct destructure, tuple destructure, or-pattern.
- **Acceptance:** > 5 new test functions, all pass, coverage of pattern.rs > 60%
- **Estimate:** 1 session

### T-020: Add unit tests for salt-lsp modules
- **Target:** `tools/salt-lsp/src/completion.rs`, `sir_display.rs`, `sir_index.rs`
- **Strategy:** Add tests for completion candidate generation, SIR display formatting, and index queries.
- **Acceptance:** > 3 new test functions per module, all pass
- **Estimate:** 1 session

### T-021: Add edge-case Z3 contract tests
- **Target:** `salt-front/src/codegen/verification/` modules
- **Strategy:** Add tests for contract edge cases: empty preconditions, contradictory postconditions, nested function calls.
- **Acceptance:** > 3 new test functions, all pass
- **Estimate:** 1 session

## Sprint 4: Final Sweep (3 tasks)

### T-022: Eliminate remaining mutant markers
- **Target:** All mutant markers in non-test source (currently ~2 sites)
- **Strategy:** Replace each marker with a permanent fix: TODO → GitHub issue, FIXME → constraint explanation, XXX → proper note, temp_ → rename.
- **Acceptance:** `grep -rPn '\b(TODO|FIXME|HACK|XXX|temp_|workaround)\b' kernel/ salt-front/src/ tools/salt-lsp/src/ --include='*.salt' --include='*.rs' | grep -v '_test\.' | grep -v '/tests/' | grep -v '/fuzz/'` returns empty
- **Estimate:** 1 session

### T-023: Verify all kernel files < 500 lines
- **Target:** All kernel `.salt` files
- **Strategy:** Run diagnostic, verify 0 kernel files over 500 lines. If any remain, split them.
- **Acceptance:** `find kernel/ -name '*.salt' | xargs wc -l | awk '$1>500' | wc -l` returns 0
- **Estimate:** 1 session

### T-024: Final metrics snapshot and STATUS.md update
- **Target:** All five goals measured and recorded
- **Strategy:** Run all diagnostic commands, record final numbers in QUALITY_METRICS.md, update STATUS.md with completion status.
- **Acceptance:** QUALITY_METRICS.md updated with final measurements. STATUS.md updated.
- **Estimate:** 1 session

---

## Execution Protocol

Each session:
1. Read this file, find first unchecked task
2. Read the target file(s) to understand current structure
3. Execute the extraction/refactoring
4. Verify: `cargo test`, `cargo clippy -- -D warnings`
5. Record before/after delta in QUALITY_METRICS.md
6. Mark task `[x]` in this file
7. Commit with message: `refactor: T-00X description (metric delta)`

If a task fails due to unexpected coupling:
- Document the blocker inline in this file
- Skip to the next unblocked task
- Do NOT force a split that breaks the build

### Split Pattern (Kernel .salt files)

```salt
// In new file: kernel/core/ring_ops.salt
use kernel::arch;
use kernel::mem;

extern fn post_cqe(slot: u64, user_data: u64, result: i64, flags: u64);

pub fn ring_write(slot: u64, fd: u64, buf: u64, len: u64) -> i64 {
    // ... implementation
}

// If called from other modules, add @no_mangle wrapper in parent:
// In ring_abi.salt:
#[no_mangle]
pub fn ring_abi_ring_write(slot: u64, fd: u64, buf: u64, len: u64) -> i64 {
    return ring_ops::ring_write(slot, fd, buf, len);
}
```

### Split Pattern (Rust files)

```rust
// Extract a helper, keep it private in the same file:
fn emit_verify_phase_contracts(&mut self, ...) -> Result<()> {
    // ... extracted logic
}

// Original function becomes thin orchestrator:
fn emit_verify(&mut self, ...) -> Result<()> {
    self.emit_verify_phase_contracts(...)?;
    self.emit_verify_phase_bounds(...)?;
    self.emit_verify_phase_witnesses(...)?;
    Ok(())
}
```
