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

## Phase 4: Frame Axiom Validation [x] 2026-07-03

**Goal:** Confirm frame axioms work and are exercised by a concrete test. Native
Z3 Array migration is deferred (Rust lifetime issue, not a Z3 bug), but the
existing versioned UF approach with frame axioms is validated.

**Findings on native Z3 Array migration:**
- `z3::ast::Array::store`/`select` work fine in z3 0.12.1 (the reported crash was a false alarm)
- Full migration blocked by Rust lifetime issues: `Array::store` requires `Ast<'ctx>` but
  `translate_to_z3` returns `Int<'a>`; `pub(crate)` visibility on wrap/Z3_context prevents FFI
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
