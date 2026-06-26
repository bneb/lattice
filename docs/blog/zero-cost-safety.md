# How Salt Eliminates Runtime Checks You Didn't Write

**Published:** June 2026 | **Author:** The KeuOS Team | **Reading time:** 12 minutes

---

Here is a function that indexes a 256-element lookup table. It takes a
`u8` — an unsigned byte, range 0 to 255. It has a bounds check:

```salt
pub fn lookup(table: &[i32; 256], idx: u8) -> i32
    requires(idx < 256)
{ return table[idx as i64]; }
```

Call it with a runtime variable — no constant, no literal:

```salt
let idx: u8 = some_runtime_value();
let result = lookup(&table, idx);
```

A conventional compiler emits `cmp idx, 256; jae panic`. Salt emits
nothing. The bounds check does not exist in the binary.

Why? Because the compiler knows `u8` ∈ [0, 255]. Before codegen, it asks: "can any value of type `u8` violate `idx < 256`?" The answer is no.
The check is mathematically redundant. It evaporates.

You didn't have to prove it. You didn't have to annotate it. The type
system proved it for you.

---

## Two Tiers, Zero Overhead

Salt's contract verification runs in two tiers:

**Tier 1: Compile-time evaluation.** Before Z3 ever sees a contract, the
constant folder attempts to resolve it using the compiler's built-in
evaluator. If the expression reduces to `true`, Z3 is skipped entirely.
This handles string operations on literal arguments, integer arithmetic
on constants, and anything the compiler can evaluate without a solver.

**Tier 2: Z3 symbolic proof.** For contracts with symbolic (runtime)
parameters, the compiler translates the expression to a Z3 formula and
checks satisfiability. The solver has 100ms per obligation. In
empirical testing, Z3 resolves every contract we have thrown at it —
including 10-variable polynomial constraints — within that window.

If Z3 proves the contract, the check is elided from the binary. If Z3
finds a counterexample, the compiler stops with the specific violating
values. If Z3 times out, a runtime assertion is emitted as a safe
fallback.

---

## The Type System Is a Proof System

Every integer type carries bounds that the solver receives as hard
constraints. You don't opt into this. It's automatic.

| Type | Constraint | Contract | Proved because |
|------|-----------|----------|---------------|
| `u8` | [0, 255] | `requires(idx < 256)` | 255 < 256 |
| `u16` | [0, 65535] | `requires(idx < 65536)` | 65535 < 65536 |
| `u32` | ≥ 0 | `requires(x >= 0)` | type guarantees it |
| `i8` | [-128, 127] | `requires(x >= -128)` | type guarantees it |
| `bool` | {0, 1} | `requires(b == 0 \|\| b == 1)` | exhaustive |

These compose via AND with whatever contracts you write. A
`requires(idx < 100)` on `u8` gives Z3 the effective bound
`idx ∈ [0, 99]`. Tighter constraints from either source only help
the proof.

This is not a special case for `u8`. It's the general mechanism: the
compiler extracts the domain of every integer type and asserts it into
the solver before checking any contract.

---

## Z3 Handles the Hard Cases

When the type system isn't enough, Z3 takes over. Here is a function
with a postcondition that depends on the input's sign:

```salt
pub fn my_abs(x: i32) -> i32
    ensures(result >= 0)
{
    if x < 0 { return -x; }    // Z3 proves: x < 0 → -x >= 0
    return x;                   // Z3 proves: x >= 0 → x >= 0
}
```

Z3 tracks path conditions through every branch. It knows that after
`if x < 0`, the else branch executes with `x >= 0`. Each return site
is verified independently.

Multiplication, division safety, bitwise operations, and 10-variable
polynomial constraints all resolve within the 100ms window. Z3
handles non-linear integer arithmetic — it's not limited to linear
constraints.

---

## String Validation at Compile Time

String operations on literal arguments never reach Z3. They're
evaluated in Rust at compile time:

```salt
pub fn validate_url(url: StringView) -> bool
    requires(url.starts_with("https://"))
    requires(url.contains(".com"))
    requires(url.ends_with("/api/v1/"))
{ return true; }

// Called with a literal:
validate_url("https://salt-lang.com/api/v1/");
```

Three string operations, all resolved before codegen. The constant
folder substitutes `"https://salt-lang.com/api/v1/"` for `url`, then
evaluates `.starts_with("https://")` → `true` using Rust's standard
library. Z3 never runs.

For symbolic (runtime) strings, the compiler can translate these
operations to Z3-str's native solver but cannot yet prove them — the
substitution mechanism currently handles only `Int`-typed parameters.
Extending it to Z3 `String` types is in progress.

---

## What Ships, What Doesn't

**Proved at compile time, zero runtime cost:**
- Integer bounds and comparisons (all six operators)
- Division and modulus safety
- Multiplication (including polynomial: `x*x + y*y`)
- Float zero-checking (`requires(b != 0.0)`)
- Postconditions across conditional branches
- String length, prefix, suffix, and containment (with literal args)
- Type-bound proofs for all integer types
- Bitwise AND/OR with constants

**Runtime assertion (Z3 can't decide, safe fallback):**
- Contracts with unbounded symbolic parameters not implied by type bounds

**Not yet wired (Z3 supports, bridge pending):**
- Full `Real` exact-rational arithmetic
- `BV` bitvector reasoning
- `forall`/`exists` quantifiers (no Salt syntax)

---

## The Proposition

You write types. You write a few `requires` clauses on the boundaries.
The compiler proves what it can, runtime-checks the rest, and tells you
exactly which is which.

No separate verification tool. No annotation language. No proof
assistant. Just a compiler that understands your types and acts on them.

[Try the tutorial →](/docs/tutorial/your-first-verified-program.md)
