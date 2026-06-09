# The Three Pillars of Salt

> **Salt V1.0**: February 2026

Salt is built on three non-negotiable pillars. Every design decision is weighed against these principles.

---

## 1. Fast Enough

**Goal**: Within 20% of C.

Salt proves that safety and speed are not mutually exclusive. Through MLIR's optimization infrastructure and careful codegen, Salt achieves C-competitive performance without sacrificing ergonomics.

### Evidence: KeuOS Benchmark (February 2026)

| Implementation | Time | Factor |
|----------------|------|--------|
| C (`-O3 -ffast-math`) | 4.3s | 1.0x |
| **Salt V1.0** | 5.1s | **1.2x** |

### Key Optimizations

- **Loop-Carried SSA Values**: Accumulators stay in registers via `scf.for` with `iter_args`
- **Vector Intrinsics**: Portable SIMD through `vector_fma`, `vector_load`, `vector_reduce_add`
- **Polyhedral Optimization**: MLIR's affine dialect enables automatic tiling and vectorization
- **Zero-Cost Abstractions**: No runtime GC, no hidden allocations

---

## 2. Ergonomics

**Goal**: Best features from modern languages.

Salt draws from Rust's safety model, Kotlin's syntax clarity, and functional programming's compositional power, while fixing the rough edges.

### Syntax Highlights

```salt
# Pipeline operator for left-to-right readability
let result = data 
    |> transform()
    |> filter(x -> x > 0)
    |> reduce(0, (a, b) -> a + b);

# Railway operator for error propagation
let file = open("data.txt") |?> parse_json() |?> validate();

# Contracts for formal verification
fn safe_div(a: i32, b: i32) -> i32
    requires(b != 0)
{
    return a / b;
}
```

### Ergonomic Wins

| Feature | Salt | Rust | C |
|---------|------|------|---|
| Null-free types | ✅ `Option<T>` | ✅ | ❌ |
| Pipeline operators | ✅ `\|>` `\|?>` | ❌ | ❌ |
| Pattern matching | ✅ Exhaustive | ✅ | ❌ |
| Lifetime annotations | ✅ Inferred | ❌ Manual | N/A |
| Compile-time contracts | ✅ Z3 | ❌ | ❌ |

### The 140-LOC Neural Network

The KeuOS benchmark implements a complete 2-layer neural network training loop in **140 lines of Salt**, compared to 200+ lines in C. The high-level syntax doesn't sacrifice performance.

---

## 3. Formally Verified

**Goal**: Mathematical certainty at compile time. Zero runtime cost when proven.

Salt integrates the Z3 theorem prover directly into the compiler. Every `requires` contract has exactly one of two outcomes:

1. **Z3 proves it** → The check is **completely elided**. No MLIR emitted. Zero overhead.
2. **Z3 can't prove it** → A standard MLIR runtime assertion is emitted (`scf.if` + `@__salt_contract_violation`). The binary panics if the contract is violated.

There is no third path. Every contract is either mathematically proven or runtime-enforced.

### The Proof-or-Panic Architecture

```salt
fn safe_div(a: i32, b: i32) -> i32
    requires(b != 0)
{
    return a / b;
}

fn main() -> i32 {
    return safe_div(10, 2);   // Z3 proves b=2 ≠ 0 → check elided
}
```

When Z3 proves the contract, the generated MLIR contains **no verification logic at all**; the `requires` clause evaporates. When Z3 cannot prove it, the compiler emits:

```mlir
%violated = arith.xori %cond, %true : i1
scf.if %violated {
    func.call @__salt_contract_violation() : () -> ()
    scf.yield
}
```

This uses only standard MLIR dialects (`arith`, `scf`, `func`); no custom dialect ops.

### Verification Status (V1.0)

| Feature | Enforcement |
|---------|-------------|
| **Preconditions** (`requires`) | 🔒 Z3 Proof-or-Panic |
| **Loop Invariants** | 🔒 Runtime-enforced via `scf.if` |
| **Layout Compatibility** (struct casts) | 🔒 Hard-enforced |
| **Numeric Promotions** | 🔒 Hard-enforced |
| **Bounds Check Elision** | ⚡ Optimized + verified |
| **Postconditions** (`ensures`) | 📋 Parsed, verification planned |

**Legend**: 🔒 Compile-time proven or runtime-enforced | ⚡ Optimized but verified | 📋 In development

### Why SSA Enables Verification

The V1.0 loop-carried value optimization isn't just about speed—it enables verification:

- **Alloca-based accumulators** are hard to verify due to aliasing
- **SSA iter_args** are immutable by definition, enabling Z3 to reason about reductions through simple induction

---

---

*Salt: Fast enough. Ergonomic. Formally verified.*
