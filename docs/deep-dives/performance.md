# Deep Dive: Performance & The KeuOS Tax

Salt targets **C-parity or better** across all 22 tracked benchmarks (February 2026). This document explains the architectural decisions that make this possible.

## 1. Zero-Cost Field Access

Salt uses **Canonical Path Flattening** to resolve module-qualified symbols at compile time into a single global address—no pointer chasing, no linker indirection.

```mlir
// Before: implicit dereference chain
%0 = llvm.load %core_ptr : !llvm.ptr -> !struct_core
%1 = llvm.extractvalue %0[12]

// After: direct address (Salt)
%1 = llvm.mlir.addressof @kernel__core__SYSCALL_ENTRY : !llvm.ptr
```

This eliminates unpredictable cache misses during critical syscall handling.

## 2. MLIR Multi-Dialect Optimization

Salt's key performance advantage comes from routing different loop patterns to different MLIR dialects:

| Loop Type | Detection | Dialect | Result |
|-----------|-----------|---------|--------|
| Analytical (tensor) | `A[i,j]` indexing | `affine.for` | Polyhedral tiling |
| Procedural (scalar) | No indexing | `scf.for` | Register throughput |

This explains the matmul speedup—MLIR’s affine passes enable automatic tiling and vectorization that `clang -O3` cannot achieve from C source.

## 3. Arena Mark/Reset (5.6× fstring_perf)

Arena-based allocation eliminates per-object `malloc`/`free` overhead:

| Allocator | Time | Memory |
|-----------|------|--------|
| Salt Arena | 197ms | 307MB |
| C sprintf | 1100ms | 1.1MB |
| Rust format! | 707ms | 1.3MB |

The pattern is simple: `mark()` → allocate → use → `reset_to(mark)`. O(1) bulk reclaim, zero fragmentation, formally verified by the ArenaVerifier.

## 4. Loop-Carried SSA Values

Salt's `scf.for` lowering uses `iter_args` to keep accumulators in registers rather than stack allocations:

- **Alloca-based accumulators** cause store-load forwarding stalls and block LLVM's loop vectorizer
- **SSA iter_args** are immutable by definition, enabling both LLVM optimization and Z3 verification through simple induction

## 5. Zero-Cost Safety Checks

Salt's heartbeat-based yield checks coexist with C-parity performance:

- **Branch weighting**: heartbeat branches are marked `unlikely`, keeping the hot loop contiguous
- **Register persistence**: SSA loop lowering prevents heartbeat calls from spilling loop-carried values

## Benchmark Summary (February 2026)

| Category | Count | Highlights |
|----------|-------|------------|
| 🚀 **Salt Wins** (1.1x+) | 16 | matmul, fstring, writer, forest, sudoku, and 11 more |
| ✅ **C Parity** (±5%) | 6 | fannkuch, fib, bitwise, trapping_rain_water, window_access, global_counter |

See [BENCHMARKS.md](../../benchmarks/BENCHMARKS.md) for full results, specific speedup numbers, and methodology.
