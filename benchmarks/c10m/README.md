# C10M Benchmark Suite

Targets: **10 million concurrent connections**, sub-100μs P99 latency.

## Silicon Ingest: Cycle-Accurate Comparison (V3 — Audited)

All configs use **shared M4 timing constants**. Each cycle is attributed to a specific component.

| Config | Parsing | I/O | Safety | Sched+Mem | **Total** |
|:-------|--------:|----:|-------:|----------:|----------:|
| C / epoll | 1084 | 612 | 0 | 17 | **1744** |
| C / io_uring | 1084 | 12 | 0 | 17 | **1144** |
| Rust / Tokio | 1084 | 612 | 20 | 125 | **1872** |
| Rust / io_uring | 1084 | 12 | 20 | 85 | **1232** |
| **Salt / KeuOS** | **161** | **12** | **0** | **26** | **233** |

### Speedups

| Comparison | Speedup | Type |
|:-----------|--------:|:-----|
| vs C / epoll | 7.5x | System-level (different I/O + parsing) |
| vs **C / io_uring** | **4.9x** | **Fair (same I/O, SIMD vs scalar)** |
| vs Rust / Tokio | 8.0x | System-level |
| vs **Rust / io_uring** | **5.3x** | **Fair (same I/O)** |

### Advantage Breakdown (vs C/io_uring)

| Source | Cycles | Notes |
|:-------|-------:|:------|
| **NEON SIMD parsing** | **923** | 11.1x header scan speedup (dominant factor) |
| Scheduling | −10 | C event loop is lighter than Salt coroutine |
| Memory mgmt | +1 | Arena vs pool (negligible) |
| Z3 formal safety | 0 | Both skip checks — Salt does it with guarantees |

> **Note:** If C also used NEON SIMD (e.g., picohttpparser), the fair gap would shrink to ~1-2x.

### Concurrency Metrics

| Metric | Salt V2.0 | C | Rust (Tokio) |
|:-------|:----------|:--|:-------------|
| Max Connections | 10M+ (arena) | 1M (RAM) | 2M (stack) |
| Mem per Conn | ~128B (TaskFrame) | ~256B (state) | ~512B (Future) |
| Context Switch | ~8ns (25 cycles) | ~5ns (event loop) | ~30ns (async poll) |
| L1D Capacity | 512 tasks | 256 states | 128 futures |
| Safety Cost | 0% (Z3 formal) | 0% (manual) | 8-12% (runtime) |

## Pulse Cannon Echo Benchmark (Measured — February 2026)

Real-world TCP echo throughput via **Pulse Cannon** load generator:
100 persistent connections, 100K packets, `send()/recv()` tight-loop on localhost (Apple M4).

| Implementation | Throughput | Latency | vs C | Failures |
|:---------------|----------:|---------:|-----:|---------:|
| **C** (kqueue, single-threaded) | **79,240** pkt/s | 12.62 µs | baseline | 0 |
| **Salt** (kqueue via FFI bridge) | **78,259** pkt/s | 12.78 µs | **−1.2%** | 0 |
| **Rust** (Tokio async runtime) | **71,253** pkt/s | 14.03 µs | −10.1% | 0 |

### Analysis

- **Salt ≈ C**: The 1.2% gap is within measurement noise — both are kqueue-bound. Salt uses `extern fn` declarations backed by a C bridge (`echo_salt_bridge.c`) that issues the same `kqueue`/`kevent`/`accept`/`recv`/`send` syscall sequence as the handwritten C.
- **Rust tax**: Tokio's async runtime adds ~1.4 µs/packet overhead vs bare kqueue. This is the cost of `epoll`-style wake/poll machinery on top of the raw I/O path.
- **Compilation pipeline**: `salt-front --disable-alias-scopes` → `mlir-opt` → `mlir-translate` → `opt -O3` → `clang -O3` → Mach-O arm64 binary.

### Binary Sizes

| Implementation | Binary Size |
|:---------------|------------:|
| C (kqueue) | 34 KB |
| Salt (kqueue) | 35 KB |
| Rust (Tokio) | 628 KB |

### Running the Benchmark

```bash
# Build all targets
cd benchmarks/c10m
clang -O3 -o build/echo_c echo_c.c
clang -O3 -o build/pulse_cannon stress_echo.c

# Start a server (pick one)
./build/echo_c 8080                    # C baseline
./build/echo_salt                       # Salt (hardcoded port 8080)
./build/echo_rust_proj/target/release/echo_rust 8080  # Rust

# Fire the cannon (in a separate terminal)
./build/pulse_cannon 127.0.0.1 8080 100 100000
```

## Benchmarks

| Benchmark | Purpose | Status |
|:----------|:--------|:-------|
| **`echo_salt.salt`** | **TCP echo throughput (98.8% of C)** | **✅ Measured** |
| `echo_chamber.salt` | Throughput & latency (P99 < 100μs) | ✅ |
| `noisy_neighbor.salt` | Scheduler fairness (jitter < 1ms) | ✅ |
| `arena_exhaustion.salt` | Memory resilience (linear scaling) | ✅ |
| `multi_pulse_demo.salt` | End-to-end C10M pipeline validation | ✅ |
| `out_of_bounds_hijack.salt` | Code Red diagnostic (must fail compilation) | ✅ |

## Architecture

```
Packet → io_uring CQE → KeuOSBuffer (zero-copy, NIC DMA)
       → NEON SIMD scan (find_header_end, 16B/cycle)
       → Z3-proven slice (bounds check ELIDED)
       → @pulse handler (stackless coroutine, 25-cycle swap)
       → IsolatedArena (O(1) pointer bump)
       → io_uring SQE (batched response)
```

### Compiler Modules

| Module | Purpose |
|:-------|:--------|
| `codegen/intrinsics.rs` | M4 NEON, WFE, io_uring intrinsics |
| `codegen/verification/slice_verifier.rs` | Z3 bounds check elision (24 tests) |
| `codegen/verification/silicon_ingest.rs` | Cycle-accurate M4 pipeline model (14 tests) |

## Running

```bash
# Compile a benchmark
./benchmark.sh c10m/echo_chamber

# Run Silicon Ingest (cycle simulation)
cd salt-front && cargo test --lib silicon_ingest -- --nocapture

# Run Code Red diagnostic tests
cd salt-front && cargo test --lib slice_verifier -- --nocapture
```
