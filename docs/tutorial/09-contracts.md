# Chapter 9: Z3 Contracts

## Zero-Cost Formal Verification

Salt's defining feature: the **Z3 theorem prover** is embedded directly in the compiler. Contracts that Z3 can prove have **zero runtime cost** — the check is elided entirely. Contracts Z3 cannot prove (timeout within 100ms, or genuinely ambiguous symbolic inputs) emit a runtime assertion as a safe fallback. The set of provable cases expands over time as the solver heuristics and proof tactics improve.

## `requires` — Preconditions

A `requires` clause on a function specifies the conditions that must be true at every call site. The compiler proves these at compile time:

```salt
package main

fn safe_div(a: i32, b: i32) -> i32
    requires(b != 0)
{
    return a / b;
}

fn main() -> i32 {
    let x = safe_div(100, 7);    // ✓ Z3 proves 7 != 0 — check elided
    println(f"100/7 = {x}");

    // let y = safe_div(100, 0); // ✗ COMPILE ERROR: Z3 finds counterexample b=0
    return 0;
}
```

When you call `safe_div(100, 7)`, Z3 proves `7 != 0` is always true, so the check evaporates — the generated binary contains no branch, no assertion, no overhead. When you call `safe_div(100, 0)`, the compiler reports:

```
VERIFICATION ERROR: could not prove '(b != 0)'
  context: precondition check at call site
  counterexample:
    b = 0
  hint: the argument 'b' must be non-zero
```

## `ensures` — Postconditions

An `ensures` clause specifies what must be true about the return value. Z3 verifies this at every `return` site using **Weakest Precondition** generation:

```salt
package main

fn absolute_value(x: i32) -> i32
    ensures(result >= 0)
{
    if x < 0 {
        return -x;    // Z3 proves: given x < 0, -x >= 0  ✓
    }
    return x;         // Z3 proves: given !(x < 0), x >= 0  ✓
}

fn clamp_to_range(val: i32) -> i32
    ensures(result >= 0 && result <= 100)
{
    if val < 0   { return 0; }
    if val > 100 { return 100; }
    return val;
    // Z3 proves: given !(val < 0) && !(val > 100), 0 <= val <= 100  ✓
}

fn main() -> i32 {
    let a = absolute_value(-42);    // ensures(a >= 0) — proven
    let c = clamp_to_range(150);    // ensures(c >= 0 && c <= 100) — proven
    println(f"abs(-42)={a}, clamp(150)={c}");
    return 0;
}
```

Every `return` site becomes a Z3 proof obligation. Guard clauses with early returns automatically narrow the path conditions — Z3 knows that code after `if x < 0 { return -x; }` executes only when `x >= 0`.

## How It Works: Proof-or-Panic

The verification follows a strict two-outcome protocol:

```
requires(b != 0)
    │
    ▼
Translate to Z3 formula: (assert (not (= b 0)))
    │
    ▼
Z3 checks satisfiability:
    │
    ├── UNSAT (no counterexample)
    │       → Condition ALWAYS holds
    │       → ELIDE CHECK — emit nothing
    │       → Zero runtime cost
    │
    ├── SAT (counterexample found: b = 0)
    │       → Condition can be violated
    │       → COMPILE ERROR with counterexample
    │
    └── UNKNOWN (Z3 timeout, 100ms)
            → Cannot determine
            → Emit runtime assertion as fallback
```

There is no third path. Every contract is either mathematically proven or runtime-enforced.

## Bounds Checking

Array access with Z3-verified bounds:

```salt
package main

fn get_element(arr: &[i32; 10], idx: i32) -> i32
    requires(idx >= 0 && idx < 10)
{
    return arr[idx as i64];
}

fn main() -> i32 {
    let data: [i32; 10] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];

    let v = get_element(&data, 5);  // ✓ Z3 proves 0 <= 5 < 10
    println(f"data[5] = {v}");

    // let bad = get_element(&data, 15);  // ✗ COMPILE ERROR
    return 0;
}
```

## Contracts on Kernel Operations

Z3 contracts are used throughout the KeuOS kernel for memory safety:

```salt
// Physical Memory Manager: prevents invalid ranges
pub fn init(start: u64, end: u64)
    requires(start < end)
{
    // Z3 proves start < end at every call site
    // ... initialize page allocator ...
}

// IPC shared memory: prevents wrap-around
fn map_ring(descriptor: RingDescriptor)
    requires(descriptor.size > 0 && descriptor.size <= 0x100000)
{
    // Z3 proves the ring is non-empty and under 1MB
    // ... map SPSC ring pages ...
}
```

## `@trusted` — Opting Out of Verification

For FFI wrappers and hand-audited code, `@trusted` skips Z3 verification:

```salt
package main

import std.core.ptr.Ptr

extern fn external_library_init(config: Ptr<u8>) -> i32;

@trusted  // We trust the external library's contract
fn init_library(config: Ptr<u8>) -> i32 {
    return external_library_init(config);
}
```

> **Rule**: Every `@trusted` function should have a comment explaining why verification is unnecessary or why the external dependency is trusted.

## Contracts in Practice

A realistic example combining contracts with error handling:

```salt
package main

import std.core.result.Result
import std.status.Status

fn binary_search(arr: &[i32], target: i32) -> Result<i32>
    requires(arr.len() > 0)
{
    let mut lo: i64 = 0;
    let mut hi: i64 = arr.len() - 1;

    while lo <= hi {
        let mid = lo + (hi - lo) / 2;
        if arr[mid] == target {
            return Result::Ok(mid as i32);
        }
        if arr[mid] < target {
            lo = mid + 1;
        } else {
            hi = mid - 1;
        }
    }
    return Result::Err(Status::from_code(-1));
}

fn main() -> i32 {
    let sorted: [i32; 5] = [1, 3, 5, 7, 9];

    let found = binary_search(&sorted, 5);
    // Z3 proves: arr.len() == 5 > 0 ✓
    // Z3 proves: all array accesses are in-bounds ✓

    match found {
        Result::Ok(idx) => println(f"found at index {idx}"),
        Result::Err(_) => println("not found"),
    }
    return 0;
}
```

## Forall Quantifier

The `forall` quantifier expresses properties over ranges of array elements:

```salt
package main

fn array_fill(arr: Ptr<i32>, value: i32, n: i64)
    requires n > 0
    ensures forall i in 0..(n-1) => arr[i] == value
{
    for i in 0..n {
        unsafe { arr[i] = value; }
    }
}
```

When the range bounds are compile-time constants (e.g., `0..3`), the forall expands to concrete comparisons at the call site — no Z3 quantifier needed. For symbolic bounds, Z3's ForAll quantifier handles the proof.

```salt
// Constant bounds: expands to arr[0] == 5 && arr[1] == 5 && arr[2] == 5
array_fill(ptr, 5, 3);
```

## Loop Invariants

For-loops support `invariant` clauses. The compiler checks the invariant at entry (base case) and proves it's preserved by the body (inductive step):

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

When both loop bounds are compile-time constants, the compiler unrolls the loop at the Z3 level — each iteration is proved separately with concrete values. After compilation, Salt reports proof coverage:

```
Z3: 8/8 checks proven (100%), 0 deferred to runtime
```

## Verifying Sorting Algorithms

Array-content invariants use `forall` inside `invariant` clauses:

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

The outer loop invariant states "the prefix arr[0..i-1] is sorted." Z3 proves the base case (vacuously true at i=0) and the inductive step when the inner loop has fixed trip counts (like bubble sort). For data-dependent inner loops (like insertion sort's while-loop), the infrastructure is in place but full proof requires case-splitting on the loop condition — an active area of development.

## Compiler Flags

```bash
# Full verification (default) — emits Z3 coverage report
salt-front my_program.salt -o my_program

# Skip verification for fast iteration
salt-front --danger-no-verify my_program.salt -o my_program
```

## Summary

| Feature | Syntax | Purpose |
|---------|--------|---------|
| Precondition | `fn foo(x: T) requires(cond)` | Prove condition at every call site |
| Postcondition | `fn foo(x: T) -> R ensures(cond)` | Prove condition at every return site |
| Forall | `forall i in lo..hi => expr` | Quantified array property |
| Invariant | `invariant x > 0;` | Loop invariant (checked at entry + inductive step) |
| Trusted | `@trusted fn foo(...) { ... }` | Skip Z3 verification (FFI, hand-audited) |
| No-verify flag | `salt-front --danger-no-verify ...` | Skip verification for fast iteration |

---

## You've Completed the Tutorial

You now know Salt from basic syntax through Z3 formal verification. The language combines:

- **Safety** without lifetime annotations (arena allocation + Scope Ladder)
- **Certainty** with zero runtime cost (Z3 compile-time proofs)

### Next Steps

- Read the [Syntax Reference](../../SYNTAX.md) for every language construct
- Explore the [Standard Library](../../salt-front/std/README.md) (70+ modules)
- Study the [Architecture Decision Records](../adr/) for design rationale
- Check out the example projects: [Basalt](../../basalt/) (LLM inference), [Lettuce](../../lettuce/) (KV store), [Facet](../../user/facet/) (2D compositor)
- Contribute! Look for [`good-first-issue`](https://github.com/bneb/keuos/labels/good-first-issue) on GitHub
