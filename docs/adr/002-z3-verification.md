# ADR 002: Z3 Theorem Prover for Compile-Time Verification

**Status:** Accepted
**Date:** 2025-10 (retroactively documented 2026-06)
**Deciders:** Salt language design

## Context

Traditional systems languages enforce safety through runtime checks (bounds checking, null checks) or type-system constraints (borrow checking). Runtime checks cost CPU cycles. Type-system constraints require developer annotation effort. Salt needed a verification approach that provides mathematical certainty with zero runtime cost for proven properties.

## Decision

**Embed the Z3 SMT solver directly in the compiler for Proof-or-Panic verification.** Every `requires` precondition and `ensures` postcondition is translated to a Z3 formula. The compiler negates the condition and asks Z3 to find a counterexample:

- **UNSAT** (no counterexample) → the condition always holds → check is **elided entirely**, zero runtime cost
- **SAT** (counterexample found) → emit a standard MLIR `scf.if` runtime assertion as fallback
- **UNKNOWN** (Z3 timeout, 100ms default) → emit runtime assertion as fallback

There is no third outcome. Every contract is either mathematically proven or runtime-enforced.

## Consequences

- **Positive**: Proven contracts have literally zero runtime overhead — no branch, no check, no instruction emitted
- **Positive**: Z3 counterexamples are surfaced as compiler errors with concrete violating values, providing actionable diagnostics
- **Positive**: Uses only standard MLIR dialects (`scf.if`, `arith`, `func`) for fallback assertions — no custom verification ops
- **Negative**: Z3 integration adds ~100ms per verification check; complex contracts may exceed the timeout and fall back to runtime checks
- **Negative**: Z3 is a large native dependency (`libz3`), complicating WASM builds (requires a stub) and CI environments
- **Negative**: The SAT/UNSAT inversion (a known Phase 1 issue) can cause false positives in verification results
