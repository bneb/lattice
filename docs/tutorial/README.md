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
| 1 | [Hello, Salt](01-basics.md) | Package declaration, `fn main`, `println`, `let`, types, comments |
| 2 | [Types & Values](02-types.md) | Integers, floats, bools, chars, arrays, tuples, String/StringView |
| 3 | [Functions](03-functions.md) | Signatures, parameters, return values, function pointers, `@export` |
| 4 | [Structs & Enums](04-structs-enums.md) | Struct definition, `impl` blocks, `&self`, enums, pattern matching |
| 5 | [Generics](05-generics.md) | Type parameters, inference, `@derive`, concepts/trait bounds |
| 6 | [Error Handling](06-error-handling.md) | `Result<T>`, `?` operator, `|?>` railway, `~` force-unwrap, `match` |
| 7 | [Arena Memory](07-arena-memory.md) | Arena allocation, `mark`/`reset_to`, the Scope Ladder, escape analysis |
| 8 | [Async, Yield, and State Machines](08-async.md) | `@yielding`, `yield` keyword, Poll ABI, stackless state machines, `Context` |
| 9 | [Z3 Contracts](09-contracts.md) | `requires`, `ensures`, compile-time proofs, counterexamples, `@trusted` |

## Quick Start

New to Salt? Start here: [Your First Verified Salt Program](your-first-verified-program.md) — a 15-minute walkthrough that builds a verified key-value store with Z3 contracts.

## Going Further

- [Language Specification](../SPEC.md) — Formal language definition
- [Syntax Reference](../../SYNTAX.md) — Every syntax construct with examples
- [Architecture Decision Records](../adr/) — Why Salt works the way it does
- [Standard Library](../../salt-front/std/README.md) — 70+ stdlib modules
