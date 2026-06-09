# Userspace Formal Verification in Salt

The Salt compiler includes a built-in verification engine backed by the Z3 SMT solver. This allows you to write mathematically proven constraints on your functions, guaranteeing properties like memory safety, absence of integer overflow, and precise bounds checking—all evaluated at compile time.

By utilizing `requires` and `ensures` clauses, userspace applications can achieve the same level of security and correctness as the KeuOS kernel.

## Contracts: `requires` and `ensures`

A contract establishes the rules for calling a function and the guarantees it provides when returning.

- **`requires`** (Pre-conditions): Conditions that MUST be true before the function is called. The compiler will check all call-sites to ensure the caller satisfies these conditions.
- **`ensures`** (Post-conditions): Conditions that MUST be true when the function returns. The compiler verifies the function body to ensure it honors these guarantees.

### Basic Example

```salt
/// Returns the absolute value of an integer.
fn abs(x: i64) -> i64 
    ensures result >= 0
{
    if x < 0 {
        salt_return -x;
    }
    salt_return x;
}

fn main() -> i32 {
    let a = -42;
    // The compiler formally proves that `b` will always be >= 0
    let b = abs(a); 
    return 0;
}
```

In this example:
1. `abs` provides an `ensures` clause promising its return value (`result`) is non-negative.
2. The Z3 solver analyzes the function's branches. In `x < 0`, it returns `-x` (which is positive). In `x >= 0`, it returns `x` (which is positive). Thus, the contract holds.
3. The caller (`main`) now has formal mathematical proof that `b >= 0`, without any runtime checks.

### Pre-conditions

To prevent invalid inputs, use a `requires` clause:

```salt
fn safe_divide(x: i64, y: i64) -> i64 
    requires y != 0
{
    salt_return x / y;
}
```

If you attempt to call `safe_divide(10, 0)`, the compiler will reject the program. If you call it with a variable, the compiler will trace the variable's constraints. If it cannot prove `y != 0`, you must add a runtime check (e.g. `if y != 0 { safe_divide(x, y) }`) to satisfy the prover.

## Complex Constraints

Constraints can express compound logic:

```salt
fn process_array(idx: i64, len: i64)
    requires idx >= 0 && idx < len
{
    // Safe to use idx within bounds of len
}
```

This is heavily utilized in systems like the Facet graphics compositor to ensure zero out-of-bounds pixel writes:

```salt
pub fn set_pixel(canvas: &mut Canvas, x: i32, y: i32)
    requires x >= 0 && x < canvas.width && y >= 0 && y < canvas.height
{
    // ...
}
```

## Running the Verifier

The formal verifier runs automatically during compilation when you use the Salt compiler (`salt-front`). It will output `Z3 CHECK PASSED` if all preconditions and postconditions are proven correct. If a constraint cannot be proven, the compiler will halt with an error, pointing you exactly to the unproven path.
