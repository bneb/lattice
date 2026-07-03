# Forall Quantifiers and Loop Invariants in Salt

Salt's Z3 verification pipeline can now prove properties about array contents -- not just memory safety, but functional correctness. Here's what changed and how to use it.

## The `forall` quantifier

Contracts can now express properties over ranges of elements:

```salt
fn array_fill(arr: Ptr<i32>, value: i32, n: i64)
    requires n > 0
    ensures forall i in 0..(n-1) => arr[i] == value
{
    for i in 0..n {
        unsafe { arr[i] = value; }
    }
}
```

When the range bounds are compile-time constants, the forall expands to concrete comparisons at the call site:

```salt
array_fill(ptr, 5, 3);
// ensures expands to: arr[0] == 5 && arr[1] == 5 && arr[2] == 5
// Z3 proves all 3 conjuncts -- no quantifier needed
```

For symbolic bounds, Z3's native ForAll quantifier handles the proof. The compiler picks the right strategy automatically.

## Loop invariants with base case + inductive step

For-loops support `invariant` clauses. The compiler checks two things:

1. **Base case**: the invariant holds at loop entry (i == start)
2. **Inductive step**: if the invariant holds at iteration i, it must hold at iteration i+1

```salt
fn count_to_n(n: i64) -> i64
    requires n >= 0
{
    let mut sum: i64 = 0;
    for i in 0..n {
        invariant i >= 0;
        sum = sum + i;
    }
    return sum;
}
```

Both checks run at compile time. If either fails, you get a counterexample.

## Concrete unrolling

When both loop bounds are compile-time constants, Salt unrolls the loop at the Z3 level. Each iteration gets its own proof with concrete values:

```salt
fn write_four(arr: Ptr<i32>)
{
    for i in 0..4 {
        invariant i >= 0;
        unsafe { arr[i] = 0; }
    }
}
```

After compilation:

```
Z3: 8/8 checks proven (100%), 0 deferred to runtime
```

That's 4 iterations × 2 checks each (base case + inductive step). All proven. Zero runtime cost.

## Array-content invariants for sorting

The forall quantifier works inside invariant clauses, enabling sorting algorithm verification:

```salt
fn bubble_sort(arr: Ptr<i32>, n: i64)
    requires n > 0
    ensures forall i in 0..(n-1) => arr[i] <= arr[i+1]
{
    for i in 0..n {
        invariant forall k in 0..(i-1) => arr[k] <= arr[k+1];
        // ... bubble pass ...
    }
}
```

The invariant states "the prefix arr[0..i-1] is sorted." Z3 proves this holds at entry (vacuously true at i=0) and is preserved by each iteration when the inner loop has a fixed trip count.

## What's provable today

| Algorithm | Loop structure | Provable? |
|-----------|---------------|-----------|
| Array fill | 1 for-loop, fixed bounds | 100% (concrete unrolling) |
| Bubble sort | 2 nested for-loops, fixed bounds | Proven for concrete sizes |
| Selection sort | 2 nested for-loops, fixed bounds | Proven for concrete sizes |
| Matrix multiply | 3 nested for-loops, fixed bounds | Proven for concrete sizes |
| Insertion sort | for + while (data-dependent) | Base case proven, inductive step needs case-splitting |

The gap for data-dependent loops (insertion sort's inner while-loop, binary search) is case-splitting on the loop condition -- infrastructure is in place, not yet wired.

## Proof coverage metrics

Every compilation reports what was proven:

```
Z3: 8/8 checks proven (100%), 0 deferred to runtime
```

You always know exactly how much of your code's safety is compile-time guaranteed vs. runtime-enforced.

## How it works

Under the hood, Salt tracks arrays as versioned Z3 uninterpreted functions. Each indexed store (`arr[i] = v`) creates a new function version with an update assertion. The frame axiom (all other elements unchanged) is either expanded concretely when the bounds are known or emitted as a bounded Z3 ForAll.

The compiler also tracks while-loop exit conditions (`j < 0 || arr[j] <= key`), which constrain loop variables to their post-loop values, narrowing the set of modified indices for the frame axiom to reason about.

## Try it

```bash
git clone https://github.com/bneb/lattice.git
cd lattice && make build
salt-front tests/z3_contracts/test_bubble_sort.salt --lib -o /dev/null
# Output: Z3: 8/8 checks proven (100%), 0 deferred to runtime
```
