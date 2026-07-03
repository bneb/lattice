# Verification Sprint v1.2.0 — Autonomous /loop Prompt (RALPH)

## Goal Prompt

```
Read .claude/goals/VERIFICATION_SPRINT.md. Find the first unchecked phase
(starting from Phase 1). That is your ONLY work for this /loop iteration.

Execution checklist:
1. Read the phase section in VERIFICATION_SPRINT.md for the specific tasks.
2. Implement the changes in the listed files.
3. After each file: cargo check to verify compilation.
4. When all changes made: cargo test --lib (must pass, 38 test binaries).
5. Run cargo clippy -- -D warnings (must pass).
6. Run bash tests/z3_contracts/run_tests.sh (must pass, 17/17).
7. Write a test file in tests/z3_contracts/ demonstrating the new capability.
8. Update VERIFICATION_SPRINT.md: check the phase box with today's date
   and record key metrics (e.g., Z3: 8/8 checks proven).
9. Commit with a descriptive message matching the phase.
10. Run git push.
11. Stop. Do not start the next phase until the next /loop iteration.

Hard constraints:
- Max 32 non-blank lines per function
- Max 500 lines per file (extract helpers to new modules if needed)
- No mutant markers (TODO/FIXME/HACK/XXX/temp_/workaround)
- cargo test must pass before commit
- cargo clippy must pass before commit
- Z3 contracts (17/17) must pass before commit
- git push after each commit

If a phase is blocked by a dependency (e.g., z3 crate bug):
  - Document the block in VERIFICATION_SPRINT.md with specific error message
  - Move to the next unblocked phase
  - Do not spend more than 2 iterations on a blocked phase

Commit message format:
  feat: <phase description> — <key metric>

Example:
  feat: case-splitting — insertion sort fully verified (12/12 checks proven)
```

## Usage

```
/loop "Read .claude/goals/VERIFICATION_SPRINT.md. Find the first unchecked phase. Execute it per the prompt in VERIFICATION_LOOP_PROMPT.md. Stop after one phase."
```

## Stop Conditions
- All phases 1 through 6 checked off → v1.2.0 ready, tag and push.
- A phase is blocked by external dependency → document, skip to next.
- Three consecutive CI failures on a phase → halt, request human review.
