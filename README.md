# Salt

**A systems language where the compiler proves your code correct at compile time.** Z3 contracts, arena memory, MLIR codegen. No garbage collector, no borrow checker, no lifetime annotations.

[![CI](https://github.com/bneb/lattice/actions/workflows/ci.yml/badge.svg)](https://github.com/bneb/lattice/actions/workflows/ci.yml)
[![Version](https://img.shields.io/badge/version-0.8.0-blue)](https://github.com/bneb/lattice)
[![License](https://img.shields.io/badge/license-MIT-green)](LICENSE)

```salt
fn binary_search(arr: &[i64], target: i64) -> i64
    requires(arr.len() > 0)
{
    let mut lo: i64 = 0;
    let mut hi: i64 = arr.len() - 1;
    while lo <= hi {
        let mid = (lo + hi) / 2;
        if arr[mid] == target { return mid; }
        if arr[mid] < target { lo = mid + 1; }
        else { hi = mid - 1; }
    }
    return -1;
}
```

`requires(arr.len() > 0)` is checked by Z3 at compile time. Passing an empty array is a compile error with a counterexample. Passing a non-empty array elides the check — zero instructions emitted.

---

## Getting started

```bash
git clone https://github.com/bneb/lattice.git
cd lattice
make build              # Build the compiler (~2 min)
make lettuce-verify     # 4/4 Z3 contracts pass
make bench              # LETTUCE vs Redis comparison
```

Prerequisites: Rust 1.75+, Z3 4.12+ (`brew install z3`), LLVM 21+.

---

## What's here

| | |
|---|---|
| `salt-front/` | Compiler (Rust → MLIR → LLVM → native) |
| `salt-front/std/` | Standard library (arenas, collections, networking, I/O) |
| `docs/tutorial/` | [Salt by Example](docs/tutorial/) — 9 chapters |
| `docs/SPEC.md` | [Language specification](docs/SPEC.md) |
| `site/` | [salt-lang.dev](https://salt-lang.dev) — website, blog, playground |
| `tools/salt-lsp/` | LSP server (VS Code extension) |

## Built with Salt

| Project | What it is | Lines |
|---------|-----------|-------|
| [KeuOS](kernel/) | Microkernel — 16-core SMP, SPSC IPC, Ring 3 daemons | — |
| [LETTUCE](lettuce/) | Redis-compatible server — leads Redis at every concurrency level | 567 |
| [Basalt](basalt/) | Llama 2 inference engine | ~600 |
| [FACET](user/facet/) | GPU-accelerated 2D compositor with Metal shaders | — |

---

## Verification

Salt embeds Z3 as a compiler pass. Functions carry `requires` and `ensures` clauses. The compiler translates them to Z3 formulas and checks them during normal compilation.

**UNSAT** — the condition always holds. The check is elided. Zero instructions.

**SAT** — Z3 found a counterexample. You get the specific values before your program runs.

**TIMEOUT** — Z3 cannot decide within 100ms. A runtime assertion is emitted. Your program still compiles and runs.

Verification is progressive: add the contracts you can prove today. The rest become runtime checks you can address later.

## Performance

LETTUCE, a 314-line Salt server implementing 9 Redis commands, was benchmarked against Redis 7 on the same machine using `redis-benchmark`. 13-point concurrency sweep, 50,000 requests per test.

| Clients | LETTUCE | Redis 7 |
|---------|---------|---------|
| 1 | 5,219 req/s | 1,437 req/s |
| 10 | 21,758 | 5,178 |
| 50 | 14,144 | 12,710 |
| 100 | 22,381 | 17,876 |

LETTUCE leads at every concurrency level. The structural advantage is the arena allocator — zero `malloc` per request vs. Redis's `zmalloc`/`zfree` contention under load. [Full benchmark data →](benchmarks/LETTUCE_BENCH.md)

## Further reading

- [Tutorial: build a verified server](lettuce/TUTORIAL.md)
- [Blog: Compile-time verification with negative overhead](https://salt-lang.dev/blog/zero-cost-safety.html)
- [Blog: Microkernel IPC without the performance tax](https://salt-lang.dev/blog/microkernel-ipc.html)
- [Blog: Why we chose arenas over borrow checking](https://salt-lang.dev/blog/arenas-over-borrow-checking.html)
- [Language specification](docs/SPEC.md)
- [Package manager design](docs/package-manager/DESIGN.md)
- [Contributor guide](CONTRIBUTING.md)

## License

MIT
