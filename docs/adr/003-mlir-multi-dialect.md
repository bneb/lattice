# ADR 003: MLIR Multi-Dialect Code Generation

**Status:** Accepted
**Date:** 2025-11 (retroactively documented 2026-06)
**Deciders:** Salt language design

## Context

Traditional compilers emit LLVM IR directly, which represents all loops as unstructured control flow (`br` instructions). This discards loop structure information that could enable domain-specific optimizations (polyhedral tiling, vectorization patterns). Salt needed a compiler backend that preserves loop semantics for optimization while still targeting LLVM's mature code generation.

## Decision

**Emit MLIR using multiple standard dialects chosen by loop-body analysis.** The compiler inspects each loop's body to select the optimal dialect:

| Loop Pattern | Detection | Dialect | Optimization |
|-------------|-----------|---------|-------------|
| Tensor/matrix indexing | Array subscript in body | `affine.for` | Polyhedral tiling, loop fusion |
| Scalar accumulation | No array indexing | `scf.for` with `iter_args` | Register pressure, SSA reduction |
| SIMD operations | `vector_*` intrinsics | `vector` dialect | NEON/AVX mapping |
| Branching control flow | General | `cf` + `llvm` | Standard LLVM backend |

Downstream lowering uses standard `mlir-opt` passes (`--lower-affine`, `--convert-scf-to-cf`, `--convert-vector-to-llvm`, etc.) to reach LLVM dialect, then `mlir-translate` to LLVM IR, then `clang` for native codegen.

## Consequences

- **Positive**: Affine dialect enables polyhedral optimization (loop tiling, fusion) that flat LLVM IR cannot express — this is why Salt outperforms C on matmul
- **Positive**: Uses only standard MLIR dialects; no custom dialect ops survive to downstream tools
- **Positive**: The compiler can mix dialects within a single function (affine loops alongside scalar loops)
- **Negative**: Pipeline complexity — the Iron Driver must orchestrate 4+ external tools (`mlir-opt`, `mlir-translate`, `llc`, `clang`)
- **Negative**: MLIR toolchain version coupling — the compiler is tied to LLVM 21's MLIR APIs
- **Negative**: `cf.cond_br` inside `affine.for` or `scf.for` is illegal in MLIR and causes crashes; the compiler must use `scf.if` exclusively inside structured regions
