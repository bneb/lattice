# FAQ — Salt / KeuOS

Anticipated questions for the HN launch. Answers should be direct, honest, and backed by evidence where available.

---

## Why not just use Rust?

Rust's borrow checker proves memory safety. Salt's Z3 integration proves functional correctness — "this index is always in bounds," "this divisor is never zero," "this ring descriptor size is always valid."

They're different proof systems for different properties. You can't express `requires(idx >= 0 && idx < arr.len())` in Rust's type system. You can in Salt, and the compiler proves it at compile time.

That said, Rust has a mature ecosystem, production users, and excellent tooling. Salt has none of those. If you're building a production service today, use Rust. We built Salt to explore whether compiler-integrated formal methods are practical — not to replace Rust.

## Is this production-ready?

No. It's research-quality code. The test suite passes, the benchmarks are real, and the contracts catch real bugs. But the standard library is incomplete, the error messages need work, and nobody outside our team has used it in anger.

We're sharing it now because the architecture is solid and the ideas are worth discussing. It is not suitable for production workloads.

## How does the Z3 integration actually work?

The compiler extracts `requires` and `ensures` expressions from the AST during type checking. Each contract becomes a Z3 formula: the condition is negated and Z3 checks satisfiability.

- **UNSAT**: no counterexample exists. The condition always holds. The compiler emits nothing — the check evaporates.
- **SAT**: Z3 found a violating input. The compiler reports the specific values and stops.
- **TIMEOUT**: Z3 couldn't decide within 100ms. The compiler emits a standard `scf.if` branch with a call to `__salt_contract_violation`. No custom MLIR dialect ops.

The timeout is per obligation, not per compilation. A function with three `requires` clauses gets up to 300ms total (3 × 100ms). In practice, most contracts resolve in under 10ms.

## What can't Z3 prove?

Z3 handles linear integer arithmetic, bit-vectors, and quantifier-free formulas efficiently. It cannot prove:

- Floating-point properties (Z3's float theory is incomplete)
- String length or content constraints
- Properties of unbounded data structures (linked lists, trees with arbitrary depth)
- Non-linear integer arithmetic (multiplication of two variables)

When Z3 can't prove something, the contract becomes a runtime check. This is safe — your program still compiles and runs — but the check has runtime cost.

## What's the unsafe story?

Salt has `unsafe` blocks for FFI, raw pointer manipulation, and inline assembly. These are restricted to the standard library by convention — application code should never need them.

Every `unsafe` function is expected to carry a `requires` contract documenting its safety preconditions. The compiler verifies these contracts at call sites. An `unsafe` function without a contract is a code review finding.

The `@trusted` attribute marks functions that bypass Z3 verification entirely — used for hand-audited FFI wrappers and assembly stubs. Every `@trusted` function must have a comment explaining why.

## Why not use Rust proc macros or a Rust DSL?

Rust macros operate on token streams. Z3 operates on SMT formulas. Translating Rust's MIR to SMT is an active research area (see Prusti, Creusot), but it requires a separate tool and annotation language.

Salt's advantage is integration: the compiler, type checker, and verifier share the same AST, the same monomorphization, the same constant folding. This means Z3 sees fully-monomorphized, constant-folded expressions. `requires(arr.len() > 0)` at a call site where `arr` is `[i32; 5]` becomes `5 > 0` before Z3 sees it. That's why most contracts resolve in under 10ms.

A Rust proc macro can't do this because it runs before monomorphization and constant propagation.

## How does performance compare to C and Rust?

[Full benchmark data →](../../benchmarks/BENCHMARKS.md)

On pure compute (fib, matmul, sieve): within 20% of C, sometimes faster when LLVM auto-vectorization kicks in on Salt's strongly-typed buffers.

On allocation-heavy workloads (LRU cache, hash maps, string formatting): Salt is often faster because the arena allocator avoids `malloc`. The C baselines use standard libc — a hand-written arena in C would close the gap.

LETTUCE leads Redis 7 by 1.1–6.8× across all concurrency levels on the commands it implements. The gap is structural — zero `malloc` per request vs Redis's `zmalloc`/`zfree` contention under concurrent load.

## Who is this for?

Right now: researchers working on formal methods, language designers, and systems programmers curious about compiler-integrated verification.

Eventually: anyone writing code where correctness matters more than ecosystem maturity — embedded systems, kernel modules, cryptographic implementations, network protocol handlers.

## What's the relationship between Salt and KeuOS?

Salt is the language. KeuOS is the microkernel we built to test it. The kernel's memory manager, IPC subsystem, and network stack use Z3 contracts. KeuOS proves that the language works for real systems code — not just benchmarks.

Both are in the same repo. You can use Salt without KeuOS (the compiler targets macOS and Linux native). You cannot use KeuOS without Salt (the kernel is written in it).

## How many people work on this?

One full-time developer, with occasional contributions. The commit history is public.

## What's the license?

MIT. Both the compiler and the kernel.
