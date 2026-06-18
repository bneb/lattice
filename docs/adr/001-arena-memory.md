# ADR 001: Arena-Based Memory Management

**Status:** Accepted
**Date:** 2025-09 (retroactively documented 2026-06)
**Deciders:** Salt language design

## Context

Systems languages face a trilemma: manual memory management (C: fast but unsafe), garbage collection (Go/Java: safe but unpredictable latency), and borrow checking (Rust: safe and fast but complex lifetime annotations). Salt needed a memory model that delivers C-level performance with compile-time safety guarantees but without requiring developers to annotate lifetimes throughout their code.

## Decision

**Use arena-based allocation with compile-time escape analysis.** Memory is allocated from fixed-size regions (arenas). The `ArenaVerifier` performs a depth-based escape analysis called the **Scope Ladder** at compile time, proving that no reference outlives its arena. Arenas support O(1) bulk deallocation via `arena.reset_to(mark)`.

Three laws govern the Scope Ladder:
1. **Return Rule**: `return x` is valid only if `depth(x) ≤ 1`
2. **Assignment Rule**: `a = b` is valid only if `depth(b) ≤ depth(a)`
3. **Transitivity Rule**: `s.field` inherits `depth(s)`

## Consequences

- **Positive**: No lifetime annotations needed in source code; the verifier infers depths automatically
- **Positive**: O(1) bulk deallocation eliminates per-object free overhead and fragmentation
- **Positive**: Z3 integration means escape violations are caught at compile time, not runtime
- **Negative**: Arena-based allocation requires developers to think in terms of allocation phases; long-lived objects need their own arena or placement at depth 0-1
- **Negative**: Not suitable for patterns requiring fine-grained, independent object lifetimes (e.g., arbitrary graph structures) — those require a separate allocator
