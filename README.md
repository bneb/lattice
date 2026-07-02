# Salt

**A systems language where the compiler proves your code correct at compile time.** Z3 contracts, arena memory, MLIR codegen. No garbage collector, no borrow checker, no lifetime annotations.

[![CI](https://github.com/bneb/lattice/actions/workflows/ci.yml/badge.svg)](https://github.com/bneb/lattice/actions/workflows/ci.yml)
[![Version](https://img.shields.io/badge/version-1.0.0-blue)](https://github.com/bneb/lattice/releases/tag/v1.0.0)
[![License](https://img.shields.io/badge/license-MIT-green)](LICENSE)

```salt
package main

use std.core.result.Result

fn safe_div(a: i64, b: i64) -> Result<i64>
    requires(b != 0)
{
    return Result::Ok(a / b);
}

pub fn main() -> i32 {
    // Z3 proves 7 != 0 at compile time — the check evaporates.
    match safe_div(100, 7) {
        Result::Ok(val) => { return val as i32; }
        Result::Err(_) => { return -1; }
    }
}
```

`requires(b != 0)` is proved by Z3 at compile time. Call `safe_div(x, 7)` and the check evaporates — zero instructions emitted. Call `safe_div(x, 0)` and the compiler stops with a counterexample. No binary produced.

---

## Getting started

```bash
git clone https://github.com/bneb/lattice.git
cd lattice
make setup              # Install dependencies (once)
make build              # Build the compiler (~2 min)
make test               # All compiler unit tests pass (1323+)
make lettuce-verify     # Z3 contract verification tests pass
make bench              # LETTUCE vs Redis comparison
```

Prerequisites: Rust 1.75+, Z3 4.12+ (`brew install z3`), LLVM 18+ (`brew install llvm@18`), QEMU (`brew install qemu`).

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
| [LETTUCE](lettuce/) | Redis-compatible server — leads Redis at every concurrency level | ~1500 |
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
- [Blog: Zero-Cost Safety](docs/blog/zero-cost-safety.md) — Z3 contracts at compile time
- [Blog: Microkernel IPC](docs/blog/microkernel-ipc.md) — SPSC rings, zero-copy, proof-carrying IPC
- [Blog: Arenas Over Borrow Checking](docs/blog/arenas-over-borrow-checking.md) — Scope Ladder escape analysis
- [LETTUCE vs Redis](benchmarks/LETTUCE_BENCH.md) — benchmark data and analysis
- [Architecture Reference](docs/ARCH.md) — compiler pipeline, kernel design, memory model
- [Language Specification](docs/SPEC.md) — formal language definition
- [Contributor Guide](CONTRIBUTING.md)

## License

MIT
