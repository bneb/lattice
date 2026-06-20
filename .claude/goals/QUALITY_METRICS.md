# Quality Metrics Tracker

Goal: monotonic improvement across all five dimensions each session.

## 2026-06-19 — Session 1: Scheduler Refactoring + Linting Infrastructure

### Files >500 lines (kernel/)
| Metric | Before | After | Delta |
|--------|--------|-------|-------|
| Count | 10 | 10 | 0 |
| Worst file | scheduler.salt (810) | ring_abi.salt (732) | scheduler.salt -271 |

### scheduler.salt specific
| Metric | Before | After |
|--------|--------|-------|
| Lines | 810 | 539 (-33%) |
| Deep-nest blocks (level 4+) | 8 | 0 |
| Functions >32 lines | 6 | 0 |
| Files split out | 0 | 1 (work_steal.salt, 116 lines) |

### Deep nesting (kernel/)
scheduler.salt: 8→0. Other kernel files unchanged.

### Infrastructure added
- `.editorconfig` — cross-editor consistency
- `check-constraints.sh` — blank-line sanity (max 2 consecutive, min 1 between fns)
- `check-constraints.sh` — incremental 500-line enforcement (existing >500 files can shrink)
- `clippy.toml` — cognitive-complexity 30→15, added too-many-arguments=8
- `lib.rs` — warn→deny for cognitive_complexity, added missing_docs + multiple_statements
- `ci.yml` — clippy now blocking (removed continue-on-error)

### Tests
- 1,243 passed, 0 failed (unchanged)
- Kernel build: SUCCESS

## 2026-06-19 — Session 2: T-001 ring_ops extraction from ring_abi.salt

### Files >500 lines (kernel/)
| Metric | Before | After | Delta |
|--------|--------|-------|-------|
| Count | 10 | 9 | -1 |
| Worst file | ring_abi.salt (732) | sparse_set.salt (671) | ring_abi.salt -355 |

### ring_abi.salt specific
| Metric | Before | After |
|--------|--------|-------|
| Lines | 732 | 377 (-48%) |
| Files created | 0 | 1 (ring_ops.salt, 376 lines) |

### Verification
- cargo test: 0 FAILED
- cargo build --release: SUCCESS
- cargo clippy: pre-existing errors (unrelated to .salt changes)

## 2026-06-19 — Session 3: T-002 scheduling_sets extraction from sparse_set.salt

### Files >500 lines (kernel/)
| Metric | Before | After | Delta |
|--------|--------|-------|-------|
| Count | 9 | 8 | -1 |
| Worst file | sparse_set.salt (671) | exec_user.salt (633) | sparse_set.salt -250 |

### sparse_set.salt specific
| Metric | Before | After |
|--------|--------|-------|
| Lines | 671 | 421 (-37%) |
| Files created | 0 | 1 (scheduling_sets.salt, 394 lines) |

Extracted sets: ThreadState, SchedulingPriority, CpuAffinity, EpochInfo, PerfCounters.
Kept delegation wrappers in sparse_set.salt so existing callers (ecs_scheduler,
ecs_epoch, ecs_ipc, ecs_bridge, commands, world) need no changes.

### Verification
- cargo test: 0 FAILED
- cargo build --release: SUCCESS

## 2026-06-19 — Session 4: T-003 spawn_coroutine extraction from exec_user.salt

### Files >500 lines (kernel/)
| Metric | Before | After | Delta |
|--------|--------|-------|-------|
| Count | 8 | 7 | -1 |
| Worst file | exec_user.salt (633) | ring3_test.salt (609) | exec_user.salt -133 |

### exec_user.salt specific
| Metric | Before | After |
|--------|--------|-------|
| Lines | 633 | 500 (-21%) |
| Files created | 0 | 1 (spawn_coroutine.salt, 167 lines) |

Extracted: spawn_ring3_coroutine, setup_kernel_stack, map_kernel_stack.
exec_user.salt retains: spawn_process, spawn_kernel_thread, spawn_process_from_inode,
and their @no_mangle wrappers. Removed 4 unused FFI declarations.

### Verification
- cargo test: 0 FAILED
- cargo build --release: SUCCESS

## 2026-06-19 — Session 5: T-004 KPTI test extraction from ring3_test.salt

### Files >500 lines (kernel/)
| Metric | Before | After | Delta |
|--------|--------|-------|-------|
| Count | 7 | 6 | -1 |
| Worst file | ring3_test.salt (609) | syscall.salt (582) | ring3_test.salt -296 |

### ring3_test.salt specific
| Metric | Before | After |
|--------|--------|-------|
| Lines | 609 | 313 (-49%) |
| Files created | 0 | 1 (ring3_kpti_test.salt, 281 lines) |

Extracted: test_ring3_kpti, test_pcid_allocation, test_pcid_cr3_noflush,
test_kpti_user_pml4_isolated + their FFI declarations.
ring3_test.salt retains: test_ring3_iretq_frame, test_ring3_e2e, test_swapgs_ring3.

### Verification
- cargo test: 0 FAILED
- cargo build --release: SUCCESS

## 2026-06-19 — Session 6: T-005 paging_destroy extraction from user_paging.salt

### Files >500 lines (kernel/)
| Metric | Before | After | Delta |
|--------|--------|-------|-------|
| Count | 6 | 5 | -1 |
| Worst file | user_paging.salt (527) | syscall.salt (582) | user_paging.salt -106 |

### user_paging.salt specific
| Metric | Before | After |
|--------|--------|-------|
| Lines | 527 | 421 (-20%) |
| Files created | 0 | 1 (paging_destroy.salt, 141 lines) |

Extracted: destroy_user_pml4, destroy_pdp, destroy_pd, destroy_pt,
unmap_user_page into paging_destroy.salt. Delegation wrappers keep
existing callers (syscall.salt, syscall_ipc.salt, exec_user.salt,
capability.salt) working without changes.

### Verification
- cargo test: 0 FAILED
- cargo build --release: SUCCESS

### Next session targets (Tier 1 priority order)
1. kernel/benchmarks/netd_bench.salt (570 lines)
2. kernel/core/process.salt (520 lines)
3. kernel/core/preempt_test.salt (509 lines)
