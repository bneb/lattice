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

### Next session targets (Tier 1 priority order)
1. kernel/ecs/sparse_set.salt (671 lines)
2. kernel/core/exec_user.salt (633 lines)
3. kernel/core/ring3_test.salt (609 lines)
