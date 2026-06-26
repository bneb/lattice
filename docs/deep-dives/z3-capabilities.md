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
| Multiply (both unbounded) | `requires(a >= 0 && b >= 0)` ensures `result >= 0` | Proved (sign only) |
| Multiply (fully unbounded) | `a * b` where neither has bounds | TIMEOUT — runtime assertion |
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

## What Becomes a Runtime Assertion

When Z3 cannot decide within the 100ms timeout per obligation, the
compiler emits a runtime assertion. The program compiles but panics
if the contract is violated at runtime.

This happens when:
- Arguments are not compile-time constants (e.g., function parameters
  passed through from a caller)
- Both operands of multiplication are unbounded variables
- The formula is too complex for Z3 to resolve within the timeout

## The Frontier

**Solvable (proved or rejected at compile time):**
- Integer arithmetic with at least one operand constant or bounded
- All comparison operators
- Compound boolean conditions
- Bitwise AND/OR with constants
- Path-sensitive reasoning through if/else chains
- Struct field bounds
- Pointer non-null
- StringView length ranges
- Postconditions across multiple return paths

**Not solvable (runtime assertion):**
- Fully unbounded integer multiplication (no bounds on either operand)
- Division result bounds (e.g., `a / b` where b is a variable)
- Floating-point operations
- String content constraints
- Loop invariants (the solver does not symbolically execute loops)
- Bitwise operations with two variable operands

**Not expressible (outside the integer theory):**
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
