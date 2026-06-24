# Housekeeping Sprint 3 — Deep Reads & Final Polish

## What's been done (sprints 1-2, passes 1-5)

- 30+ files deleted (AI slop with zero callers)
- 500+ AI tags eliminated across 40+ compiler files
- All hallucinated version numbers (V25.8 through V1.0) removed
- All ALL-CAPS AI labels stripped (VERIFIED METAL, GRAYDON FIX, etc.)
- 0 test failures, 0 regressions
- 137 commits in history

## What remains — by priority

### Sprint 1: Line-by-Line Deep Reads (4 tasks)

These 4 files had the most AI tags originally. Batch sed cleaned the obvious
stuff. Now each needs a full read to catch subtle contamination: comments
that over-explain, first-person voice, redundant descriptions, comments that
say what the code does rather than why.

- [x] **H3-001:** Deep-read type_bridge.rs (3019→2942 lines) — cleaned: dead code, 7 eprintln!, 10 bracket labels, orphaned separators
- [x] **H3-002:** Deep-read context.rs (4228→4215 lines) — cleaned: 4 AI artifacts (Linus/Graydon, Directive 2.1), 5 eprintln!, 10 bracket labels, duplicate comment
- **H3-003:** Deep-read stmt.rs (3389 lines) — had V25.8/CONSTITUTIONAL GUARD
- **H3-004:** Deep-read mod.rs (2793 lines) — had COUNCIL tags

### Sprint 2: user/ Directory Cleanup (3 tasks)

100 Salt files remain. Some are real (lettuce, basalt, facet), some unknown.

- **H3-005:** Audit user/basalt/ (5 files, imported by lettuce) — strip AI headers
- **H3-006:** Audit user/facet/ Salt files — check comments
- **H3-007:** Scan remaining user/ files for AI contamination

### Sprint 3: Project Artifacts (3 tasks)

- **H3-008:** Clean memory files in ~/.claude/ — archive or rewrite
- **H3-009:** Audit separator density — flag files >10% decoration
- **H3-010:** Final build, test, and comprehensive zero-scan

### Deferred (lower priority)

- Separator bar density reduction (1697 bars — cosmetic)
- Commit message history cleanup (137 formulaic messages — git history)
- scheduler.salt 539→<500 (legitimate exception — needs careful work)
- salt-lsp test coverage (deferred — needs test harness)
- Z3 contract edge-case tests (deferred — Z3 shim disabled)
