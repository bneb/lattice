# Housekeeping Sprint — AI Slop Removal

Goal: eliminate AI-generated cringe, hallucinated jargon, and word-salad
comments from the entire codebase. Each task covers one file or group of
related files. Deep-audit each file — one offense indicates deeper
contamination.

## Sprint 1: Grade A — Delete Hallucinated Files (5 tasks)

### H-001: Delete user/os/process.salt
- **File:** `user/os/process.salt` (46 lines)
- **Offense:** 7 hallucinated `facet_os_*` extern fns with zero kernel backing.
  Line 17 is the worst line in the repo: word salad of 20 adverbs.
- **Action:** Delete entire file. It wraps functions that don't exist.
- **Acceptance:** `git rm`, build passes, no references broken.

### H-002: Deep-clean user/netd/crypto/bigint.salt
- **File:** `user/netd/crypto/bigint.salt` (176 lines)
- **Offense:** Word-salad comments throughout. "Zero-Copy Large Number
  Mathematical Algebra." "Natively performs 32-byte addition executing
  carries implicitly naturally."
- **Action:** Full file audit. Strip all AI comments. Verify the actual
  bigint implementation is correct, replace comments with plain
  descriptions.
- **Acceptance:** Zero AI-signature phrases, build passes.

### H-003: Deep-clean user/netd/crypto/aes.salt
- **File:** `user/netd/crypto/aes.salt` (38 lines)
- **Offense:** "Xoring against abstract context bounds seamlessly
  guaranteeing memory pointer safety."
- **Action:** Full audit. Replace word salad with accurate description
  of the AES operation being performed.
- **Acceptance:** Zero AI-signature phrases, build passes.

### H-004: Deep-clean user/netd/crypto/ecdsa.salt
- **File:** `user/netd/crypto/ecdsa.salt` (55 lines)
- **Offense:** "Cryptographic signatures limits failed seamlessly
  evaluating cleanly."
- **Action:** Full audit. Replace word salad.
- **Acceptance:** Zero AI-signature phrases, build passes.

### H-005: Deep-clean user/netd/tls/handshake.salt
- **File:** `user/netd/tls/handshake.salt` (36 lines)
- **Offense:** "Forges the legacy 0x0303 formatting arrays bridging
  Port 443 seamlessly."
- **Action:** Full audit. Replace word salad.
- **Acceptance:** Zero AI-signature phrases, build passes.

## Sprint 2: Grade B — Strip AI Jargon from Compiler (6 tasks)

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

## Sprint 3: Grade C — Clean Artifacts (3 tasks)

### H-012: Archive AI-generated tracking docs
- **Files:** `.claude/goals/SPRINT_PLAN.md` (176 lines), `.claude/goals/QUALITY_METRICS.md` (277 lines)
- **Action:** Delete both files. They were sprint ephemera. STATUS.md retains
  the meaningful project history.
- **Acceptance:** Files removed, git history preserved.

### H-013: Clean CLAUDE.md language
- **File:** `CLAUDE.md`
- **Offense:** Contains AI-authored strategy framing ("Tier 1 — Kernel,
  highest impact, smallest files"). Priority queue section reads like
  AI process theater.
- **Action:** Rewrite priority queue section to be a plain checklist.
  Remove "ratchet" language. Keep factual content.
- **Acceptance:** Reads like a human wrote it.

### H-014: Clean up overstated labels
- **Target:** 16 CRITICAL: and 11 SECURITY FIX: labels that are actually
  normal bounds checks or null guards.
- **Action:** Audit each occurrence. Downgrade to plain comments where
  appropriate. Keep only genuinely security-relevant invariants.
- **Acceptance:** Only genuine security invariants labeled as such.

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
