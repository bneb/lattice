# ADR 009: Textual Preprocessing Before Structured Parsing

**Status:** Accepted
**Date:** 2025-08 (retroactively documented 2026-06)
**Deciders:** Salt compiler design

## Context

Salt's syntax includes constructs that Rust's `syn` parser library cannot handle natively: C++/Java-style generics (`foo<i32>()` vs Rust's `foo::<i32>())`, pipe operators (`|>`), railway operators (`|?>`), f-strings (`f"hello {name}"`), the matmul operator (`@`), and postfix force-unwrap (`~`). Writing a full custom parser for all of these would be a significant engineering investment. Using `syn` (a battle-tested Rust parser) would leverage existing work but requires the input to be valid Rust syntax.

## Decision

**Apply a textual preprocessing layer that transforms Salt-specific syntax into `syn`-compatible Rust syntax before parsing.** The preprocessor (`lib.rs::preprocess()`) performs line-by-line transformations:

- `foo<i32>(42)` → `foo::<i32>(42)` (turbofish conversion)
- `a |> f()` → `__pipe__!(a, f())`
- `a |?> f()` → `__railway__!(a, f())`
- `f"hello {name}"` → `__fstring__!("hello {name}")`
- `a @ b` → `a.matmul(b)`
- `expr~` → `__force_unwrap__!(expr)`
- `// comments` → removed
- `@derive(...)` → expanded to trait impls
- `use` → `import` (until name resolution converts back)

The preprocessor output is valid Rust syntax that `syn` can parse into an AST. The compiler then works with this AST, translating back to Salt semantics during codegen.

## Consequences

- **Positive**: Leverages `syn`'s mature parser — no need to write or maintain a full custom parser
- **Positive**: New syntactic sugar can be added as a preprocessor rule without touching the parser
- **Negative**: Error messages report positions in the transformed source, not the original — column numbers can be off
- **Negative**: The preprocessor is ~1,900 lines of string manipulation with ad-hoc regex patterns; subtle edge cases with nested generics or mixed operators exist
- **Negative**: Debugging requires mentally translating between Salt source and preprocessed Rust output
- **Negative**: The `use` → `import` and back conversion adds complexity to the module system
