# Salt by Example

A hands-on introduction to the Salt programming language. Each chapter builds on the previous one and includes runnable code examples.

## How to Use This Tutorial

Follow the chapters in order. Every code sample is a complete, compilable Salt program — copy it into a `.salt` file and run it:

```bash
salt-front my_program.salt -o my_program && ./my_program
```

Or use the package manager:

```bash
sp new my_project && cd my_project
# Edit src/main.salt with the example code
sp run
```

## Prerequisites

Run `make setup` (or `./scripts/bootstrap.sh`) from the repository root to install dependencies and build the compiler. You need LLVM 21, Z3 4.12+, and Rust 1.75+.

## Chapters

| # | Chapter | What You'll Learn |
|---|---------|-------------------|
| 1 | [Variables, Types, and Printing](01-basics.md) | Package declaration, `fn main`, `println`, `let`, types, comments |
| 2 | [Functions, Contracts, and Verification](02-functions.md) | Function signatures, parameters, return values, `requires`/`ensures` |
| 3 | [Structs, Enums, and Pattern Matching](03-structs-enums.md) | Struct definition, `impl` blocks, `&self`, enums, pattern matching |
| 4 | [Generics and Monomorphization](04-generics.md) | Type parameters, inference, monomorphization, `where` bounds |
| 5 | [Arenas and Memory](05-arena-memory.md) | Arena allocation, `mark`/`reset_to`, the Scope Ladder, escape analysis |
| 6 | [Error Handling, Result, and Pipe](06-error-handling.md) | `Result<T>`, `?` operator, `|?>` railway, `~` force-unwrap, `match` |
| 7 | [FFI, Extern, and Unsafe](07-ffi.md) | Extern declarations, `unsafe`, `Ptr<T>`, `@no_mangle`, `@export` |
| 8 | [Async, Yield, and State Machines](08-async.md) | `@yielding`, `yield` keyword, Poll ABI, stackless state machines, `Context` |
| 9 | [Z3 Contracts](09-contracts.md) | `requires`, `ensures`, compile-time proofs, counterexamples, `@trusted` |

## Quick Start

New to Salt? Start here: [Your First Verified Salt Program](your-first-verified-program.md) — a 15-minute walkthrough that builds a verified key-value store with Z3 contracts.

## Going Further

- [Language Specification](../SPEC.md) — Formal language definition
- [Syntax Reference](../../SYNTAX.md) — Every syntax construct with examples
- [Architecture Decision Records](../adr/) — Why Salt works the way it does
- [Standard Library](../../salt-front/std/README.md) — 70+ stdlib modules
