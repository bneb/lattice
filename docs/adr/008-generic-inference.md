# ADR 008: Six-Phase Generic Inference Pipeline

**Status:** Accepted
**Date:** 2026-02 (retroactively documented 2026-06)
**Deciders:** Salt compiler design

## Context

Salt supports generic functions and structs with C++/Java-style syntax (`identity<i32>(42)`). The compiler must infer generic type arguments at call sites where they are not explicitly provided. This is a constraint-solving problem: given a function signature `fn foo<A, B>(a: A, b: B) -> C` and a call `foo(x, y)`, determine the concrete types for A, B, and C. Bidirectional type inference (where return type context constrains argument types) adds further complexity.

## Decision

**A six-phase canonical inference pipeline in `generic_resolver.rs`:**

1. **Turbofish phase**: Extract explicitly provided type arguments (`foo::<i32>`)
2. **Struct-level phase**: Infer type arguments from struct field types
3. **Argument inference phase**: Unify parameter types with argument expressions
4. **Self-type inference phase**: Resolve `Self` in impl contexts
5. **Return-type inference phase**: Use expected return type to constrain arguments (bidirectional)
6. **Phantom inference phase**: Fallback heuristics for under-constrained type variables

Each phase narrows the set of possible type assignments. After all phases, a completeness check verifies that every type variable has been resolved.

## Consequences

- **Positive**: Supports both explicit (`::<T>`) and inferred generic arguments
- **Positive**: Bidirectional inference (Phase 5) enables ergonomic patterns like `let x: Vec<i64> = Vec::new()`
- **Negative**: Six-phase design is complex and phases have implicit ordering dependencies
- **Negative**: Phantom phase (Phase 6) uses fallback heuristics that may produce surprising results for deeply nested generics
- **Negative**: Error messages for inference failures report "could not infer type" without tracing which phase failed — harder to debug than Rust's trait-based error reporting
