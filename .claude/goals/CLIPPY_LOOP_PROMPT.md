# Clippy Sprint — Autonomous /loop Prompt

## Goal Prompt

```
Read .claude/goals/CLIPPY_SPRINT.md. Find the first unchecked phase item
(starting from Phase A1). That is your ONLY work for this /loop iteration.

Execution checklist:
1. Read the phase section in CLIPPY_SPRINT.md for the specific prompt.
2. cd salt-front && cargo clippy -- -D warnings 2>&1 | head -5
   If clippy shows 0 errors for your target lint: mark it 0 instances in
   the sprint plan, check it off, commit, and move to the next phase next iteration.
3. Remove the allow from Cargo.toml for your target lint(s) only.
4. Run cargo clippy -- -D warnings 2>&1 to find all instances (file:line).
5. Fix each instance per the phase-specific prompt instructions.
6. After each file: cargo check to verify compilation.
7. When all instances fixed: cargo test --lib (must pass).
8. Update CLIPPY_SPRINT.md: check the box with today's date.
9. Commit: "chore: fix clippy::<lint> — N instances, allow removed"
10. Stop. Do not start the next phase until the next /loop iteration.

Hard constraints (hook-enforced):
- Max 32 non-blank lines per function
- Max 500 lines per file (do not grow files already >500)
- Max 3 nesting levels
- No mutant markers (TODO/FIXME/HACK/XXX/temp_/workaround)
- cargo test must pass before commit

If a fix would violate a constraint: add #[allow(clippy::<lint>)] at the
specific site with a // REASON: comment, and document the exception in
the commit message. Remove the crate-level allow anyway — the site-level
one handles that instance.

Never re-add a crate-level allow to Cargo.toml once removed.
If an instance cannot be fixed this session, leave the lint commented out
in Cargo.toml with a note: "# <lint> = "allow" — N remaining in <file>"

Commit after every lint category is complete.
```

## Usage

For continuous execution:
```
/loop "Read .claude/goals/CLIPPY_SPRINT.md. Find the first unchecked item. Execute it per the prompt in CLIPPY_LOOP_PROMPT.md. Stop after one category."
```

For a single sprint:
```
Fix all clippy::if_same_then_else instances in salt-front. [paste A1 prompt]
```

## Stop Conditions
- All phases A1 through D checked off → sprint complete.
- A phase requires architectural changes beyond the scope of lint fixing → document in the sprint plan under "Blocked" with rationale, move to next phase.
- Three consecutive instances in a phase require site-level #[allow] → assess whether that phase should be deferred as a group and allow at crate level with a dated note.
