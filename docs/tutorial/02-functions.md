# Chapter 2: Functions, Contracts, and Verification

## Function Basics

Functions are the building blocks of Salt programs. A function takes parameters, optionally returns a value, and uses `return` to return:

```salt
package main

fn add(a: i32, b: i32) -> i32 {
    return a + b;
}

fn greet(name: StringView) {
    // No arrow + type = returns nothing (unit)
    println(f"Hello, {name}!");
}

fn main() -> i32 {
    let sum = add(3, 4);
    greet("world");
    println(f"3 + 4 = {sum}");
    return 0;
}
```

Parameters are declared `name: Type`. The return type follows `->`. Functions without a return type omit the arrow entirely. By default, every parameter is immutable -- there is no `mut` on parameters.

## Preconditions with `requires`

A `requires` clause states what must be true at every call site. The Z3 theorem prover embedded in the compiler checks this at compile time:

```salt
fn safe_div(a: i32, b: i32) -> i32
    requires(b != 0)
{
    return a / b;
}

fn main() -> i32 {
    let x = safe_div(100, 7);    // Z3 proves 7 != 0 -- check elided
    // let y = safe_div(100, 0); // COMPILE ERROR: counterexample b = 0
    println(f"100/7 = {x}");
    return 0;
}
```

When you call `safe_div(100, 7)`, Z3 proves the condition holds and the check is **elided at compile time** -- zero instructions, zero branches, zero runtime cost. When you pass `0`, the compiler reports:

```
VERIFICATION ERROR: could not prove '(b != 0)'
  counterexample:
    b = 0
  hint: the argument 'b' must be non-zero
```

## Postconditions with `ensures`

An `ensures` clause states what must be true about the return value. Z3 verifies this at every `return` site:

```salt
fn absolute_value(x: i32) -> i32
    ensures(result >= 0)
{
    if x < 0 {
        return -x;    // Z3: given x < 0, -x >= 0
    }
    return x;          // Z3: given !(x < 0), x >= 0
}

fn main() -> i32 {
    let a = absolute_value(-5);
    println(f"abs(-5) = {a}");
    return 0;
}
```

The special name `result` refers to the return value. Every `return` becomes a proof obligation. Guard clauses narrow the path conditions automatically.

## Three Outcomes of Verification

Every contract check has exactly one outcome:

```
requires(b != 0)
    |
    v
Z3 checks satisfiability:
    |
    |-- UNSAT (no counterexample found)
    |       Condition ALWAYS holds
    |       ELIDE CHECK -- zero runtime cost
    |
    |-- SAT (counterexample found, e.g. b = 0)
    |       Condition CAN be violated
    |       COMPILE ERROR with full counterexample
    |
    |-- UNKNOWN (Z3 timeout at 100ms)
            Cannot determine statically
            Emit runtime assertion as safe fallback
```

**UNSAT** is the goal -- your proof is complete and costs nothing. **SAT** means a bug exists at the call site; the compiler shows you the exact values that violate the contract. **UNKNOWN** means Z3 couldn't finish within its 100ms budget; a runtime assertion guards the operation instead.

## Putting It Together: Binary Search

This example combines `requires` and `ensures` to verify a binary search:

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
    // Z3 proves: arr.len() == 5 > 0
    // Z3 proves: all array accesses are in-bounds

    match found {
        Result::Ok(idx) => println(f"found at index {idx}"),
        Result::Err(_)  => println("not found"),
    }
    return 0;
}
```

The `requires(arr.len() > 0)` precondition ensures Z3 proves the array is non-empty at every call site. Z3 also walks every path through the loop and proves no `arr[mid]` access goes out of bounds.

## Summary

| Concept | Syntax | Purpose |
|---------|--------|---------|
| Function | `fn name(params) -> R { }` | Declare a function |
| Precondition | `requires(condition)` | Prove condition at call sites |
| Postcondition | `ensures(condition)` | Prove condition at return sites |
| Return value | `return expr;` | Return a value from a function |

Next: [Chapter 3: Structs, Enums, and Pattern Matching](03-structs-enums.md)
