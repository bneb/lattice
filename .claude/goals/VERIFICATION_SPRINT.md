# Verification Sprint — Closing the Gap

**Start:** 2026-07-03 | **Target:** 2 weeks | **Tag:** v1.2.0

## Completed (v1.1.0)

- `forall i in lo..hi => body` syntax — constant expansion + Z3 ForAll
- For-loop invariants — base case (i==start pinning) + inductive step (i→i+1)
- Concrete for-loop unrolling — 100% proof coverage on constant-bounded loops
- While-loop invariants — base case + inductive step (Havoc)
- Array store tracking — versioned UFs + update assertions + bounded frame axioms
- Body scanning — recursive AST walker for indexed assignments
- Z3 metrics — `Z3: 8/8 checks proven (100%), 0 deferred to runtime`
- Red-team fixes — honest claims, no @trusted bypass, UNSAFE.md accuracy
- CI stability — clippy::manual-checked-ops fixed

## Phase 1: Case-Splitting for Data-Dependent Loops [x] 2026-07-03

**Goal:** Insertion sort fully verified. The inner while-loop exit condition `j < 0 || arr[j] <= key` is already asserted. Split the inductive step into two Z3 cases: `j == -1` and `j >= 0 && arr[j] <= key`. Prove each separately.

**Tasks:**
1. In `array_tracker.rs:prove_for_loop_concrete`, after asserting while-loop exit conditions, push two Z3 sub-frames: one with `j == -1`, one with `j >= 0`
2. In each sub-frame, check the invariant at i+1
3. Both must be UNSAT for the invariant to be preserved

**Files:** `array_tracker.rs`, test: `test_insertion_sort_concrete.salt`
**Effort:** 1-2 days

## Phase 2: Algorithm Verification Suite [x] 2026-07-03

**Goal:** Working, verified implementations of classic algorithms demonstrating each proof technique.

**Tasks:**
1. `test_bubble_sort.salt` — forall ensures + for-loop invariant (already done, 8/8 proven)
2. `test_selection_sort.salt` — same pattern, different inner loop
3. `test_array_fill.salt` — forall ensures with concrete unrolling
4. `test_binary_search.salt` — while-loop invariants for bounds
5. Add all to `tests/z3_contracts/run_tests.sh`

**Files:** `tests/z3_contracts/test_*.salt`, `run_tests.sh`
**Effort:** 1-2 days

## Phase 3: `exists` Quantifier [x] 2026-07-03

**Goal:** `exists i in lo..hi => body` syntax. Same expansion pattern as forall.

**Tasks:**
1. Add `Expr::Exists` variant or reuse `__z3_exists` marker (parallel to forall)
2. Parser: `exists ident in expr..expr => expr`
3. Z3 translation: `exists_const` (stub already present)
4. Test: `test_exists.salt`

**Files:** `grammar/expr_utils.rs`, `memory.rs`, `keywords.rs`
**Effort:** 1 day

## Phase 4: Frame Axiom Validation [x] 2026-07-03 — CLOSED

**Status:** Versioned UF + frame axioms provide equivalent guarantees. Native Z3 Array
migration is permanently deferred — blocked by the LoweringContext two-lifetime
architecture (`translate_to_z3` returns `Int<'a>` but `Array::store` requires
`Ast<'ctx>`). Collapsing lifetimes would cascade through every verification-path
function. The UF approach passes 37/37 contracts including preservation tests.

**Findings on native Z3 Array migration:**
- `z3::ast::Array::store`/`select` work fine in z3 0.12.1 (the reported crash was a false alarm)
- Full migration blocked by Rust lifetime issues: `Array::store` requires `Ast<'ctx>` but
  `translate_to_z3` returns `Int<'a>`; `pub(crate)` visibility on wrap/Z3_context prevents FFI
- Versioned UF + frame axioms provide the same guarantees through a different mechanism
- DESIGN NOTE added at `memory.rs:987` documenting the rationale
- Versioned UF + frame axioms provide the same guarantees through a different mechanism

**What was done:**
1. Validated frame axioms via `test_preservation.salt`: proves `arr[k] == k` for all k
   after a loop that writes `arr[i] = i` — requires preservation of unwritten elements
2. Added preservation test to Z3 regression suite (22/22 pass)
3. Removed dead `resolve_bound_as_i64` stub (unused, kept `resolve_bound` for ForAll path)
4. Verified: 1359 lib tests pass, clippy clean, 22/22 Z3 contracts

**Files:** `test_preservation.salt` (new), `run_tests.sh` (+8 lines), `memory.rs` (+1 line allow)

## Phase 5: Auto-Invariant Inference [DEFERRED] — requires deep Z3/IR integration, 2-3 day scope

**Goal:** Extend `try_infer_while_invariant` to handle array access patterns. Current version only handles `while i < N { i = i + 1 }`. Add array patterns.

**Tasks:**
1. Pattern: `while i < N && arr[i] > key` → infer `invariant i >= 0 && arr[i-1] <= arr[i]`
2. Pattern: `for i in 0..n { arr[i] = v }` → infer `invariant forall k in 0..(i-1): arr[k] == v`
3. Wire into while-loop and for-loop emitters

**Files:** `while_stmt.rs`, `for_loop_emit.rs`
**Effort:** 2-3 days

## Phase 6: Documentation & Blog [x] 2026-07-03

**Goal:** Publish the verification story.

**Tasks:**
1. Blog post: "Proving Sorting Algorithms at Compile Time" — based on `docs/blog/forall-invariants.md`
2. Update BENCHMARKS_E2E.md with algorithm verification coverage
3. Add tutorial chapter: "Verifying Your First Algorithm"
4. Update README with verification badge/metrics example

**Effort:** 1-2 days

## Phase 7: Verification Depth [x] 2026-07-04

**Goal:** Close high-ROI verification gaps identified in the stack-ranked audit.

**Tasks:**
1. `let`-binding in Z3 translation — defensive `Expr::Let` handling in translators,
   evaluator, and constant folder. Prevents silent catch-all failures.
2. Cross-function contract chaining — wired `caller_preconditions` (dead code→live),
   `apply_ensures_to_solver` flows callee postconditions into caller's Z3 solver.
3. Struct field type bounds — `assert_field_type_bounds` constrains field values
   to their type domains (u8 ∈ [0,255], etc.) with thread-local dedup cache.
4. Nested body scanner — `scan_expr_depth` recurses into `Binary`, `Call`,
   `MethodCall`, `Index`, `Unary`, `Field`, `Let` for complete store detection.
5. Auto-invariant `&&` conditions — `try_infer_while_invariant` handles
   `while cond1 && cond2` by trying each sub-condition independently.

**Files:** `memory.rs`, `fold_constants.rs`, `evaluator.rs`, `call_helpers.rs`,
`calls.rs`, `while_stmt.rs`, `array_tracker.rs`
**Tests:** `test_cross_fn_chain.salt`, `test_struct_field_bounds.salt`
**Effort:** 2 sessions (2026-07-03 — 2026-07-04)

## Phase 8: Verification Documentation [x] 2026-07-04

**Goal:** Canonical reference document for what can and cannot be verified.

**Tasks:**
1. Create `docs/VERIFICATION_CAPABILITIES.md` — 16 provable capabilities,
   11 explicit limitations, pipeline architecture, rules of thumb
2. Update BENCHMARKS_E2E.md with new verification coverage
3. Update forall-invariants.md with cross-function chaining + struct bounds examples

**Files:** `docs/VERIFICATION_CAPABILITIES.md` (new), `BENCHMARKS_E2E.md`,
`docs/blog/forall-invariants.md`
**Effort:** 1 session
