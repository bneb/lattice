# Science of Measurement

Salt benchmarks are designed for accuracy and reproducibility. This document describes the methodology used across the benchmark suite.

## Timing Methodology

Salt uses `Instant::now()` from `std.time` for high-resolution monotonic timing:

```salt
use std.time.Instant

let start = Instant::now();

// Critical section
run_benchmark();

let elapsed = start.elapsed();
println(f"Elapsed: {elapsed.as_millis()} ms");
```

On macOS ARM64, this uses `mach_absolute_time()` for nanosecond-precision monotonic measurement. The clock is unaffected by NTP adjustments or system sleep.

## DCE Prevention

All benchmarks use **loop-carried dependencies** to prevent dead code elimination:

```salt
let mut acc: i64 = 0;
for i in 0..N {
    acc += compute(i);
}
println(f"Result: {acc}");  // Forces acc to be live
```

This ensures the compiler cannot optimize away the computation being measured.

## Platform

The 22 tracked Salt-vs-C comparison benchmarks are collected on:
- **Hardware**: Apple Silicon M4
- **OS**: macOS ARM64
- **Compiler flags**: Salt (default), C (`clang -O3`), Rust (`-O` / `--release`)

> [!NOTE]
> KeuOS kernel benchmarks (syscall latency, IPC, scheduler) run separately on **x86_64 KVM** (AWS z1d.metal). See [KEUOS_BENCHMARKS.md](KEUOS_BENCHMARKS.md) for those results.

## Reproducibility

Each benchmark includes matching C and Rust implementations that perform equivalent work. All source code is in the [`benchmarks/`](../../benchmarks/) directory.

To run all benchmarks:
```bash
bash benchmark_all.sh
```

See [BENCHMARKS.md](../BENCHMARKS.md) for full results.
