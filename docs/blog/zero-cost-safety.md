# How Salt Eliminates Runtime Checks You Didn't Write

**Published:** June 2026 | **Reading time:** 14 minutes

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

Why? Because the compiler knows `u8` ∈ [0, 255]. Before codegen, it
asks Z3: "can any value of type `u8` violate `idx < 256`?" The answer
is no. The check is mathematically redundant. It evaporates.

You didn't prove it. You didn't annotate it. The type system proved it
for you.

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
empirical testing, Z3 resolves every contract thrown at it —
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

## Contracts Chain Across Calls

Preconditions compose. When a caller proves `x > 5` and passes `x` to a
callee requiring `x > 0`, the compiler knows `x > 5 → x > 0` and elides
the callee's check. No runtime work. No annotation propagation.

```salt
pub fn callee(x: i32) -> i32
    requires(x > 0)
{ return x; }

pub fn caller(x: i32) -> i32
    requires(x > 5)         // caller's precondition
{ return callee(x); }       // Z3 proves: x > 5 ⇒ x > 0, check elided
```

If the caller can't prove the callee's contract — say, `requires(x > 5)`
calling a function that needs `x > 10` — Z3 finds the counterexample
(`x = 6` satisfies the caller but violates the callee) and the compiler
reports the exact value. You fix the contract or the call site before
the binary exists.

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

## Exact Rational Arithmetic

Float literals in contracts use Z3's `Real` sort — exact rationals, not
IEEE 754 approximations. `3.14` becomes 157/50. No floating-point error.
No rounding artifacts.

```salt
pub fn gt_pi(x: f64) -> f64
    requires(x > 3.14)     // Z3 proves: x > 157/50
{ return x; }

// Call with 4.0: 4.0 > 157/50 → proven, check elided
// Call with 2.0: 2.0 > 157/50 → counterexample, compile error
```

The constant folder handles literals through Rust's `f64` parser,
converting to exact rational strings for Z3. Comparisons use Z3's
native real arithmetic. Integer operands in float contexts promote
automatically via `Real::from_int`.

---

## Bitwise Operations Through BV

Bitwise operators (`&`, `|`, `^`, `<<`, `>>`) translate through Z3's
bitvector theory. The compiler converts integer operands to 64-bit
bitvectors, applies the operation, and converts back — giving Z3
bit-precise semantics for operations that need it.

```salt
pub fn mask(x: i32) -> i32
    requires(x & 0xFF == x)    // Z3 proves via BV: x fits in 8 bits
{ return x; }
```

The type-bound mechanism still applies: if `x` is `u8`, the compiler
already knows `x ∈ [0, 255]`. The bitwise contract is redundant for
`u8` but meaningful for wider types.

---

## String Validation at Compile Time

String operations on literal arguments resolve at compile time. The
constant folder evaluates them in Rust before Z3 runs:

```salt
pub fn validate_key(key: StringView) -> bool
    requires(key.starts_with("salt-"))
    requires(key.contains("lang"))
    requires(key.ends_with(".salt"))
    requires(key.matches("^[a-z.-]+$"))
{ return true; }

// Called with a literal — all four checks resolve in Rust:
validate_key("salt-lang.salt");
```

For symbolic string parameters, Z3's string theory takes over. The
compiler translates the parameter to a Z3 `String` constant via
hash-consing — the same approach used for `Int` and `Real` substitution.
When a caller passes a concrete string, the substitution connects the
symbolic parameter to the literal value and Z3 proves the contract.

`.starts_with()`, `.ends_with()`, and `.contains()` use Z3's native
sequence prefix/suffix/containment operations. `.matches()` uses
Z3's `Regexp` sort for regex patterns on symbolic strings. String
length comparisons on literal arguments evaluate in Rust; symbolic
length comparisons use Z3's uninterpreted function with path condition
constraints from the caller.

---

## What Ships, What Doesn't

**Proved at compile time, zero runtime cost:**
- Integer bounds and comparisons (all six operators)
- Division and modulus safety
- Multiplication (including polynomial: `x*x + y*y`)
- Float comparisons via exact rational arithmetic (`Real` sort)
- Bitwise operations via bitvector theory (`BV` sort)
- Postconditions across conditional branches
- Type-bound proofs for all integer types and struct fields
- String length, prefix, suffix, containment, and regex (literal + symbolic)
- `forall` and `exists` quantifiers with constant expansion + Z3 fallback
- Loop invariants with base case + inductive step (for and while)
- Array store tracking with versioned frame axioms
- Cross-function contract chaining (callee ensures → caller solver)
- Case splitting for data-dependent loops

**Runtime assertion (Z3 can't decide, safe fallback):**
- Contracts with unbounded symbolic parameters not implied by type bounds
- Non-linear arithmetic with two symbolic variables (`x * y`)

---

## New in v1.2.0 (July 2026)

**Cross-function chaining.** When `f` calls `g`, the compiler now flows
`g`'s `ensures` into `f`'s Z3 solver. This means the caller can prove
properties that depend on the callee's guarantees:

```salt
fn negate(x: i64) -> i64
    ensures(result == 0 - x)
{ return 0 - x; }

fn double_negate(x: i64) -> i64
    ensures(result == x)           // Proven because Z3 knows a == -x
{ let a = negate(x); return negate(a); }
```

**Struct field type bounds.** Field accesses in contracts now carry their
type-domain constraints. `requires(p.x < 256)` on a struct with `x: u8`
is always true — Z3 knows `0 <= p.x <= 255`.

**39 contract regression tests** cover all verification modes. Full
capability/limitation tables at `docs/deep-dives/z3-contracts.md`.

---

## The Proposition

You write types. You write a few `requires` clauses on the boundaries.
The compiler proves what it can, runtime-checks the rest, and tells you
exactly which is which.

No separate verification tool. No annotation language. No proof
assistant. Just a compiler that understands your types and acts on them.

[Try the tutorial →](/docs/tutorial/your-first-verified-program.md)
