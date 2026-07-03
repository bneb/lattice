# Show HN: Salt — a systems language with Z3 theorem proving in the compiler

Salt is a systems programming language that embeds the Z3 SMT solver in the compiler. You add `requires` and `ensures` clauses to functions, and the compiler proves them at compile time. When Z3 succeeds, the check is elided — zero instructions emitted. When it fails, you get a counterexample. When it times out (100ms limit per obligation), the check is skipped and counted.

It compiles through MLIR to LLVM and targets KeuOS, a microkernel with an ECS (Entity Component System) architecture. Both are MIT-licensed.

## How it works

```salt
fn safe_div(a: i32, b: i32) -> i32
    requires(b != 0)
{
    return a / b;
}
```

Call `safe_div(x, 7)` and Z3 proves `7 != 0`. Check elided. Call `safe_div(x, 0)` and the compiler stops. No binary produced.

The key difference from Rust/Zig/C: the compiler calls Z3 during normal compilation. No separate verification tool, no annotation language, no proof assistant. The contract syntax is part of the language.

## What's real

- **Compiler**: 1,752 unit tests passing, clippy clean. Compiles through MLIR to LLVM IR. x86-64 and ARM64 backends.
- **Kernel**: 14/14 QEMU e2e tests pass. TCP stack (connect/send/recv/close), ICMP, deterministic builds. NetD (network daemon) runs as a Ring 3 process on SPSC shared memory rings.
- **ECS architecture**: 13 entity syscalls (402-413). Entity lifecycle (spawn/exit/wait), memory regions as entities (map/brk/alloc), I/O routing via capabilities, socket entity tracking, performance counters, world persistence diagnostics.
- **Shell**: Inline `ecs`, `ps_ecs`, `free_ecs` commands query ECS World without spawning child processes.
- **Benchmarks**: Salt vs C (`clang -O3`) on 21 algorithm benchmarks. Salt at parity or faster on 19/21. Allocation-heavy workloads (hashmap, LRU, buffered writer) see 2-10x wins from arena allocation. Compute-bound (matmul, sieve, fib) at 0.9-1.0x of C. Cache-tiled matmul (`@` operator) beats hand-tuned C by 3-6% at 2K-4K sizes.
- **LSP**: VS Code extension ships with semantic tokens, go-to-def, find-refs, Z3 hover.

## What's not done

Research-quality code. Not production infrastructure.

- The standard library is incomplete. Many things you'd expect are missing.
- Z3 handles integer arithmetic, bit-vectors, and reals. String and quantifier support is partial. Provable checks elide at compile time; the rest become runtime assertions. Proof coverage is reported after compilation: `Z3: 10/24 checks proven (41%), 14 deferred to runtime`. For-loop invariants and Ptr<T> bounds proofs are automatically wired.
- Error messages from the Z3 pass can be opaque.
- The kernel targets QEMU (x86-64). No hardware deployment yet.
- One full-time developer.

## Why this exists

The goal was to find out whether formal verification could be a compiler feature rather than a separate toolchain. The benchmarks say the compiler is fast enough (Lettuce compiles in under a second with contracts enabled). The contracts prevent bugs at implementation time — for example, an `ensures(result != 0)` clause on the SYN cookie function forced handling of a degenerate case that would violate RFC 793. But the language hasn't been used by anyone outside the project, and that's the test that matters.

## Links

- [Source](https://github.com/bneb/lattice)
- [Tutorial](https://github.com/bneb/lattice/blob/main/docs/tutorial/your-first-verified-program.md)
- [Architecture](https://github.com/bneb/lattice/blob/main/docs/ARCH.md)
- [Benchmarks](https://github.com/bneb/lattice/blob/main/benchmarks/BENCHMARKS.md)
