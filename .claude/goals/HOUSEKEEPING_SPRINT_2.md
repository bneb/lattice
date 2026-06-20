# Housekeeping Sprint 2 — Line-by-Line Decontamination

Goal: read every contaminated file in full, strip all AI voice from comments,
remove patronizing language, fix hallucinated references, and verify code
quality. This is NOT batch sed work — each file gets a full read.

## Sprint 1: Grade A — Delete, Don't Salvage (3 tasks)

Files so thoroughly AI-generated that salvage is more work than rewrite.

### H2-001: Audit user/netd/ directory
- **Target:** `user/netd/` — 11 files remaining after H-001 deletions
- **Check:** Are these files real implementations or AI stubs? Do they compile against the kernel? Do they have callers?
- **Action:** Delete any file with no callers and no real implementation. Keep and clean any that are genuine.
- **Acceptance:** Only real, working code remains.

### H2-002: Audit user/basalt/ directory
- **Target:** `user/basalt/` — 5 files
- **Check:** Same as above. Are these real? "Basalt" is a suspicious name.
- **Action:** Delete or clean.
- **Acceptance:** Only real code remains.

### H2-003: Audit user/facet/ directory
- **Target:** `user/facet/` — 25 files, 7072 lines
- **Check:** Facet has real C/ObjC files — it's a real graphics framework. But do the Salt files have AI-contaminated comments?
- **Action:** Deep-read Salt files. Clean comments. Verify C FFI bindings match real functions.
- **Acceptance:** Salt files are clean. C files are unchanged (not our domain).

## Sprint 2: Grade B — Deep-Clean Contaminated Compiler Files (8 tasks)

These files had [VERIFIED METAL], [PHASE X], or [KEUOS VN] tags. One tag
indicates deeper contamination. Each task: read the full file, fix every
comment, check for code quality issues.

### H2-004: Deep-clean salt-front/src/codegen/type_bridge.rs
- **File:** 3258 lines. Most contaminated: had 9 AI tags.
- **Offenses to fix:** "We" voice in comments, over-explaining, patronizing
  "simply/just/clearly", hallucinated "In strict mode we could panic"
  in a compiler that has no strict mode.
- **Acceptance:** Comments describe what/why without first-person, exaggeration,
  or hand-waving about future features.

### H2-005: Deep-clean salt-front/src/codegen/context.rs
- **File:** 4901 lines. Had 8 AI tags.
- **Offenses to fix:** Same audit pattern. Check for hallucinated Z3 methods
  (the disabled block at line 4806), over-explaining accessor comments.
- **Acceptance:** Clean, professional comments.

### H2-006: Deep-clean salt-front/src/codegen/mod.rs
- **File:** 2793 lines. Had 2 AI tags plus scattered contamination.
- **Action:** Full read. Fix comments.
- **Acceptance:** Clean.

### H2-007: Deep-clean salt-front/src/codegen/expr/ files
- **Files:** mod.rs, resolver.rs, binary_ops.rs, memory.rs, calls.rs,
  special_methods.rs, method_resolution.rs, literals.rs, utils.rs,
  control_flow.rs, aggregate_eq.rs
- **Action:** Full read of each. Fix comments. Check for patronizing language.
- **Acceptance:** Clean across all 11 files.

### H2-008: Deep-clean salt-front/src/types.rs + lib.rs + grammar.rs
- **Files:** types.rs (1163 lines), lib.rs (1911 lines), grammar.rs (1593 lines)
- **Action:** Full read. These had KEUOS V tags and heavy "we" usage.
- **Acceptance:** Clean.

### H2-009: Deep-clean salt-front/src/codegen/verification/ files
- **Files:** mod.rs, slice_verifier.rs, proof_witness.rs, exhaustiveness.rs,
  silicon_ingest.rs, state_tracker.rs, stack_stability.rs,
  ptr_bounds_verifier.rs, pointer_state.rs
- **Offense:** "We" voice, over-explaining, AI confidence about unverified code.
- **Action:** Full read of each.
- **Acceptance:** Comments are factual, not aspirational.

### H2-010: Deep-clean salt-front/src/hir/ files
- **Files:** typeck.rs, async_lower.rs, lower.rs, mod.rs, vc.rs,
  verify_pulse_bounds/mod.rs
- **Action:** Full read. Strip AI voice.
- **Acceptance:** Clean.

### H2-011: Deep-clean remaining salt-front files with AI markers
- **Files:** tracer.rs, struct_deriver.rs, driver.rs, grammar_tokens.rs,
  keywords.rs, passes/comptime.rs, const_eval.rs, collector.rs,
  trait_registry.rs, cli.rs, fuzz_ast.rs, evaluator.rs, abi.rs
- **Action:** Full read of each.
- **Acceptance:** Clean.

## Sprint 3: Grade C — Kernel Comment Audit (4 tasks)

### H2-012: Deep-clean kernel/core/ files with contamination
- **Files:** Any kernel .salt file with "we", "obviously", "simply",
  "FATAL", or "CRITICAL" in comments. Also check for duplicated
  architectural explanations across files.
- **Action:** Full read. Fix comments. Remove redundant architecture
  explanations (if the same explanation appears in 3 files, it goes
  in one doc, not three source files).
- **Acceptance:** Clean comments, no redundancy.

### H2-013: Deep-clean kernel/mem/ files
- **Files:** user_paging.salt, paging_destroy.salt, slab_cache.salt,
  pmm_sharded.salt
- **Action:** Full read. Fix comments.
- **Acceptance:** Clean.

### H2-014: Deep-clean kernel/net/ files
- **Files:** All .salt files in kernel/net/
- **Action:** Full read. Fix comments.
- **Acceptance:** Clean.

### H2-015: Deep-clean remaining kernel files
- **Files:** kernel/ecs/, kernel/benchmarks/, kernel/lib/, kernel/sched/,
  kernel/drivers/, kernel/arch/, kernel/sys/
- **Action:** Full read of each. Fix comments.
- **Acceptance:** Clean.

## Sprint 4: Grade D — Final Sweep (3 tasks)

### H2-016: Global scan for remaining AI signatures
- **Action:** Run comprehensive grep for all known AI patterns.
  Flag any new ones found during sprints 1-3.
- **Acceptance:** Zero hits for all known patterns.

### H2-017: Verify user/ directory is clean
- **Action:** Final scan of all user/ files. Confirm no AI slop remains.
- **Acceptance:** Clean.

### H2-018: Final build + test verification
- **Action:** `cargo build --release`, `cargo test`, verify 0 failures.
- **Acceptance:** Green build, green tests.

## Execution Rules

1. **Read the full file.** Don't spot-fix. If a file is in the sprint, read
   every line.
2. **Fix comments, don't delete them.** If a comment is wrong or wordy, rewrite
   it accurately and concisely. If it explains what the code visibly does,
   remove it. If it explains why, keep it.
3. **Never edit code behavior.** This is a comment/docs cleanup. Don't change
   logic, function signatures, or types.
4. **Build after each task.** `cargo build --release` must pass.
5. **Commit with `chore: H2-00X description`.**
