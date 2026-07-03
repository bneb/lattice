# Verification Sprint — Closing the Gap

## Completed (shipped)

- `forall i in lo..hi => body` syntax with constant expansion + Z3 ForAll fallback
- For-loop invariants: base case (i==start pinning) + inductive step (i→i+1 substitution)
- Concrete for-loop unrolling: when bounds are constants, 100% proof coverage
- While-loop invariants: base case + inductive step (Havoc semantics)
- Array store tracking: versioned UFs + update assertions + bounded frame axioms
- Body scanning: recursive AST walker for indexed assignments
- Z3 metrics: `Z3: 8/8 checks proven (100%), 0 deferred to runtime`
- `ensures forall` for postconditions on array contents
- Red-team fixes: honest claims, no @trusted bypass, UNSAFE.md accuracy

## Remaining (in priority order)

### Phase 1: CI stability (now)
**All CI runs are failing.** Need to identify root cause — likely Clippy lint on CI platform vs local.
**Effort**: 1-2 hours. **Impact**: unblocks all future work.

### Phase 2: Case-splitting for data-dependent loops (1-2 days)
The key missing piece. Insertion sort's inner while-loop (`while j >= 0 && arr[j] > key`) has data-dependent iterations. Fix: assert `(j < 0) || (arr[j] <= key) && j >= 0` as a post-condition, split the proof into two cases, prove each separately in Z3.
**Effort**: 1-2 days. **Impact**: insertion sort fully verified. Unlocks all algorithms with conditional inner loops.

### Phase 3: Z3 native Array theory (2-3 days)
Replace versioned UFs with Z3's `store`/`select`. Eliminates manual frame axioms. z3-0.12 crashes — need to debug or upgrade to z3-0.20 (377 API breakages to fix).
**Effort**: 2-3 days. **Impact**: simpler code, faster proofs, no frame axiom overhead.

### Phase 4: Algorithm verification suite (1-2 days)
Write verified implementations of:
- Bubble sort (fixed loops, forall invariants)
- Selection sort (fixed loops, forall invariants)
- Matrix multiply (tiled, @ operator)
- Binary search (while-loop, requires clauses)
- Array fill (forall ensures)
**Effort**: 1-2 days. **Impact**: demonstrable verification coverage, blog material.

### Phase 5: `exists` quantifier (1 day)
Add `exists i in lo..hi => body` syntax. Same expansion pattern as forall. Needed for search algorithm postconditions.
**Effort**: 1 day. **Impact**: completes quantifier story.

### Phase 6: Auto-invariant inference (2-3 days)
Extend `try_infer_while_invariant` to handle array access patterns. Current version only handles `while i < N { i = i + 1 }`. Add: `while i < N && arr[i] > key` → `invariant i >= 0 && forall k in 0..i: arr[k] >= key`.
**Effort**: 2-3 days. **Impact**: reduces annotation burden for common patterns.
