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

## Phase 1: Case-Splitting for Data-Dependent Loops

**Goal:** Insertion sort fully verified. The inner while-loop exit condition `j < 0 || arr[j] <= key` is already asserted. Split the inductive step into two Z3 cases: `j == -1` and `j >= 0 && arr[j] <= key`. Prove each separately.

**Tasks:**
1. In `array_tracker.rs:prove_for_loop_concrete`, after asserting while-loop exit conditions, push two Z3 sub-frames: one with `j == -1`, one with `j >= 0`
2. In each sub-frame, check the invariant at i+1
3. Both must be UNSAT for the invariant to be preserved

**Files:** `array_tracker.rs`, test: `test_insertion_sort_concrete.salt`
**Effort:** 1-2 days

## Phase 2: Algorithm Verification Suite

**Goal:** Working, verified implementations of classic algorithms demonstrating each proof technique.

**Tasks:**
1. `test_bubble_sort.salt` — forall ensures + for-loop invariant (already done, 8/8 proven)
2. `test_selection_sort.salt` — same pattern, different inner loop
3. `test_array_fill.salt` — forall ensures with concrete unrolling
4. `test_binary_search.salt` — while-loop invariants for bounds
5. Add all to `tests/z3_contracts/run_tests.sh`

**Files:** `tests/z3_contracts/test_*.salt`, `run_tests.sh`
**Effort:** 1-2 days

## Phase 3: `exists` Quantifier

**Goal:** `exists i in lo..hi => body` syntax. Same expansion pattern as forall.

**Tasks:**
1. Add `Expr::Exists` variant or reuse `__z3_exists` marker (parallel to forall)
2. Parser: `exists ident in expr..expr => expr`
3. Z3 translation: `exists_const` (stub already present)
4. Test: `test_exists.salt`

**Files:** `grammar/expr_utils.rs`, `memory.rs`, `keywords.rs`
**Effort:** 1 day

## Phase 4: Z3 Native Array Theory

**Goal:** Replace versioned UFs with Z3 `store`/`select`. Eliminates manual frame axioms. Currently blocked on z3-0.12 crash — needs either z3 upgrade or C API workaround.

**Tasks:**
1. Debug z3-0.12 `Array::store` crash (null AST pointer at `ast.rs:630`)
2. If unfixable, try z3-sys raw FFI: `Z3_mk_store` / `Z3_mk_select` directly
3. Replace `FuncDecl` in `translate_to_z3:Expr::Index` with `Array::select`
4. Replace `apply_array_store_in_z3` with `Array::store`
5. Remove version tracking, frame axioms, StoreRecord infrastructure

**Files:** `memory.rs`, `array_tracker.rs`, `z3_stub.rs`
**Effort:** 2-3 days (research-heavy)

## Phase 5: Auto-Invariant Inference

**Goal:** Extend `try_infer_while_invariant` to handle array access patterns. Current version only handles `while i < N { i = i + 1 }`. Add array patterns.

**Tasks:**
1. Pattern: `while i < N && arr[i] > key` → infer `invariant i >= 0 && arr[i-1] <= arr[i]`
2. Pattern: `for i in 0..n { arr[i] = v }` → infer `invariant forall k in 0..(i-1): arr[k] == v`
3. Wire into while-loop and for-loop emitters

**Files:** `while_stmt.rs`, `for_loop_emit.rs`
**Effort:** 2-3 days

## Phase 6: Documentation & Blog

**Goal:** Publish the verification story.

**Tasks:**
1. Blog post: "Proving Sorting Algorithms at Compile Time" — based on `docs/blog/forall-invariants.md`
2. Update BENCHMARKS_E2E.md with algorithm verification coverage
3. Add tutorial chapter: "Verifying Your First Algorithm"
4. Update README with verification badge/metrics example

**Effort:** 1-2 days
