# Master Sprint Plan — From Cleanup to Complete

## Phase 1: Clippy Zero (1-2 sessions)
Goal: `cargo clippy -- -D warnings` passes with zero errors.

### [x] S1-001: Safe auto-fix batch (533→126, -407 warnings, 0 test failures)

### S1-002: Manual clippy fix pass
- Fix remaining 129 warnings manually
- Priority: too_many_arguments (21), map_or (24), empty else (12), useless format (9)
- For `compile_ast` (11 args): introduce a `CompileOptions` struct
- Acceptance: `cargo clippy -- -D warnings` returns zero

## Phase 2: Remaining Housekeeping (2-3 sessions)
Goal: Every file that had AI tags gets a full line-by-line read.

### S2-001: Deep-read context.rs (4901 lines)
- Was the second-most tagged file (8 tags)
- Check for: disabled Z3 block, over-explaining, "we" voice, remaining AI patterns
- Acceptance: clean read, no regressions

### S2-002: Deep-read stmt.rs (3389 lines)
- Had V25.8 / CONSTITUTIONAL GUARD — deepest hallucination level
- Check for: remaining ALL-CAPS, hallucinated feature descriptions, "we" voice
- Acceptance: clean read, no regressions

### S2-003: Deep-read mod.rs (2793 lines)
- Had COUNCIL tags
- Check for: remaining AI patterns
- Acceptance: clean read, no regressions

### S2-004: Audit user/basalt/ and user/facet/
- basalt: 5 files, imported by lettuce — strip AI headers, verify code is real
- facet: ~12 Salt files, real C backing — check comments
- Scan remaining 100 user/ files for AI signatures
- Acceptance: zero AI contamination in user/

## Phase 3: Scheduler Refactoring (2-3 sessions)
Goal: Last kernel file >500 lines brought under 500.

### S3-001: Analyze scheduler coupling
- Map every function → SCHED_ARRAY access pattern
- Identify extractable pure functions vs. coupled functions
- Document the specific coupling that blocks extraction
- Acceptance: clear map of what can/can't move

### S3-002: Extract extractable functions
- Move any pure functions to sched_helpers.salt or sched_bitmap.salt
- Keep core dispatch, spawn, and bitmap functions in scheduler.salt
- Acceptance: scheduler.salt < 500 lines OR documented exception updated

### S3-003: Verify and document
- Build + test verification
- Update CLAUDE.md and STATUS.md with final state
- Acceptance: all kernel files <500 OR exceptions documented

## Phase 4: Back to Building (ongoing)
Goal: Resume feature development on a clean foundation.

### S4-001: Lettuce productionization (Phase 4, Goal 13)
- Verify the end-to-end HTTP demo still works
- Clean up any remaining AI contamination in lettuce/
- Acceptance: verified HTTP demo passes

### S4-002: Coverage gaps
- salt-lsp tests (deferred — needs test harness)
- Fuzz targets: fuzz_parser.rs, fuzz_preprocess.rs
- Acceptance: coverage > baseline

### S4-003: Documentation
- Update docs/ with current architecture
- Verify deep-dive blog posts are accurate
- Acceptance: docs match current code

## Risks and Dependencies

| Risk | Impact | Mitigation |
|------|--------|-----------|
| Clippy `--broken-code` breaks more than expected | Blocks S1 | Apply fix categories incrementally, not all at once |
| Scheduler refactoring hits SCHED_ARRAY coupling | Blocks S3 | Accept as documented exception, don't force |
| Deep reads find more contamination than expected | Extends S2 | Prioritize by file size, skip low-risk files |
| Lettuce demo doesn't build after cleanup | Blocks S4 | Test early in S4-001, fix any broken imports |
