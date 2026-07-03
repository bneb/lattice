# Rigorous E2E Benchmarks

Automated, multi-iteration, high-fidelity benchmarks across the KeuOS macro-applications.

## Microbenchmarks (Algorithms & Data Structures)

### Matrix Multiplication (f64, M4 Pro, clang -O3 -ffast-math -march=native)

| Target | 1024² | 2048² | 4096² | Notes |
|---|---|---|---|---|
| **C i,j,k (naive)** | 0.84s | — | — | Inner k-loop: non-sequential, no SIMD |
| **C i,k,j (tuned)** | 0.13s | 1.12s | 8.82s | Hand-tuned loop order, auto-vectorized |
| **Salt `@` (untiled)** | 0.13s | — | — | i,k,j loops, parity with hand-tuned C |
| **Salt `@` (tiled)** | 0.13s | **1.06s** | **8.57s** | ii,kk tile loops + i,k,j compute: beats C at scale |
| **Rust** | 0.13s | — | — | i,k,j loops, ndarray |

### Z3 Verification Coverage (basalt kernels)

`test_kernels.salt` — all functions compile with `requires` clauses + loop invariants, no `unsafe` blocks needed.

| Function | Bounds Checks | Proven | Deferred | Method |
|---|---|---|---|---|
| `rmsnorm` | 6 | 6 (100%) | 0 | for-loop invariant `i < size` |
| `softmax` | 4 | 1 (25%) | 3 | requires `size > 0` for `x[0]` |
| `mat_mul` | 8 | 2 (25%) | 6 | pairwise product bound `m*n` |
| `mat_mul_vec` | 6 | 1 (17%) | 5 | requires `m > 0, d > 0` |
| **Total** | 24 | 10 (42%) | 14 | hybrid: provable→elided, ambiguous→runtime |

The hybrid model is key: Z3 proves the subset it can resolve within 100ms.
The rest become runtime assertions — still safe, just not zero-cost.
The provable set expands as the solver and proof tactics improve.

### Algorithm Verification Coverage (v1.1.0+)

| Algorithm | Checks | Proven | Method |
|---|---|---|---|
| `bubble_sort` (n=4) | 8 | 8 (100%) | forall ensures + for-loop invariant + concrete unrolling |
| `array_fill` (n=4) | 9 | 8 (88%) | for-loop invariant + concrete unrolling |
| `selection_sort` | 5 | 4 (80%) | integer loop invariants |
| `binary_search` | 1 | 0 (0%) | while-loop bounds invariants (symbolic) |

**Provable today:**
- Fixed-bounded loops with unconditional stores (array fill, bubble sort outer loop)
- Integer invariants on for-loops (i >= 0, min_idx >= i)
- Constant-bound forall expansion (ensures forall i in 0..3 => ...)

**Requires conditional store application (gap):**
- Data-dependent inner loops (insertion sort while-loop, selection sort if-guarded swap)
- Case-splitting infrastructure is wired; conditional store modeling is the next increment
