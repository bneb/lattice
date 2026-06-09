# Rigorous E2E Benchmarks

Automated, multi-iteration, high-fidelity benchmarks across the KeuOS macro-applications.

## C10M TCP Echo (Pulse Cannon)

| Target | Binary Size (KB) | Peak RSS (KB) | throughput_rps | latency_avg_us |
|---|---|---|---|---|
| **Salt (MLIR/LLVM, kqueue)** | 89.6 | 1152 | 27,275.67 | 36.69 |
| **C (clang -O3, kqueue)** | 33.4 | 480 | 29,515.00 | 35.36 |
| **Rust (Tokio async)** | 613.5 | 3360 | 26,403.33 | 38.07 |

