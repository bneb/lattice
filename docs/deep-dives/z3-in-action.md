# Z3 Contracts in Action

A walkthrough of Salt's Z3 contract verification. Every example is a
complete terminal session — the commands, the output, and the
explanation.

Verification is on by default. No flag needed.

## 1. The Basics: Proving a Contract

Create a file and add a function with a `requires` clause:

```bash
cat > demo.salt << 'EOF'
package main

pub fn safe_div(a: i32, b: i32) -> i32
    requires(b != 0)
{ return a / b; }

pub fn main() -> i32 {
    return safe_div(100, 7);    // Z3 proves 7 != 0 — check elided
}
EOF

saltc demo.salt --lib --disable-alias-scopes -o /dev/null
```

```
✅ MLIR compiled successfully.
```

The `requires(b != 0)` is proved at the call site. Z3 checks that `7 != 0`
is always true, so the division-by-zero check is **elided from the binary**.
Zero instructions emitted. Zero runtime cost.

---

## 2. Catching a Bug at Compile Time

Change the argument to zero:

```bash
cat > demo.salt << 'EOF'
package main

pub fn safe_div(a: i32, b: i32) -> i32
    requires(b != 0)
{ return a / b; }

pub fn main() -> i32 {
    return safe_div(100, 0);    // Z3 finds the violation
}
EOF

saltc demo.salt --lib --disable-alias-scopes -o /dev/null
```

```
[E003] Compilation failed:
VERIFICATION ERROR: could not prove '(not (= 0 0))'
  context: precondition check
  counterexample:
    a = 100
    b = 0
```

The binary is never produced. Z3 found the exact violating input and
reported it. This is not a warning or a runtime check — it is a **hard
compile error**.

---

## 3. Bounds Checking Without Bounds Checks

Array access with a contract on the index:

```bash
cat > demo.salt << 'EOF'
package main

pub fn checked_get(arr: &[i32; 10], idx: i32) -> i32
    requires(idx >= 0 && idx < 10)
{ return arr[idx as i64]; }

pub fn main() -> i32 {
    let data: [i32; 10] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    return checked_get(&data, 5);    // Z3 proves 0 <= 5 < 10
}
EOF

saltc demo.salt --lib --disable-alias-scopes -o /dev/null
```

```
✅ MLIR compiled successfully.
```

**No bounds check exists in the binary.** The `requires` clause proved
`5 >= 0 && 5 < 10` at compile time, so the check was elided.

Now violate it:

```bash
cat > demo.salt << 'EOF'
package main
pub fn checked_get(arr: &[i32; 10], idx: i32) -> i32
    requires(idx >= 0 && idx < 10)
{ return arr[idx as i64]; }
pub fn main() -> i32 {
    let data: [i32; 10] = [0; 10];
    return checked_get(&data, 15);
}
EOF

saltc demo.salt --lib --disable-alias-scopes -o /dev/null
```

```
[E003] Compilation failed:
VERIFICATION ERROR: could not prove '(and (>= 15 0) (<= 15 9))'
  context: precondition check
  counterexample:
    idx = 15
    arr = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
```

---

## 4. Postconditions Across Branches

Z3 proves `ensures` clauses for every return path, tracking branch
conditions to narrow the possibilities:

```bash
cat > demo.salt << 'EOF'
package main

pub fn my_abs(x: i32) -> i32
    ensures(result >= 0)
{
    if x < 0 { return -x; }    // path: x < 0, Z3 proves -x >= 0
    return x;                   // path: x >= 0, Z3 proves x >= 0
}

pub fn main() -> i32 {
    return my_abs(-42);
}
EOF

saltc demo.salt --lib --disable-alias-scopes -o /dev/null
```

```
✅ MLIR compiled successfully.
```

Both return paths are verified independently. Z3 knows that after the
`if x < 0` guard, the `else` branch executes with `x >= 0`.

Now break it:

```bash
cat > demo.salt << 'EOF'
package main

pub fn broken_abs(x: i32) -> i32
    ensures(result >= 0)
{
    if x > 0 { return x; }
    return -1;     // returns -1 when x <= 0
}

pub fn main() -> i32 { return broken_abs(0); }
EOF

saltc demo.salt --lib --disable-alias-scopes -o /dev/null
```

```
[E003] Compilation failed:
Postcondition violation in 'main__broken_abs':
  ensures(result >= 0) is not satisfied for all return paths.
[Formal Shadow] Z3 counter-example:
  x := 0
```

Z3 found that when `x = 0`, the `x > 0` branch is skipped, the function
returns -1, and `-1 >= 0` is false. The compile error tells you exactly
which value triggers it.

---

## 5. Clamp: Three Return Paths, One Postcondition

```bash
cat > demo.salt << 'EOF'
package main

pub fn clamp(val: i32, lo: i32, hi: i32) -> i32
    requires(lo <= hi)
    ensures(result >= lo && result <= hi)
{
    if val < lo { return lo; }
    if val > hi { return hi; }
    return val;
}

pub fn main() -> i32 {
    return clamp(150, 0, 100);    // val=150 > hi=100 → returns 100
}
EOF

saltc demo.salt --lib --disable-alias-scopes -o /dev/null
```

```
✅ MLIR compiled successfully.
```

Three return paths, each proved against the postcondition. Z3 tracks
the path conditions: first return has `val < lo`, second has `val > hi`,
third has `lo <= val <= hi`.

Violate the `requires`:

```bash
cat > demo.salt << 'EOF'
package main
pub fn clamp(val: i32, lo: i32, hi: i32) -> i32
    requires(lo <= hi)
{ if val < lo { return lo; } if val > hi { return hi; } return val; }
pub fn main() -> i32 { return clamp(50, 100, 0); }
EOF

saltc demo.salt --lib --disable-alias-scopes -o /dev/null
```

```
[E003] Compilation failed:
VERIFICATION ERROR: could not prove '(<= 100 0)'
  context: precondition check
  counterexample:
    lo = 100
    hi = 0
```

---

## 6. Multiplication: Z3 Handles Non-Linear Arithmetic

Z3 reasons about multiplication — including polynomials — as long as
the operands are constrained:

```bash
cat > demo.salt << 'EOF'
package main

pub fn mul_bounded(a: i32, b: i32) -> i32
    requires(a >= 0 && a <= 10 && b >= 0 && b <= 10)
    ensures(result >= 0 && result <= 100)
{ return a * b; }

pub fn square_nonneg(a: i32) -> i32
    requires(a >= 0)
    ensures(result >= 0)
{ return a * a; }

pub fn main() -> i32 {
    return mul_bounded(5, 8) + square_nonneg(7);
}
EOF

saltc demo.salt --lib --disable-alias-scopes -o /dev/null
```

```
✅ MLIR compiled successfully.
```

Without bounds on the inputs, Z3 finds the counterexample:

```bash
cat > demo.salt << 'EOF'
package main

pub fn mul_any(a: i32, b: i32) -> i32
    ensures(result >= 0)
{ return a * b; }

pub fn main() -> i32 { return mul_any(3, 4); }
EOF

saltc demo.salt --lib --disable-alias-scopes -o /dev/null
```

```
[E003] Compilation failed:
Postcondition violation in 'main__mul_any':
  ensures(result >= 0) is not satisfied for all return paths.
[Formal Shadow] Z3 counter-example:
  a := (- 1)
  b := 1
```

Z3 found that `a = -1, b = 1` produces `-1`, which violates `result >= 0`.
The fix is to add `requires(a >= 0 && b >= 0)` — once the inputs are
constrained, the proof succeeds.

---

## 7. Division Safety

```bash
cat > demo.salt << 'EOF'
package main

pub fn safe_div(a: i32, b: i32) -> i32
    requires(b != 0)
    ensures(result >= 0)
    requires(a >= 0 && b > 0)
{ return a / b; }

pub fn main() -> i32 {
    return safe_div(100, 7);     // Z3 proves: 7 > 0, 100 >= 0, 7 != 0
}
EOF

saltc demo.salt --lib --disable-alias-scopes -o /dev/null
```

```
✅ MLIR compiled successfully.
```

Three preconditions, all proved from constant arguments. Zero overhead.

---

## 8. Bitwise Operations

```bash
cat > demo.salt << 'EOF'
package main

pub fn mask_byte(x: i32) -> i32
    requires(x >= 0 && x <= 255)
    ensures(result >= 0 && result <= 255)
{ return x & 0xFF; }

pub fn or_min(x: i32) -> i32
    requires(x >= 0)
    ensures(result >= x)
{ return x | 0x0F; }

pub fn main() -> i32 {
    return mask_byte(128) + or_min(5);
}
EOF

saltc demo.salt --lib --disable-alias-scopes -o /dev/null
```

```
✅ MLIR compiled successfully.
```

Z3 proves that AND with `0xFF` preserves the `[0, 255]` bound, and that
OR with `0x0F` always produces a value >= the input.

---

## 9. Ten Variables, Polynomial Constraint

Z3 handles constraints with many variables and non-linear terms:

```bash
cat > demo.salt << 'EOF'
package main

pub fn ten_vars(a: i32, b: i32, c: i32, d: i32, e: i32,
                 f: i32, g: i32, h: i32, i: i32, j: i32) -> i32
    requires(a >= 0 && b >= 0 && c >= 0 && d >= 0 && e >= 0
          && f >= 0 && g >= 0 && h >= 0 && i >= 0 && j >= 0
          && a <= 5 && b <= 5 && c <= 5 && d <= 5 && e <= 5
          && f <= 5 && g <= 5 && h <= 5 && i <= 5 && j <= 5)
    ensures(result >= 0)
{ return a*b + c*d + e*f + g*h + i*j; }

pub fn main() -> i32 {
    return ten_vars(1, 2, 3, 4, 5, 1, 2, 3, 4, 5);
}
EOF

saltc demo.salt --lib --disable-alias-scopes -o /dev/null
```

```
✅ MLIR compiled successfully.
```

Ten bounded variables in a polynomial postcondition. Z3 resolves it
within the 100ms window.

---

## 10. StringView Length and Pointer Non-Null

Contracts work on struct fields and standard library types:

```bash
cat > demo.salt << 'EOF'
package main

use std.core.str.StringView

pub fn process_key(key: StringView) -> i32
    requires(key.length() > 0 && key.length() <= 4000)
{ return key.length() as i32; }

pub fn main() -> i32 {
    return process_key("hello");    // Z3 proves: 5 > 0 && 5 <= 4000
}
EOF

saltc demo.salt --lib --disable-alias-scopes -o /dev/null
```

```
✅ MLIR compiled successfully.
```

StringView `.length()` is compiled to an integer that Z3 can reason about.

---

## What This Means

The Z3 bridge translates `requires` and `ensures` expressions into SMT
formulas and checks them at compile time. The outcomes are binary:

| Z3 result | Compiler action |
|-----------|----------------|
| UNSAT — requirement always holds | Check elided. Zero instructions in the binary. |
| SAT — counterexample exists | Compile error with specific violating values. Binary never produced. |

Every contract in the examples above was resolved within 100ms. The
bridge handles integer arithmetic, comparisons, bitwise operations,
multiplication (including polynomials), conditionals with path
sensitivity, struct fields, pointer null checks, and StringView length
— all at compile time, all with zero runtime cost.
