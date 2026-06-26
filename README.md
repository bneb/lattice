# Salt

**A systems language where the compiler proves your code correct at compile time.** Z3 contracts, arena memory, MLIR codegen. No garbage collector, no borrow checker, no lifetime annotations.

[![CI](https://github.com/bneb/lattice/actions/workflows/ci.yml/badge.svg)](https://github.com/bneb/lattice/actions/workflows/ci.yml)
[![Version](https://img.shields.io/badge/version-0.10.0-blue)](https://github.com/bneb/lattice)
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
make setup              # Install dependencies (once)
make build              # Build the compiler (~2 min)
make test               # 11/11 kernel tests pass
make lettuce-verify     # 4/4 Z3 contracts pass
make bench              # LETTUCE vs Redis comparison
```

Prerequisites: Rust 1.75+, Z3 4.12+ (`brew install z3`), LLVM 21+ (`brew install llvm@21`), QEMU (`brew install qemu`).

Your first Salt program:

```salt
// hello.salt
package main

fn main() -> i32 {
    println("Hello, Salt!");
    return 0;
}
```

```bash
salt-front hello.salt --lib --disable-alias-scopes -o /tmp/hello && /tmp/hello
```

[Full tutorial →](docs/tutorial/your-first-verified-program.md) — build a verified key-value store in 15 minutes.

---

## Architecture

```
Salt Source (.salt)
    │
    ▼
Parser → Type Checker → Z3 Verifier → MLIR Emitter    [salt-front/]
    │
    ▼
mlir-opt → mlir-translate → LLVM IR → clang -O3       [LLVM backend]
    │
    ▼
KeuOS Kernel: boot.S → kmain → Drivers → Ring 3        [kernel/]
    │
    ▼
User Programs: LETTUCE, Basalt, FACET, NetD            [user/]
```

[Full architecture reference →](docs/ARCH.md)

## What's here

| | |
|---|---|
| `salt-front/` | Compiler (Rust → MLIR → LLVM → native) |
| `salt-front/std/` | Standard library (arenas, collections, networking, I/O) |
| `kernel/` | KeuOS microkernel — SMP, SPSC IPC, TCP stack, Ring 3 daemons |
| `user/` | User-space programs (echo_server, fetch, NetD, FACET) |
| `docs/` | [Documentation](docs/) — tutorials, blog, specs, deep-dives |
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

- [Tutorial: Your First Verified Program](docs/tutorial/your-first-verified-program.md) — 15-minute walkthrough
- [Blog: Zero-Cost Safety](docs/blog/zero-cost-safety.md) — how Z3 contracts work
- [LETTUCE vs Redis](benchmarks/LETTUCE_BENCH.md) — benchmark data and analysis
- [Architecture Reference](docs/ARCH.md) — compiler pipeline, kernel design, memory model
- [Language Specification](docs/SPEC.md) — formal language definition
- [Tutorial: Salt by Example](docs/tutorial/README.md) — 9-chapter hands-on guide
- [Contributor Guide](CONTRIBUTING.md)

## License

MIT
