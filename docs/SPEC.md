# Salt — Language Reference

**July 2026** | Syntax: [SYNTAX.md](../SYNTAX.md) | Architecture: [ARCH.md](ARCH.md)

Salt is a systems language with a Z3 SMT solver baked into the compiler. You write `requires` and `ensures` clauses on functions and the compiler proves them at compile time. No separate verification tool. No annotation language. Just code.

The compiler produces MLIR, then LLVM IR, then native binaries. It's written in Rust. The kernel (KeuOS) is written in Salt. Both are MIT-licensed.

---

## 1. Language

### 1.1 The basics

`let` for immutable bindings. `let mut` when you need to change something. Types on the left, values on the right — same as Rust.

```salt
let x: i32 = 42;
let mut counter = 0;
counter += 1;
```

Functions use `->` for return types. Every function that returns something uses `return`.

```salt
fn add(a: i32, b: i32) -> i32 {
    return a + b;
}
```

Errors are values. `Result<T>` carries either an `Ok(T)` or an `Err(Status)`. The `?` operator extracts the value or returns the error. `|?>` does the same thing but in a pipeline.

```salt
fn process(input: Result<i64>) -> Result<i64> {
    let val = input?;
    let doubled = transform(val)?;
    return Result::Ok(doubled);
}
```

### 1.2 Memory

No garbage collector. No borrow checker either.

Instead: arena allocation with `Arena`. You allocate freely inside a region. When you're done, the whole thing gets freed at once — O(1), deterministic. `mark()` saves your position, `reset_to()` rewinds to it.

For data that outlives a single scope, `HeapAllocator` wraps malloc/free.

Values are moved when passed to functions. The caller loses ownership. Use-after-move is a compile error.

```salt
let arena = Arena::new(4096);
let buf = arena.alloc(256);
let mark = arena.mark();
// ... use buf ...
arena.reset_to(mark);  // everything between mark and here is freed
```

### 1.3 Types

| Type | Example |
|------|---------|
| Signed integers | `i8`, `i16`, `i32`, `i64` |
| Unsigned integers | `u8`, `u16`, `u32`, `u64` |
| Float | `f32`, `f64` |
| Boolean | `bool` |
| Character | `char` (emitted as `i8`) |
| Pointer | `Ptr<T>` (typed, provenance-tracked) |
| Reference | `&T` (immutable), `&mut T` |
| Fixed array | `[T; N]` |
| Tuple | `(T, U)` |
| Function pointer | `fn(T1, T2) -> R` |
| String | `String` (heap-owning), `StringView` (non-owning slice) |

Enums are algebraic:

```salt
enum Shape {
    Circle(f32),
    Rect(f32, f32),
}
```

`match` must be exhaustive. There is no null — use `Option<T>`.

Generics use monomorphization. `Vec<T, A>` takes two type parameters. Function pointers are first-class types: `fn(u64, u64) -> u64`.

Traits (`Clone`, `Eq`, `Hash`, `Ord`) are implemented manually or auto-generated with `@derive`.

### 1.4 Verification

This is why Salt exists. You write contracts on functions:

```salt
fn safe_div(a: i32, b: i32) -> i32
    requires(b != 0)
    ensures(result * b == a)
{
    return a / b;
}
```

The compiler hands `requires` and `ensures` expressions to Z3 during type checking. Z3 has 100ms per obligation.

- **Z3 proves it**: the check evaporates. Zero instructions emitted.
- **Z3 finds a counterexample**: compile error with the violating values.
- **Z3 times out**: the compiler emits a runtime assertion. Your program still compiles.

In practice, most contracts resolve in under 10ms. Integer arithmetic, bounds checks, and linear constraints are essentially free. The things Z3 struggles with (floating-point theory, string constraints, non-linear arithmetic) are documented in [the FAQ](FAQ.md).

---

## 2. Compiler

### 2.1 Pipeline

```
Source (.salt) → Preprocessor → Parser (syn) → HIR → Type Resolution
  → Z3 Verification → MLIR Emission → [mlir-opt → llc → binary]
```

The preprocessor handles Salt-specific syntax that Rust's `syn` parser can't natively parse: `|>`, `|?>`, `@` matmul, f-strings, hex literals, `~` force-unwrap, `use`/`import` conversion, and C++-style generics. The parser is recursive-descent, built on syn.

HIR lowering resolves types, monomorphizes generics, checks ownership. Z3 verification proves contracts (or falls back to runtime checks). MLIR emission produces standard dialect ops — no custom Salt dialect.

### 2.2 MLIR dialects

The compiler emits standard MLIR. No custom ops.

- `arith` — integer and float math
- `scf` — structured control flow (`if`, `for`, `while`)
- `memref` — typed memory buffers
- `func` — function calls
- `llvm` — inline assembly, pointer operations
- `affine` — loop tiling and polyhedral optimization

### 2.3 Optional passes

- **SIR emission** (`--emit-sir`): exports the Intermediate Representation as JSON for tooling
- **Pulse injection**: inserts yield checks at loop back-edges for `@pulse`/`@yielding` functions
- **Binary audit**: post-link disassembly verification for KeuOS targets

---

## 3. Directory structure

```
lattice/
├── SYNTAX.md                     # Syntax reference by example
├── docs/SPEC.md                  # This file
├── docs/ARCH.md                  # Compiler architecture
├── docs/FAQ.md                   # Frequently asked questions
├── salt-front/                   # Rust compiler
│   ├── src/grammar/              # Parser
│   ├── src/codegen/              # MLIR codegen (30+ modules)
│   └── std/                      # Standard library (70+ modules)
├── salt/                         # Legacy C++ dialect (archived)
├── kernel/                       # KeuOS microkernel (in Salt)
├── benchmarks/                   # Performance suite
└── tests/                        # Integration tests
```

---

## 4. Example

Source (`example.salt`):

```salt
package test;

fn safe_div(a: i32, b: i32) -> i32
    requires(b != 0)
{
    return 0;
}

pub fn main() -> i32 {
    return safe_div(10, 0);  // Z3 finds counterexample: b = 0
}
```

This produces a compile error:

```
VERIFICATION ERROR: contract evaluates to false with the given arguments
  context: precondition check (example.salt:8)
```

If you change `safe_div(10, 0)` to `safe_div(10, 7)`, Z3 proves `7 != 0` and the program compiles. The check doesn't exist in the binary.

---

## 5. What works (July 2026)

Ready: `requires`/`ensures`, loop invariants, contract runtime fallback, overflow checks in contracts, prelude injection, ADT lowering with exhaustive match, trait resolution, generic monomorphization, RAII-Lite cleanup, SIMD vector intrinsics, SSA reduction, function pointers, SIR emission, binary audit, pipe operators, f-strings, hex literals, `@matmul`, `@derive`, `@inline`/`@pure`/`@trusted`/`@export`, `@yielding`/`@pulse` scheduling.

Experimental: `concept` keyword, `_` placeholder forwarding in method chains.

Planned: LLVM JIT, Wasm32 target.

---

## 6. Stability

These interfaces are frozen. Changing them requires a major version bump.

**Core syntax**: `fn`, `let`/`let mut`, `return`, `if`/`else`, `while`, `for`, `loop`, `match`, `struct`, `enum`, `impl`, `trait`, `package`, `import`.

**Types**: `i8`–`i64`, `u8`–`u64`, `f32`, `f64`, `bool`, `Ptr<T>`, `&T`/`&mut T`, `[T; N]`, `(T, U)`, `fn(T) -> R`, `String`, `StringView`.

**Contracts**: `requires(expr)`, `ensures(expr)` grammar and Z3 integration.

**Standard library paths**: `std.core.*`, `std.string.*`, `std.io.*`, `std.net.*`, `std.http.*`, `std.json.*`, `std.sync.*`, `std.channel.*`, `std.collections.*`, `std.process.*`, `std.thread.*`.

**CLI**: `--release`, `--binary`, `-c`, `--target`, `--verify`, `--emit-sir`, `-g`/`--debug-info`, `--lib`, `--sip`, `-o`, `--skip-scan`, `--disable-alias-scopes`.

**MLIR output**: Standard dialects only (`func`, `arith`, `scf`, `llvm`, `memref`, `cf`). No custom ops.

**ABI**: C ABI via `extern`, syscall ABI (see `docs/abi/KEUOS_ABI_STABLE.md`).

**Experimental** (may change): `_` placeholder forwarding, `concept` keyword, LLVM JIT, Wasm32, `@pulse_budget`, `async`/`await`, `std.autograd`.

**Deprecated**: `--no-verify` (use `--danger-no-verify`), legacy C++ `salt/` backend.

---

## 7. CLI

```
saltc <file.salt> [-o output] [--release] [--binary] [-c]
      [--target <target>] [--lib] [-g] [--emit-sir] [--skip-scan]
      [--verify] [--danger-no-verify] [--disable-alias-scopes]

  --release               Optimizations on
  --binary                Produce native binary (Mach-O or ELF)
  -c                      Produce .o object file
  --target T              macos, linux-arm64, keuos, keuos-x86_64
  --verify                Run Z3 verification
  --skip-scan             Skip import scanning
  --lib                   Library mode (no main required)
  --sip                   Mode B SIP safety enforcement
  -g / --debug-info       DWARF debug info
  --disable-alias-scopes  Suppress LLVM alias scope metadata
  --emit-sir              Emit SIR as JSON
  --danger-no-verify      Skip ALL verification (debug only)
  -o <path>               Output path
```
