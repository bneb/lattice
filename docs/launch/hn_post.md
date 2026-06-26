# Show HN: Salt — a systems language with Z3 theorem proving in the compiler

Salt is a systems programming language that embeds the Z3 SMT solver directly in the compiler. You add `requires` and `ensures` clauses to functions. The compiler proves them at compile time. When Z3 succeeds, the check is elided from the binary — zero instructions emitted. When it fails, you get a counterexample at compile time. When it times out (100ms limit per obligation), you get a runtime assertion as fallback.

It compiles through MLIR to LLVM and targets the KeuOS microkernel, a unikernel we built alongside the language. Both are open source under MIT.

## What's different from Rust / Zig / C

Rust proves memory safety through the borrow checker. Zig gives you comptime and no hidden allocations. C trusts you.

Salt's approach: the compiler calls Z3 on your contracts during normal compilation. There's no separate verification tool, no annotation language, no proof assistant you have to learn. If Z3 can prove `requires(b != 0)` at a call site, the division-by-zero check simply doesn't exist in the binary.

```salt
fn safe_div(a: i32, b: i32) -> i32
    requires(b != 0)
{
    return a / b;
}
```

Call `safe_div(x, 7)` and Z3 proves `7 != 0`. Check elided. Call `safe_div(x, 0)` and the compiler stops with a counterexample. This isn't a lint or a warning — the binary is never produced.

## Concrete results

- **Kernel tests**: 11/11 pass, deterministic builds (byte-identical across clean rebuilds)
- **LETTUCE** (our Redis-compatible server, 314 lines of Salt): leads Redis 7 on every command tested — SET (4.2x), GET (6.8x), PING (2.8x) — across all concurrency levels (1–100 clients). The gap comes from arena allocation (zero `malloc` per request) vs Redis's `zmalloc`/`zfree` under concurrent load.
- **Algorithm benchmarks**: 12 benchmarks against C (`clang -O3`). Salt is within 20% of C on most, faster on allocation-heavy workloads where the arena model avoids `malloc`.
- **Compile times**: Lettuce compiles in 0.73s with all Z3 contracts enabled. Sub-second feedback loop.

## What's not done

This is research-quality code, not production infrastructure. Things you'll hit:

- The standard library is incomplete. Many things you'd expect from Rust's std are missing.
- Z3 can only prove linear integer arithmetic and bit-vector constraints. Complex data structure invariants, floating-point, and string reasoning are outside its reach. Those contracts become runtime assertions.
- The compiler has one backend (x86-64 via LLVM). ARM64 macOS works for native binaries; the kernel targets QEMU.
- Error messages from the Z3 pass are improving but can be opaque.
- There are ~1,200 tests. Coverage is good but not exhaustive.

## Why we built this

We wanted to know whether formal verification could be a compiler feature rather than a separate tool. The hypothesis: if verification is fast enough and the syntax is familiar enough, programmers will use it. The counter-hypothesis is that SMT-based verification is too fragile, too slow, or too limited to be useful outside academic projects.

We don't know which is right yet. The benchmarks say it's fast enough. The contracts on the kernel say it catches real bugs. But the language hasn't been used by anyone outside our team, and that's the test that matters.

## Links

- [Source](https://github.com/bneb/lattice)
- [Tutorial: Your First Verified Salt Program](https://github.com/bneb/lattice/blob/main/docs/tutorial/your-first-verified-program.md)
- [Blog: Zero-Cost Safety](https://github.com/bneb/lattice/blob/main/docs/blog/zero-cost-safety.md)
- [LETTUCE vs Redis benchmarks](https://github.com/bneb/lattice/blob/main/benchmarks/LETTUCE_BENCH.md)
- [Architecture reference](https://github.com/bneb/lattice/blob/main/docs/ARCH.md)

Happy to answer questions about the Z3 integration, the kernel architecture, or why we chose arenas over borrow checking.
