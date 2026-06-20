# Housekeeping Sprint 2 — Line-by-Line Decontamination

Goal: read every contaminated file in full, strip all AI voice from comments,
remove patronizing language, fix hallucinated references, and verify code
quality. This is NOT batch sed work — each file gets a full read.

## Sprint 1: Grade A — Delete, Don't Salvage ✅

### [x] H2-001: Delete user/netd/ (13 files, 2113 lines — zero callers, word salad)
### [x] H2-002: user/basalt/ skipped (imported by lettuce — needs careful review)
### [x] H2-003: user/facet/ skipped (real C/ObjC code, Salt files clean)

## Sprint 2: Grade B — Compiler Tag Cleanup ✅

### [x] H2-004 through H2-011: Two-pass tag removal
Pass 1: VERIFIED METAL, GRAYDON FIX, INCEPTION GUARD, PHASE X, KEUOS VN, MIGRATION
Pass 2: KEUOS FIX, KEUOS PHASE N, GRAMMAR CHECK, SCORCHED EARTH, ABI FIX,
ITERATOR PROTOCOL, SALT SYNTAX, PHANTOM FIX, COMPILER BUG FIX, KEUOS WRITER PROTOCOL
Total: ~275 tags removed from 30+ files.

Patronizing language ("simply/clearly/obviously/just") already at near-zero.
"We" voice found to be mostly natural usage, not AI grandiosity.
Over-explaining and overly-dramatic labels are mostly in test files (appropriate context).

### [x] Build + test fix: updated mlir_finalization_test assertions for renamed comment string

## Sprint 3: Grade C — Kernel Comment Audit (deferred)

Kernel files have minimal AI contamination. Most remaining "FATAL" and "CRITICAL"
labels in kernel are genuine error paths, not AI grandiosity. Deeper audit
deferred to avoid removing useful context.

## Sprint 4: Grade D — Final Sweep ✅

### [x] H2-016: Global scan — all known AI patterns at zero
### [x] H2-017: user/ directory — netd deleted, basalt/facet deferred
### [x] H2-018: Build + test — 1254 lib + all integration tests pass, 0 failures
