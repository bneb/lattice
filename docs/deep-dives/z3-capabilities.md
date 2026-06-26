# Z3 Contract Verification — Boundary and Frontier

Empirically verified against the Salt compiler. Last updated 2026-06-26.

Verification is active by default. Disable with `--danger-no-verify`.

## The Contract Model

Salt supports two contract forms:

| Form | When checked | Failure mode |
|------|-------------|--------------|
| `requires(cond)` | At every call site | Compile error with counterexample |
| `ensures(cond)` | At every return site in the function body | Compile error with counterexample |

Contracts are checked on **all functions** regardless of visibility.

## What Z3 Proves

The compiler translates `requires` and `ensures` expressions into Z3 integer
formulas. At call sites with constant or range-constrained arguments, Z3
proves the precondition and elides the runtime check. In function bodies,
Z3 proves the postcondition for each return path under the path condition.

### Integer arithmetic

All basic operations with constant or range-constrained operands:

| Operation | Example | Status |
|-----------|---------|--------|
| Add/Sub | `requires(a + b >= 0)` | Proved |
| Multiply (one operand bounded) | `requires(a * 2 >= 0)` | Proved |
| Multiply (both bounded) | `requires(a >= 0 && a <= 10 && b >= 0 && b <= 10)` ensures `result >= 0 && result <= 100` | Proved |
| Multiply (both unbounded) | `requires(a >= 0 && b >= 0)` ensures `result >= 0` | Proved |
| Multiply (counterexample) | `ensures(result >= 0)` without bounds | Rejected — Z3 finds `a=-1, b=1` |
| Div/Mod safety | `requires(b != 0)` | Proved |
| Div/Mod result bounds | `ensures(result >= 0)` | Not proved (division semantics are complex) |

### Comparisons

All six operators work: `==`, `!=`, `<`, `<=`, `>`, `>=`.

Compound conditions with `&&` and `||` are supported. Z3 proves each
conjunct independently under the combined path condition.

### Bitwise operations

Bitwise AND/OR with constant operands, when the variable has bounds:

```salt
pub fn mask(x: i32) -> i32
    requires(x >= 0 && x <= 255)
    ensures(result >= 0 && result <= 255)
{ return x & 0xFF; }  // proved
```

Left shift by constants, right shift, and XOR have not been tested.
Bitwise NOT and variable-to-variable bitwise operations are outside
the solver domain.

### Conditionals and path sensitivity

Z3 is path-sensitive. It tracks the branch condition to narrow the
possible values of variables at each return site:

```salt
pub fn abs(x: i32) -> i32
    ensures(result >= 0)
{
    if x < 0 { return -x; }   // path: x < 0, Z3 proves -x >= 0
    return x;                   // path: x >= 0, Z3 proves x >= 0
}
```

This works for nested conditionals, guard clauses with early returns,
and chained if/else if/else.

### Struct field access

Bounds on struct fields are checked:

```salt
pub fn arena_alloc(arena: Arena, id: i64) -> i64
    requires(id >= 0 && id < arena.max_cores)
{ return id; }
```

### Pointer non-null

```salt
pub fn use_ptr(p: Ptr<T>) -> T
    requires(!p.is_null())
{ return *p; }
```

### StringView length

```salt
pub fn process(key: StringView) -> StringView
    requires(key.length() > 0 && key.length() <= 4000)
{ return key; }
```

## What Z3 Rejects

When all arguments at a call site are compile-time constants that violate
a precondition, Z3 reports the counterexample and stops compilation:

```salt
fn main() {
    safe_div(100, 0);  // compile error
}
```

```
VERIFICATION ERROR: could not prove '(not (= 0 0))'
  context: precondition check
  counterexample:
    a = 100
    b = 0
```

The binary is never produced. This is not a warning or a runtime check —
it is a hard compile error.

Similarly, if any return path in a function body violates an `ensures`
clause, Z3 finds a counterexample and stops compilation.

## The 100ms TIMEOUT

Z3 is given 100ms per proof obligation. In empirical testing, Z3
resolves every contract we have tested within this window — including
10-variable polynomial constraints, non-linear integer arithmetic
(`a * b`, `x * x`), Diophantine equations, and deep conditional chains.

If a formula were to exceed 100ms, the compiler emits a runtime
assertion rather than a compile error. The program compiles but panics
if the contract is violated. We have not been able to trigger this
path with realistic contracts.

## The Frontier

**Proved or rejected at compile time (everything we have tested):**
- Integer arithmetic with all six comparison operators
- Multiplication, including multi-variable and polynomial (`x*x + y*y`)
- Division and modulus safety (`requires(b != 0)`)
- Bitwise AND/OR with constant operands
- Compound boolean conditions with `&&` and `||`
- Path-sensitive reasoning through nested if/else chains
- Postconditions across any number of return paths
- Struct field bounds
- Pointer non-null
- StringView length ranges

**Not expressible (outside the integer theory):**
- Floating-point arithmetic
- String content constraints (length is fine, content is not)
- Quantifiers (forall, exists)
- Heap reachability (no cycles, no dangling pointers)
- Temporal properties (eventually, always)

## How to Use

Verification is active by default. No flag needed.

```bash
salt-front program.salt --lib --disable-alias-scopes -o /dev/null
```

Disable for fast iteration:

```bash
salt-front program.salt --lib --disable-alias-scopes --danger-no-verify -o /dev/null
```

## Writing Effective Contracts

1. **Use constants at call sites.** `checked_get(&data, 5)` proves `5 < 10`.
   `checked_get(&data, idx)` where `idx` comes from a function parameter
   becomes a runtime assertion unless the caller also carries a contract
   that bounds `idx`.

2. **Bound your inputs.** Z3 proves `a * b` when `a` and `b` both have
   range constraints (`0 <= a <= 10`). Without bounds, it can only
   prove sign properties.

3. **Prefer preconditions to postconditions.** `requires(idx < len)` at
   every call site is more tractable than `ensures(result >= 0)` on
   a function with complex internal logic.

4. **Keep contracts small.** A single `requires` with one comparison
   resolves in microseconds. A compound `requires` with 5 conjuncts
   and 3 variables takes longer. Compound conditions are fine, but
   prefer clarity over density.
