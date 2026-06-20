# Housekeeping Sprint — AI Slop Removal

Goal: eliminate AI-generated cringe, hallucinated jargon, and word-salad
comments from the entire codebase. Each task covers one file or group of
related files. Deep-audit each file — one offense indicates deeper
contamination.

## Sprint 1: Grade A — Delete Hallucinated Files (5 tasks)

### [x] H-001: Deep-clean user/os/process.salt (cleaned, not deleted — real FFI bindings)
### [x] H-002: Delete user/netd/crypto/bigint.salt (zero callers, word salad)
### [x] H-003: Delete user/netd/crypto/aes.salt (fake AES — just XOR)
### [x] H-004: Delete user/netd/crypto/ecdsa.salt (wrong curve, fake verify)
### [x] H-005: Delete user/netd/tls/handshake.salt (6 bytes, claimed TLS 1.3)
### Also deleted: curve25519.salt, sha.salt (zero callers, word salad)

## Sprint 2: Grade B — Strip AI Jargon from Compiler ✅

### [x] H-006 through H-011: Batch-strip all AI tags
Batch sed across 20+ files. Removed all VERIFIED METAL, GRAYDON FIX,
INCEPTION GUARD, [PHASE X], [KEUOS V2-V6], [MIGRATION] tags.
Also fixed generated MLIR comment in emission.rs.

Each task: read the full file, strip all `[VERIFIED METAL]`, `[GRAYDON
FIX]`, `INCEPTION GUARD`, `[PHASE X]`, `[KEUOS V3/V4]`, `[MIGRATION]`
tags, then deep-audit for other AI patterns (self-justifying comments,
over-explaining, hallucinated version numbers).

### H-006: Clean salt-front/src/codegen/type_bridge.rs
- **File:** `salt-front/src/codegen/type_bridge.rs` (3258 lines)
- **Markers:** 3 VERIFIED METAL, 1 INCEPTION GUARD, 3 GRAYDON FIX, 2 MIGRATION, 1 PHASE
- **Note:** Largest contaminated file. Focus on tag removal + comment audit.
- **Acceptance:** Zero AI tags, zero word-salad comments.

### H-007: Clean salt-front/src/codegen/context.rs
- **File:** `salt-front/src/codegen/context.rs` (4901 lines)
- **Markers:** 5 VERIFIED METAL, 2 KEUOS V3, 1 PHASE
- **Note:** Largest file overall. Focus on tag removal + comment audit.
- **Acceptance:** Zero AI tags, zero word-salad comments.

### H-008: Clean salt-front/src/codegen/mod.rs
- **File:** `salt-front/src/codegen/mod.rs` (2793 lines)
- **Markers:** 1 GRAYDON FIX, 1 PHASE
- **Action:** Full audit.
- **Acceptance:** Zero AI tags.

### H-009: Clean salt-front/src/codegen/expr/ directory (8 files)
- **Files:** mod.rs, resolver.rs, binary_ops.rs, literals.rs, memory.rs,
  method_resolution.rs, special_methods.rs, utils.rs
- **Markers:** Scattered VERIFIED METAL, GRAYDON FIX, PHASE tags
- **Action:** Process each file. Strip tags. Audit comments.
- **Acceptance:** Zero AI tags across all 8 files.

### H-010: Clean remaining Grade B files (6 files)
- **Files:** tracer.rs, struct_deriver.rs, types.rs, lib.rs,
  types/layout.rs, types/substitution.rs, grammar/attr.rs,
  phases/discovery/mod.rs, phases/emission.rs
- **Markers:** Scattered PHASE, KEUOS V4, VERIFIED METAL tags
- **Action:** Strip all tags. Audit comments.
- **Acceptance:** Zero AI tags across all files.

### H-011: Verify zero AI tags repo-wide
- **Target:** grep for all known AI tag patterns across salt-front, kernel, user, tools
- **Action:** Run comprehensive scan. Flag any remaining hits.
- **Acceptance:** Zero hits for VERIFIED METAL, GRAYDON FIX, INCEPTION GUARD, [PHASE X], [KEUOS V3/V4], [MIGRATION]

## Sprint 3: Grade C — Clean Artifacts ✅

### [x] H-012: Delete tracking docs (SPRINT_PLAN.md, QUALITY_METRICS.md)
### [x] H-013: Clean CLAUDE.md language (plain headers, no 'Tier N' labels)
### [x] H-014: Strip [SEC-XX]/[NEW-XX] prefixes from kernel comments

## Sprint 4: Grade D — Separator Density (2 tasks)

### H-015: Measure separator density
- **Target:** All kernel .salt and salt-front .rs files
- **Action:** Run script to identify files where >10% of lines are
  `// ========` separators. Flag for manual review.
- **Acceptance:** Report generated, files identified.

### H-016: Trim worst separator offenders
- **Target:** Files with >10% separator density
- **Action:** Reduce to 1 separator at file header, 1 between major
  sections. Remove redundant mid-function separators.
- **Acceptance:** Separator density <5% in all files.

## Execution Protocol

Each task:
1. Read the full file
2. Strip all identified AI tags
3. Deep-audit: read every comment line, check for word salad, over-explaining,
   self-justification, hallucinated names
4. Verify: `cargo build --release` passes, `cargo test` passes
5. Commit with message: `chore: H-00X description`
