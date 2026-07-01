# FAQ — Salt / KeuOS

Anticipated questions for the HN launch. Answers should be direct, honest, and backed by evidence where available.

---

## Why not just use Rust?

Rust's borrow checker proves memory safety. Salt's Z3 integration proves functional correctness — "this index is always in bounds," "this divisor is never zero," "this ring descriptor size is always valid."

They're different proof systems for different properties. You can't express `requires(idx >= 0 && idx < arr.len())` in Rust's type system. You can in Salt, and the compiler proves it at compile time.

That said, Rust has a mature ecosystem, production users, and excellent tooling. Salt has none of those. If you're building a production service today, use Rust. Salt was built to explore whether compiler-integrated formal methods are practical — not to replace Rust.

## Is this production-ready?

No. It's research-quality code. The test suite passes, the benchmarks are real, and the contracts catch real bugs. But the standard library is incomplete, the error messages need work, and nobody outside the project has used it in anger.

It's being shared now because the architecture is solid and the ideas are worth discussing. It is not suitable for production workloads.

## How does the Z3 integration actually work?

The compiler extracts `requires` and `ensures` expressions from the AST during type checking. Each contract becomes a Z3 formula: the condition is negated and Z3 checks satisfiability.

- **UNSAT**: no counterexample exists. The condition always holds. The compiler emits nothing — the check evaporates.
- **SAT**: Z3 found a violating input. The compiler reports the specific values and stops.
- **TIMEOUT**: Z3 couldn't decide within 100ms. The compiler counts the timeout and continues. No runtime check is emitted (this is a known gap).

The timeout is per obligation, not per compilation. A function with three `requires` clauses gets up to 300ms total (3 × 100ms). In practice, most contracts resolve in under 10ms. The runtime-assertion fallback for timeouts is planned but not yet implemented.

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

Salt is the language. KeuOS is the microkernel built alongside it for testing. The kernel's memory manager, IPC subsystem, and network stack use Z3 contracts. KeuOS proves that the language works for real systems code — not just benchmarks.

Both are in the same repo. You can use Salt without KeuOS (the compiler targets macOS and Linux native). You cannot use KeuOS without Salt (the kernel is written in it).

## How does this compare to SPARK?

[SPARK](https://www.adacore.com/languages/spark) is the closest production equivalent. It's an Ada subset with formal verification — preconditions, postconditions, and invariants proved at compile time. It's used in avionics, defense, and rail systems. It's been in development for decades. It's sound: if SPARK says a property holds, it holds.

Salt takes the same idea but embeds the verifier directly in the compiler pipeline. SPARK uses a separate tool (GNATprove) that runs alongside your build. Salt calls Z3 during normal compilation.

The practical differences:

- **Proof guarantees.** SPARK is deductive and sound — it won't miss a violation within its domain. Salt uses SMT with a 100ms timeout per obligation. When Z3 times out, the check becomes a runtime assertion. Salt skips verification silently in those cases, which is a real trade-off.
- **What gets proved.** Both prove absence of runtime errors (bounds checks, division by zero, overflow). Salt additionally lets you express arbitrary functional properties in contracts — `ensures(result * b == a)` — which Z3 can sometimes prove. SPARK can do this too, but the proof burden is higher.
- **Maturity.** SPARK is industrial. Salt is one person's research project. SPARK has an IDE, debugger, package manager, and a user base that builds safety-critical systems. Salt has a VS Code extension and a tutorial.
- **Memory model.** SPARK uses ownership tracking derived from Rust's borrow checker. Salt uses arena allocation with escape analysis — no lifetimes, no borrow checking. The trade-off is that Salt's model only works for allocation patterns that fit the arena shape (request-response, frame-at-a-time).
- **Language.** SPARK is based on Ada. Salt looks like Rust with a few syntactic differences. If you already know Rust, Salt is easier to pick up. If you work in aerospace or defense, you probably know Ada already.

If you're building a safety-critical system today, use SPARK. It works now. Salt is an experiment in whether verification can be simpler, not in whether verification is possible.

## How many people work on this?

One full-time developer, with occasional contributions. The commit history is public.

## What's the license?

MIT. Both the compiler and the kernel.
