# Z3 Contracts in Salt

Salt embeds the Z3 SMT solver in the compiler. You write `requires` and
`ensures` clauses on functions. The compiler proves them at compile time.
When a proof succeeds, the check is **elided from the binary** — zero
instructions emitted.

Verification is on by default. Disable with `--danger-no-verify`.

---

## 1. Integer Contracts

### Division safety

```bash
cat > div.salt << 'EOF'
package main
pub fn safe_div(a: i32, b: i32) -> i32
    requires(b != 0)
{ return a / b; }
pub fn main() -> i32 { return safe_div(100, 7); }
EOF
saltc div.salt --lib --disable-alias-scopes -o /dev/null
```

```
✅ MLIR compiled successfully.
```

Z3 proved `7 != 0` at the call site. The division check does not exist in
the binary. Now give it zero:

```bash
cat > div.salt << 'EOF'
package main
pub fn safe_div(a: i32, b: i32) -> i32
    requires(b != 0)
{ return a / b; }
pub fn main() -> i32 { return safe_div(100, 0); }
EOF
saltc div.salt --lib --disable-alias-scopes -o /dev/null
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
reported it as a compile error, not a runtime panic.

### Bounds checking

```bash
cat > bounds.salt << 'EOF'
package main
pub fn get(arr: &[i32; 10], idx: i32) -> i32
    requires(idx >= 0 && idx < 10)
{ return arr[idx as i64]; }
pub fn main() -> i32 {
    let data: [i32; 10] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    return get(&data, 5);
}
EOF
saltc bounds.salt --lib --disable-alias-scopes -o /dev/null
```

```
✅ MLIR compiled successfully.
```

Z3 proved `5 >= 0 && 5 < 10`. No bounds check in the binary. With an
out-of-bounds index:

```bash
cat > bounds.salt << 'EOF'
package main
pub fn get(arr: &[i32; 10], idx: i32) -> i32
    requires(idx >= 0 && idx < 10)
{ return arr[idx as i64]; }
pub fn main() -> i32 { let d: [i32; 10] = [0; 10]; return get(&d, 15); }
EOF
saltc bounds.salt --lib --disable-alias-scopes -o /dev/null
```

```
[E003] Compilation failed:
VERIFICATION ERROR: could not prove '(and (>= 15 0) (<= 15 9))'
  counterexample: idx = 15
```

### Multiplication (non-linear arithmetic)

Z3 handles polynomial constraints, not just linear arithmetic:

```bash
cat > mul.salt << 'EOF'
package main
pub fn mul_bounded(a: i32, b: i32) -> i32
    requires(a >= 0 && a <= 10 && b >= 0 && b <= 10)
    ensures(result >= 0 && result <= 100)
{ return a * b; }
pub fn main() -> i32 { return mul_bounded(5, 8); }
EOF
saltc mul.salt --lib --disable-alias-scopes -o /dev/null
```

```
✅ MLIR compiled successfully.
```

Z3 proved the postcondition for all values in the bounded range. Without
bounds, it finds counterexamples:

```bash
cat > mul.salt << 'EOF'
package main
pub fn mul_any(a: i32, b: i32) -> i32
    ensures(result >= 0)
{ return a * b; }
pub fn main() -> i32 { return mul_any(3, 4); }
EOF
saltc mul.salt --lib --disable-alias-scopes -o /dev/null
```

```
[E003] Compilation failed:
Postcondition violation: ensures(result >= 0) is not satisfied.
Z3 counter-example: a := (- 1), b := 1
```

Z3 found that `a = -1, b = 1` produces `-1`, which violates `result >= 0`.
Add `requires(a >= 0 && b >= 0)` and the proof succeeds.

### Ten variables, polynomial constraint

```bash
cat > poly.salt << 'EOF'
package main
pub fn ten(a: i32, b: i32, c: i32, d: i32, e: i32,
           f: i32, g: i32, h: i32, i: i32, j: i32) -> i32
    requires(a >= 0 && a <= 5 && b >= 0 && b <= 5
          && c >= 0 && c <= 5 && d >= 0 && d <= 5
          && e >= 0 && e <= 5 && f >= 0 && f <= 5
          && g >= 0 && g <= 5 && h >= 0 && h <= 5
          && i >= 0 && i <= 5 && j >= 0 && j <= 5)
    ensures(result >= 0)
{ return a*b + c*d + e*f + g*h + i*j; }
pub fn main() -> i32 { return ten(1,2,3,4,5,1,2,3,4,5); }
EOF
saltc poly.salt --lib --disable-alias-scopes -o /dev/null
```

```
✅ MLIR compiled successfully.
```

---

## 2. Postconditions Across Branches

Z3 tracks path conditions through every `if`/`else` branch:

```bash
cat > abs.salt << 'EOF'
package main
pub fn my_abs(x: i32) -> i32
    ensures(result >= 0)
{
    if x < 0 { return -x; }    // Z3 proves: x < 0 → -x >= 0
    return x;                   // Z3 proves: x >= 0 → x >= 0
}
pub fn main() -> i32 { return my_abs(-42); }
EOF
saltc abs.salt --lib --disable-alias-scopes -o /dev/null
```

```
✅ MLIR compiled successfully.
```

Three return paths, three postcondition proofs. Z3 knows that after the
`if x < 0` guard, the else branch executes with `x >= 0`:

```bash
cat > clamp.salt << 'EOF'
package main
pub fn clamp(val: i32, lo: i32, hi: i32) -> i32
    requires(lo <= hi)
    ensures(result >= lo && result <= hi)
{
    if val < lo { return lo; }
    if val > hi { return hi; }
    return val;
}
pub fn main() -> i32 { return clamp(150, 0, 100); }
EOF
saltc clamp.salt --lib --disable-alias-scopes -o /dev/null
```

```
✅ MLIR compiled successfully.
```

---

## 3. Float Literals

Float literals in contracts are truncated to integers for Z3 comparison.
This handles zero-checking and sign checks:

```bash
cat > fdiv.salt << 'EOF'
package main
pub fn safe_fdiv(a: f64, b: f64) -> f64
    requires(b != 0.0)
{ return a / b; }
pub fn main() -> i32 { let _ = safe_fdiv(100.0, 7.0); return 0; }
EOF
saltc fdiv.salt --lib --disable-alias-scopes -o /dev/null
```

```
✅ MLIR compiled successfully.
```

Violations are caught:

```bash
cat > fdiv.salt << 'EOF'
package main
pub fn safe_fdiv(a: f64, b: f64) -> f64
    requires(b != 0.0)
{ return a / b; }
pub fn main() -> i32 { let _ = safe_fdiv(100.0, 0.0); return 0; }
EOF
saltc fdiv.salt --lib --disable-alias-scopes -o /dev/null
```

```
[E003] Compilation failed:
VERIFICATION ERROR: could not prove '(not (= 0 0))'
  counterexample: b = 0
```

---

## 4. String Length

String literal lengths are constant-folded before Z3 sees them:

```bash
cat > str.salt << 'EOF'
package main
use std.core.str.StringView
pub fn check(key: StringView) -> i32
    requires(key.length() > 0)
{ return key.length() as i32; }
pub fn main() -> i32 { return check("hello"); }
EOF
saltc str.salt --lib --disable-alias-scopes -o /dev/null
```

```
✅ MLIR compiled successfully.
```

`"hello".length()` is constant-folded to `5`. Z3 proves `5 > 0`. Empty
strings are rejected:

```bash
cat > str.salt << 'EOF'
package main
use std.core.str.StringView
pub fn check(key: StringView) -> i32
    requires(key.length() > 0)
{ return key.length() as i32; }
pub fn main() -> i32 { return check(""); }
EOF
saltc str.salt --lib --disable-alias-scopes -o /dev/null
```

```
[E003] Compilation failed:
VERIFICATION ERROR: contract evaluates to false with the given arguments
```

---

## 5. Type-Bound Proofs

Z3 receives type bounds for every integer parameter. Contracts that are
implied by the type are proved **without a concrete call-site value**.

The canonical use case: array indexing with `u8`. The bounds check is
free because the type already guarantees the index is in range:

```bash
cat > index.salt << 'EOF'
package main

// A 256-element lookup table. Any u8 indexes it safely.
pub fn lookup(table: &[i32; 256], idx: u8) -> i32
    requires(idx < 256)       // Z3 proves: u8 ∈ [0, 255] ⊂ [0, 255]
{ return table[idx as i64]; }

pub fn main() -> i32 {
    let table: [i32; 256] = [0; 256];
    let idx: u8 = 200;         // not a literal — a runtime variable
    return lookup(&table, idx);
}
EOF
saltc index.salt --lib --disable-alias-scopes -o /dev/null
```

```
✅ MLIR compiled successfully.
```

`idx` is a runtime variable, not a constant. Despite this, Z3 proves
`idx < 256` because it knows `u8` ∈ [0, 255]. The bounds check does not
exist in the binary. A conventional compiler would emit `cmp idx, 256;
jae panic`. Salt emits nothing.

**How it works.** Before checking a contract, the compiler asserts the
parameter's type bounds as Z3 solver constraints. For `fn lookup(idx: u8)`,
Z3 receives `idx >= 0` and `idx <= 255`. The negation of the contract
(`idx >= 256`) is unsatisfiable under these constraints — no
counterexample can exist. The check is elided.

This is not a special case for `u8`. Every integer type gets its bounds:

| Type | Bounds injected | Contract | Proved because |
|------|----------------|----------|---------------|
| `u8` | [0, 255] | `idx < 256` | 255 < 256 |
| `u16` | [0, 65535] | `idx < 65536` | 65535 < 65536 |
| `u32` | ≥ 0 | `x >= 0` | type guarantees it |
| `i8` | [-128, 127] | `x >= -128` | type guarantees it |
| `bool` | {0, 1} | `b == 0 \|\| b == 1` | exhaustive |

User contracts compose with type bounds via AND. `requires(idx < 100)`
on `u8` gives Z3 the effective bound `idx ∈ [0, 99]`. Tighter
constraints from either source only help the proof.

---

## 6. Bitwise Operations

```bash
cat > bitwise.salt << 'EOF'
package main
pub fn mask(x: i32) -> i32
    requires(x >= 0 && x <= 255)
    ensures(result >= 0 && result <= 255)
{ return x & 0xFF; }
pub fn or_min(x: i32) -> i32
    requires(x >= 0)
    ensures(result >= x)
{ return x | 0x0F; }
pub fn main() -> i32 { return mask(128) + or_min(5); }
EOF
saltc bitwise.salt --lib --disable-alias-scopes -o /dev/null
```

```
✅ MLIR compiled successfully.
```

---

## 7. Struct Fields and Pointers

```bash
cat > struct.salt << 'EOF'
package main
struct Arena { max_cores: i64 }
pub fn alloc(arena: Arena, id: i64) -> i64
    requires(id >= 0 && id < arena.max_cores)
{ return id; }
pub fn main() -> i32 {
    let a = Arena { max_cores: 16 };
    return alloc(a, 8) as i32;
}
EOF
saltc struct.salt --lib --disable-alias-scopes -o /dev/null
```

```
✅ MLIR compiled successfully.
```

---

## 8. String Content — Compile-Time Validation

String operations on literal arguments are evaluated in Rust at compile
time. Z3 never runs. Here is a URL validation pipeline — three checks,
all resolved before codegen:

```bash
cat > validate.salt << 'EOF'
package main
use std.core.str.StringView

pub fn validate_url(url: StringView) -> bool
    requires(url.starts_with("https://"))
    requires(url.contains(".com"))
    requires(url.ends_with("/api/v1/"))
{ return true; }

pub fn main() -> i32 {
    let _ok = validate_url("https://salt-lang.com/api/v1/");
    return 0;
}
EOF
saltc validate.salt --lib --disable-alias-scopes -o /dev/null
```

```
✅ MLIR compiled successfully.
```

Three string operations, all resolved in Rust before Z3 sees the
expression. `"https://salt-lang.com/api/v1/".starts_with("https://")`
is `true` — the constant folder evaluates it, returns a boolean literal,
and the `requires` clause becomes `true`. No solver, no runtime check.

**Important limitation:** String content contracts only work with
literal arguments (compile-time constants). With a symbolic (runtime)
string parameter, Z3 will reject the contract even if every caller
satisfies it — the substitution mechanism is `Int`-only, so the
parameter appears as an unconstrained variable. Use `.starts_with()`,
`.ends_with()`, and `.contains()` on literals, not on parameters.

---

## The Frontier

**What Z3 proves or rejects.** Every contract type in sections 1–7 has been
empirically verified. Z3 resolves all tested cases within its 100ms
timeout window.

**Not yet wired (Z3 supports these; the compiler bridge does not translate them):**

| Feature | Z3 support | Bridge status |
|---------|-----------|---------------|
| String `.contains()`, `.startsWith()`, `.endsWith()` | Z3-str | Literal args only (substitution Int-only) |
| Regex (`.matches()`) | Z3-str `Regexp` | Literal args only |
| String parameter substitution | Z3 `substitute` | Int-only — String type pending |
| Float theory (IEEE 754) | Z3 FPA | Truncation-to-int for literals |
| `Real` (exact rationals) | Z3 Real | `Z3Numeric` type designed |
| `BV` (bitvectors) | Z3 BV | Stub type ready |
| Quantifiers (`forall`, `exists`) | Z3 | No Salt syntax |

**Outside Z3's domain:**
- Heap reachability (no cycles, no dangling pointers) — requires separation logic.
- Temporal properties (eventually, always) — requires a model checker.

**What becomes a runtime assertion.** When arguments are symbolic
(variables from an outer caller) and the contract is not implied by type
bounds, Z3 emits a runtime check. This is safe — the program compiles and
panics if the contract is violated at runtime. The check is a standard
`scf.if` branch, compiled through the same LLVM pipeline.

---

## Writing Effective Contracts

1. **Use constants at call sites.** `get(&data, 5)` proves `5 < 10`.
   `get(&data, idx)` becomes a runtime assertion unless `idx` has a type
   bound that implies the contract.

2. **Leverage type bounds.** `requires(idx < 256)` on `u8` is always true.
   No call-site constant needed. Use `u8`, `u16`, `bool`, and `i8`/`i16`
   for parameters where the type implies your contract.

3. **Prefer preconditions to postconditions.** `requires(idx < len)` at
   the call site is more tractable than `ensures(result >= 0)` on a
   function with complex internal logic. Both work; preconditions resolve
   faster.

4. **Keep contracts small.** A single comparison resolves in microseconds.
   Compound conditions are fine but each conjunct is a separate proof
   obligation.

5. **Verify with `saltc` directly.** No special flag needed — verification
   is on by default:

   ```bash
   saltc program.salt --lib --disable-alias-scopes -o /dev/null
   ```
