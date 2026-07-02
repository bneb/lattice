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
