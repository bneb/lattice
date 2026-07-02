# Grammar

**The Mission:** Define the Salt parser and attribute system.

## Components

| File | Role |
|------|------|
| [`attr.rs`](./attr.rs) | **Attribute Parser.** Handles `@pulse(N)`, `@derive(Clone, Eq)`, `@yielding`, `@inline`, `@trusted`. |
| [`pattern.rs`](./pattern.rs) | **Pattern Parser.** Handles destructuring in `match` arms, `let` bindings, and `for` loops. Supports tuple, struct, enum, and wildcard (`_`) patterns. |

## Design Notes

- The parser is a hand-written recursive-descent parser (not generated from a grammar file)
- It lives in [../](../) as the main parser entry point, with grammar-specific helpers in this directory
- Attributes use `@name` syntax (not Rust's `#[name]`) — see [SYNTAX.md](../../../../SYNTAX.md) for the full list
