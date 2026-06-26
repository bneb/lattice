# Z3 Contract Verification — Capabilities and Limits

Empirically verified against the Salt compiler (2026-06-26).

Contracts must be on `pub` functions in `--lib` mode. Non-pub functions
skip verification silently.

## What Z3 Proves (UNSAT → check elided, zero runtime cost)

**Integer bounds at call sites with constant arguments:**

```salt
pub fn checked_get(arr: &[i32; 10], idx: i32) -> i32
    requires(idx >= 0 && idx < 10)
{ return arr[idx as i64]; }

fn main() { checked_get(&data, 5); }  // Z3 proves 0 <= 5 < 10
```

**Division and modulus safety:**

```salt
pub fn safe_div(a: i32, b: i32) -> i32
    requires(b != 0)
{ return a / b; }

fn main() { safe_div(100, 7); }  // Z3 proves 7 != 0
```

**Compound bounds (all comparison operators):**

```salt
pub fn bounded_add(a: i32, b: i32) -> i32
    requires(a >= 0 && a <= 10 && b >= 0 && b <= 10)
    ensures(result >= 0 && result <= 20)
{ return a + b; }
```

**Multiplication with bounded inputs:**

```salt
pub fn multiply_bounded(a: i32, b: i32) -> i32
    requires(a >= 0 && a <= 10 && b >= 0 && b <= 10)
    ensures(result >= 0 && result <= 100)
{ return a * b; }
```

**Multiplication with unbounded inputs (sign only):**

```salt
pub fn multiply_any(a: i32, b: i32) -> i32
    requires(a >= 0 && b >= 0)
    ensures(result >= 0)
{ return a * b; }  // Z3 proves non-negative * non-negative >= 0
```

**Postconditions across conditional branches:**

```salt
pub fn absolute(x: i32) -> i32
    ensures(result >= 0)
{
    if x < 0 { return -x; }   // Z3 proves: x < 0 → -x >= 0
    return x;                   // Z3 proves: x >= 0 → x >= 0
}
```

**Nested conditionals (clamp):**

```salt
pub fn clamp(val: i32, lo: i32, hi: i32) -> i32
    requires(lo <= hi)
    ensures(result >= lo && result <= hi)
{
    if val < lo { return lo; }
    if val > hi { return hi; }
    return val;
}
// Z3 proves the postcondition for all three return paths
```

**Bitwise operations with bounded inputs:**

```salt
pub fn bitwise_range(x: i32) -> i32
    requires(x >= 0 && x <= 255)
    ensures(result >= 0 && result <= 255)
{ return x & 0xFF; }  // Z3 proves AND with 0xFF stays in [0, 255]

pub fn bitwise_or_min(x: i32) -> i32
    requires(x >= 0)
    ensures(result >= x)
{ return x | 0x0F; }  // Z3 proves OR always increases magnitude
```

**Logical implication through branches:**

```salt
pub fn implies_test(x: i32) -> i32
    requires(x >= 0)
    ensures(result >= 0)
{
    if x > 10 { return x; }  // Z3 proves: x > 10 → x >= 0
    return 10;                 // Z3 proves: 10 >= 0
}
```

**Division chains (multiple preconditions):**

```salt
pub fn div_chain(a: i32, b: i32, c: i32) -> i32
    requires(b != 0 && c != 0)
{ return (a / b) / c; }
// Z3 proves both divisors non-zero at call sites with constants
```

## What Z3 Rejects (SAT → compile error with counterexample)

When all arguments are compile-time constants that violate the contract,
Z3 reports the specific values and stops compilation:

```
VERIFICATION ERROR: could not prove '(not (= 0 0))'
  context: precondition check
  counterexample:
    a = 100
    b = 0
```

**Rejection examples tested and confirmed:**

| Call | Contract | Counterexample |
|------|----------|----------------|
| `safe_div(x, 0)` | `requires(b != 0)` | `b = 0` |
| `needs_positive(0)` | `requires(x > 0)` | `x = 0` |
| `needs_positive(-5)` | `requires(x > 0)` | `x = -5` |
| `bounded_add(15, 5)` | `requires(a <= 10)` | `a = 15` |
| `clamp(v, 100, 0)` | `requires(lo <= hi)` | `lo = 100, hi = 0` |
| `broken_ensures` returning -1 | `ensures(result >= 0)` | `x = 0` |

## Runtime Assertions (TIMEOUT / UNKNOWN)

When arguments are not compile-time constants, or when Z3 cannot decide
within 100ms, the compiler emits a runtime assertion (`@__salt_contract_violation`).
The program compiles but panics at runtime if the contract is violated.

## What Z3 Cannot Handle

- **Non-linear integer arithmetic**: `a * b` where both `a` and `b` are
  unbounded variables. Z3 can reason about `a * b` when at least one
  operand is bounded (constant or range-constrained).
- **Floating-point**: Z3's float theory is incomplete. Use integer
  arithmetic for contract properties.
- **String operations**: Not in the solver domain.
- **Contracts on non-`pub` functions** in `--lib` mode: silently skipped.

## How to Use

```bash
# Verification is active by default (no --verify flag needed)
salt-front program.salt --lib --disable-alias-scopes -o /dev/null

# Explicitly disable for fast iteration
salt-front program.salt --lib --disable-alias-scopes --danger-no-verify -o /dev/null
```

The `--verify` flag is a no-op — verification runs whenever `--danger-no-verify`
is not passed.
