# Clippy Sprint — Autonomous /loop Prompt

Feed this to `/loop` (self-paced mode). The agent will work through
`.claude/goals/CLIPPY_SPRINT.md` one lint category at a time.

---

## Goal Prompt

```
Read .claude/goals/CLIPPY_SPRINT.md. Find the first unchecked lint category.
That is your ONLY target for this session.

Workflow:
1. Remove the "= allow" for that one lint from salt-front/Cargo.toml
2. Run: cargo clippy -- -D warnings 2>&1
   This shows every instance of the lint with file:line locations.
3. Read each affected file and fix each instance. Apply fixes one file at a time.
4. After fixing a file, run: cargo test --lib
   If tests fail, undo and fix correctly.
5. When cargo clippy -- -D warnings shows zero errors for that lint:
   - The allow is already removed from Cargo.toml (step 1)
   - Run cargo test --lib one final time
   - Check the box in CLIPPY_SPRINT.md with today's date
   - Commit: "chore: fix clippy::<lint> — N instances, allow removed"

Hard constraints (hook-enforced):
- Never make a function longer than 32 lines
- Never add nesting beyond 3 levels
- Never make a file exceed 500 lines (if already >500, don't increase it)
- cargo test must pass before commit

If a fix would violate a constraint, leave that instance, add a comment
explaining why, and note it in the commit message. Remove the allow anyway
for the instances you DID fix — add #[allow] at the specific site for
the unfixable one.

Never suppress a lint at the crate level (Cargo.toml or lib.rs) for a
category that's been started. Only site-level #[allow] with a comment
for exceptional cases.

When done with one category, move to the next unchecked one in the next
/loop iteration.

Stop conditions:
- All categories checked = sprint complete
- A category requires architectural changes beyond the scope of lint fixing
  (document why in CLIPPY_SPRINT.md and move on)
```

## Usage

```
/loop "..."   # with the prompt above
```

Or for a single sprint:
```
/loop "Sprint 1: Fix every instance of clippy::cmp_owned. [paste full prompt from CLIPPY_SPRINT.md for cmp_owned]"
```
