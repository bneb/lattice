# Red Team Review — Master Sprint Plan

## Finding 1: Clippy --broken-code semantic risk [HIGH]

The `--broken-code` flag allows clippy to make changes that "might break compilation." But the real risk is subtler: changes that COMPILE but CHANGE BEHAVIOR.

Example: `redundant_closure` converts `|x| foo(x)` to `foo`. If `foo` is a function pointer vs. a function item, or if there are trait resolution ambiguities, this can change which `foo` is called.

**Recommendation:** Apply fixes in batches by lint category, running `cargo test` between each batch. Don't apply all 417 fixes at once.

## Finding 2: S1-002 'CompileOptions struct' is a breaking API change [HIGH]

The `compile_ast` function has 11 arguments and is likely called from `driver.rs`, `lib.rs`, and possibly the LSP. Adding a `CompileOptions` struct changes the public API of the compiler. This could break external consumers.

**Recommendation:** Check all call sites before refactoring. Consider a builder pattern or default implementation to minimize breakage.

## Finding 3: Deep reads are unbounded [MEDIUM]

"Read 4901 lines of context.rs" has no defined completion criteria. If the file is heavily contaminated, this could take many sessions with no clear endpoint.

**Recommendation:** Use sampling. Read the doc comments and section headers (the most visible content). Spot-check function bodies at random. Define completion as "no AI patterns found in sampled regions" rather than "every line read."

## Finding 4: Scheduler refactoring has a 3-attempt failure history [MEDIUM]

Three independent attempts to extract functions from scheduler.salt failed due to SCHED_ARRAY coupling. The plan assumes attempt #4 will succeed.

**Recommendation:** Time-box the attempt. If 1 session of analysis doesn't reveal a clean extraction path, accept the documented exception and move on. Don't loop.

## Finding 5: Lettuce may have broken imports from deleted files [MEDIUM]

We deleted 13 files from user/netd/ and 6 from user/netd/crypto/. We verified zero direct callers at the time, but:
- Indirect imports (A imports B imports C) might exist
- The `lettuce/src/server.salt` imports `user.basalt.main`
- Tests might reference deleted modules

**Recommendation:** Before starting S4-001, run `cargo build --release` from a clean state and verify the full kernel + lettuce compilation succeeds.

## Finding 6: No kernel integration test in the plan [LOW]

All verification is `cargo test` in salt-front. The kernel .salt files compile through the Salt compiler and boot in QEMU. We've verified the compiler builds and tests pass, but haven't verified the kernel still boots.

**Recommendation:** Document kernel boot verification as a pre-release gate. Don't block the sprint on it (QEMU requires interactive inspection per CLAUDE.md).

## Finding 7: Phase ordering creates unnecessary coupling [LOW]

Phase 2 (housekeeping) and Phase 3 (scheduler) are independent. They could run in parallel if multiple sessions are available. Phase 1 (clippy) blocks Phase 4 (building) because `-D warnings` must pass before feature work.

**Recommendation:** Run Phase 2 and Phase 3 concurrently if resources allow. Gate Phase 4 on Phase 1 completion.

## Finding 8: Missing: salt-lsp verification [LOW]

The LSP server has its own build and test targets. Our changes to salt-front source files could affect LSP behavior.

**Recommendation:** Add `cargo test -p salt-lsp` to the acceptance criteria for any task that modifies shared source.

## Summary

| # | Severity | Action |
|---|----------|--------|
| 1 | HIGH | Batch clippy fixes by category, test between batches |
| 2 | HIGH | Audit `compile_ast` callers before refactoring |
| 3 | MEDIUM | Use sampling for deep reads, not exhaustive line-by-line |
| 4 | MEDIUM | Time-box scheduler attempt to 1 session |
| 5 | MEDIUM | Verify lettuce compilation before S4-001 |
| 6 | LOW | Document kernel boot as pre-release gate |
| 7 | LOW | Parallelize Phase 2 and Phase 3 if possible |
| 8 | LOW | Add LSP tests to acceptance criteria |

## Updated Plan (incorporating red team findings)

1. **S1-001 (Clippy batch 1):** Run `--fix` for single_char_add_str, needless_return, needless_borrow, collapsible_if only. Test.
2. **S1-002 (Clippy batch 2):** Run `--fix` for redundant_closure, useless_format, map_identity. Test.
3. **S1-003 (Clippy manual):** Audit compile_ast callers. Fix remaining 129 manually. Zero warnings.
4. **S2-001 to S2-004:** Sample-based deep reads (headers + random spot-checks, not exhaustive).
5. **S3-001:** Time-boxed scheduler analysis. Accept exception if no clean path found.
6. **S4-gate:** Verify lettuce + kernel compile before feature work.
