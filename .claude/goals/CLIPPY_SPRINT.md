# Clippy Zero Sprint — Final Plan

Goal: remove every `= "allow"` from `Cargo.toml [lints.clippy]`.
13 categories remain, ~70 instances total.

## Rules
- One category per session. Fix all instances, never add crate-level allows.
- Site-level `#[allow]` only with a `// REASON:` comment when the fix is structurally blocked.
- `cargo test --lib` must pass before commit.
- Commit format: `chore: fix clippy::<lint> — N instances, allow removed`

---

## Phase A: Medium-Difficulty (16 instances, 4 sessions)

### A1. if_same_then_else (6 instances)
**Files:** `src/codegen/abi.rs` (4), `src/codegen/emit_hir.rs` (2)
**What:** `if cond { X } else { X }` — identical branches. May indicate copy-paste bugs or dead conditions.
**Fix:** If the condition is dead, remove the `if` and keep the body. If both branches are genuinely identical, hoist the code above the `if` and leave only the meaningful conditional (if any).
**Risk:** Medium — verify semantics haven't changed.
**Prompt:**
```
Fix every instance of clippy::if_same_then_else in salt-front.
Remove if_same_then_else = "allow" from Cargo.toml.
Run: cargo clippy -- -D warnings 2>&1 to find all instances.
For each:
- Read the surrounding code. Determine if the condition is dead
  (always true/false) or if both branches genuinely do the same thing.
- If dead condition: remove the if, keep the body.
- If same body: hoist the body before the if, remove body from branches.
- If uncertain (complex logic): add #[allow(clippy::if_same_then_else)]
  at the site with a // REASON: comment explaining why.
Run cargo test --lib after each file.
When clean, commit: "chore: fix clippy::if_same_then_else — N instances, allow removed"
```

### A2. only_used_in_recursion (4 instances)
**Files:** `src/codegen/verification/` (2), `src/codegen/passes/` (2)
**What:** Function parameters that are only used in recursive calls with the same value.
**Fix:** Remove the parameter, wrap the function body in an inner function or closure that captures the recursive argument. Or if the parameter serves as documentation, add `#[allow]` with comment.
**Risk:** Low for removal, medium if the parameter affects public API.
**Prompt:**
```
Fix every instance of clippy::only_used_in_recursion in salt-front.
Remove only_used_in_recursion = "allow" from Cargo.toml.
Run clippy to find all instances.
For each parameter flagged:
- If the function is private: remove the parameter, use an inner fn/clojure
  that captures the value for recursive calls.
- If the function is pub: add #[allow(clippy::only_used_in_recursion)]
  with // REASON: public API stability.
Run cargo test --lib. When clean, commit.
```

### A3. unnecessary_filter_map (2 instances)
**What:** `.filter_map(|x| expr)` where `expr` always returns `Some(...)`.
**Fix:** Replace `.filter_map(|x| expr)` with `.map(|x| expr.unwrap())` or restructure to use `.map()` directly.
**Risk:** None — mechanical.
**Prompt:**
```
Fix every instance of clippy::unnecessary_filter_map in salt-front.
Remove unnecessary_filter_map = "allow" from Cargo.toml.
For each: replace .filter_map() with .map().
Run cargo test --lib. When clean, commit.
```

### A4. Singleton Roundup (4 instances, 1 session)
**Lints:** `useless_conversion`, `new_without_default`, `should_implement_trait`, `match_like_matches_macro`
**Files:** Various (1 instance each)
**Prompts per lint:**
- `useless_conversion`: Remove redundant `.into()` / `.try_into()` call.
- `new_without_default`: Add `#[derive(Default)]` or impl `Default` for `PointerStateTracker`.
- `should_implement_trait`: Rename `from_str` method or impl `std::str::FromStr`.
- `match_like_matches_macro`: Replace single-pattern match with `matches!()` macro.

---

## Phase B: Pattern Merge (20 instances, 4-6 sessions)

### B1-B6. collapsible_match (10 instances) + collapsible_if (10 instances)
**Files:** See breakdown below. Each file = 1 session.

| Session | Files | Instances |
|---------|-------|-----------|
| B1 | `src/codegen/context.rs` + `context/resolver.rs` | 4 |
| B2 | `src/codegen/expr/mod.rs` + `expr/resolver.rs` | 4 |
| B3 | `src/codegen/mod.rs` | 3 |
| B4 | `src/codegen/seeker.rs` + `stmt.rs` | 3 |
| B5 | `src/codegen/expr/method_resolution.rs` + `expr/calls.rs` + `expr/literals.rs` | 4 |
| B6 | `src/codegen/expr/memory.rs` + `intrinsics/system.rs` + `generic_resolver.rs` + `phases/resolution/name_resolver.rs` | 4 |
| B7 | `src/hir/async_lower.rs` | 4 |

**What:** Nested `if let` patterns that can be merged by inlining the inner destructure into the outer pattern.
**Fix pattern:**
```rust
// Before:
if let Outer(inner) = expr {
    if let Pattern(x) = inner {
        // body
    }
}
// After:
if let Outer(Pattern(x)) = expr {
    // body
}
```
**Risk:** Low — pure syntactic rewrite. Always verify the inner `if let` destructures the value bound in the outer `if let`.
**Prompt (for each session):**
```
Fix collapsible_match and collapsible_if in <FILE_LIST>.
Remove collapsible_match = "allow" and collapsible_if = "allow" from Cargo.toml.
Run: cargo clippy -- -D warnings 2>&1 to find all instances in the target files.
For each instance in <FILE_LIST> only:
- Merge the nested if-let by inlining the inner pattern into the outer.
  Example: if let Some(x) = expr { if let Pattern(y) = x { body } }
  Becomes: if let Some(Pattern(y)) = expr { body }
- Remove the extra closing brace from the now-eliminated inner if-let.
- Compile and test between each file.
When all instances in the target files are fixed:
- Restore allows in Cargo.toml for files NOT yet fixed
- Commit the fixed files
- File count should decrease; collapsible count should decrease
Run cargo test --lib. When all target files clean, commit.
After all B1-B7 sessions complete: remove both allows from Cargo.toml permanently.
```

---

## Phase C: Structural Refactoring (36 instances, 5-7 sessions)

### C1. type_complexity (3 instances)
**What:** Very complex types that should be factored into `type` aliases.
**Fix:** Extract `type` definitions at module level.
**Risk:** Low — name the aliases descriptively.
**Prompt:**
```
Fix every instance of clippy::type_complexity in salt-front.
Remove type_complexity = "allow" from Cargo.toml.
For each complex type flagged: extract a module-level `type` alias with
a descriptive name (e.g., `type MethodResolver = ...`).
Run cargo test --lib. When clean, commit.
```

### C2. large_enum_variant (1 instance)
**What:** `SynType::ShapedTensor` variant is much larger than others (≥40 bytes vs 248 bytes).
**Fix:** Box-wrap the large variant's fields: `ShapedTensor { element: Box<Type>, ... }`.
**Risk:** Medium — changes the enum layout. Update all pattern matches on this variant.
**Prompt:**
```
Fix clippy::large_enum_variant in salt-front/src/grammar.rs (SynType enum).
Remove large_enum_variant = "allow" from Cargo.toml.
Box-wrap the ShapedTensor variant's largest field. Update all pattern matches
and construction sites to add/remove the Box wrapper.
Run cargo test --lib. When clean, commit.
```

### C3. inherent_to_string (1 instance)
**What:** `grammar::SynPath` has an inherent `to_string(&self) -> String` method that shadows `std::string::ToString::to_string`.
**Fix:** Implement `std::fmt::Display` for `SynPath` instead, or rename the method.
**Risk:** Medium — changes trait impl surface. Callers using `.to_string()` still work via the blanket impl.
**Prompt:**
```
Fix clippy::inherent_to_string in salt-front/src/grammar.rs (SynPath type).
Remove inherent_to_string = "allow" from Cargo.toml.
Replace the inherent to_string() method with an impl of std::fmt::Display.
Update any callers that relied on the inherent method (Display gives to_string() for free).
Run cargo test --lib. When clean, commit.
```

### C4-C7. too_many_arguments (30 instances, 3-4 sessions)
**What:** Functions with 8+ parameters. The plan calls for introducing config structs for the worst offenders.
**Priority targets:**

| Session | Functions | Args | Approach |
|---------|-----------|------|----------|
| C4 | `compile_ast` (11), `load_imports` (10) | 11, 10 | Introduce `CompileOptions`, `ImportOptions` structs |
| C5 | `emit_struct_eq` (8), `emit_tuple_eq` (8), `emit_array_eq` (9), `emit_enum_eq` (8) | 8-9 | Introduce `EqEmitOptions` struct |
| C6 | `resolve_method_generics` (9), `emit_low_level_call` (8), `try_resolve_static_method` (8) | 8-9 | Group related params |
| C7 | Remaining ~15 functions with 8 args | 8 | Group params or site-level allow with rationale |

**Prompt (per session):**
```
Fix too_many_arguments in <TARGET_FUNCTIONS> in salt-front.
Remove too_many_arguments = "allow" from Cargo.toml.
For each target function with N > 7 args:
- Identify logically related groups of parameters
- Create a config struct bundling the related params
- Update the function signature to take the struct
- Update all call sites to construct the struct
Run cargo test --lib after each function.
When the target functions are fixed, restore the allow for remaining functions.
Commit with: "chore: fix clippy::too_many_arguments — <functions>, <N> args reduced"
After all C4-C7 sessions: remove the allow permanently.
```

---

## Phase D: Cleanup (1 session)
After all categories are fixed:
- Remove all commented-out `= "allow"` lines from Cargo.toml
- Verify `cargo clippy -- -D warnings` returns zero with NO allows
- Verify `cargo test --lib` passes
- Mark CLIPPY_SPRINT.md as complete
- Update STATUS.md

---

## Progress Tracker

| Phase | Lint | Instances | Sessions | Status |
|-------|------|-----------|----------|--------|
| A1 | if_same_then_else | 6 | 1 | ⬜ |
| A2 | only_used_in_recursion | 4 | 1 | ⬜ |
| A3 | unnecessary_filter_map | 2 | 1 | ⬜ |
| A4 | singletons (4 lints) | 4 | 1 | ⬜ |
| B1-B7 | collapsible_match/if | 20 | 7 | ⬜ |
| C1 | type_complexity | 3 | 1 | ⬜ |
| C2 | large_enum_variant | 1 | 1 | ⬜ |
| C3 | inherent_to_string | 1 | 1 | ⬜ |
| C4-C7 | too_many_arguments | 30 | 4 | ⬜ |
| D | cleanup | - | 1 | ⬜ |
| **Total** | **13 categories** | **~70** | **~19** | |
