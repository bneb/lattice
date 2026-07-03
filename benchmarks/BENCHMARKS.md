# Salt Performance Benchmarks

Salt compiles through MLIR to LLVM IR, producing native code competitive with C (`clang -O3`).

Measurements use runtime-dynamic inputs to prevent constant folding. Averages from 3 runs with warmup on macOS ARM64 (Apple Silicon M4). June 2026.

## Results

Within 15% of C is "Parity." Beyond that, Salt is faster or slower.

| Benchmark | C (`clang -O3`) | Salt | Salt/C | Status |
| :--- | :--- | :--- | :--- | :--- |
| `fib` | 289ms | 263ms | 0.9x | Parity |
| `sieve` | 219ms | 217ms | 1.0x | Parity |
| `matmul` | 249ms | 241ms | 1.0x | Parity — with cache tiling: beats C by 3-6% at 2K-4K (see E2E) |
| `fannkuch` | 215ms | 205ms | 0.9x | Parity |
| `sudoku_solver` | 37ms | 35ms | 0.9x | Parity |
| `hashmap_bench` | 58ms | 36ms | 0.6x | Salt faster |
| `lru_cache` | 33ms | 22ms | 0.7x | Salt faster |
| `vector_add` | 154ms | 120ms | 0.8x | Salt faster |
| `window_access` | 117ms | 113ms | 1.0x | Parity |
| `forest` | 27ms | 41ms | 1.5x | Salt slower |
| `binary_tree_path` | 9ms | 11ms | 1.2x | Salt slower |
| `global_counter` | 121ms | 124ms | 1.0x | Parity |
| `string_hashmap_bench` | 51ms | 29ms | 0.6x | Salt faster |
| `bitwise` | 38ms | 38ms | 1.0x | Parity |
| `trapping_rain_water` | 109ms | 118ms | 1.1x | Parity |
| `merge_sorted_lists` | 27ms | 27ms | 1.0x | Parity |
| `longest_consecutive` | 1,156ms | 362ms | 0.3x | Salt faster |
| `buffered_writer_perf` | 789ms | 46ms | 0.1x | Salt faster |
| `fstring_perf` | 1,858ms | 401ms | 0.2x | Salt faster |
| `writer_perf` | 212ms | 177ms | 0.8x | Salt faster |

**Summary: Salt is at parity or faster on 18 of 20 benchmarks.** The two where Salt trails (`forest` 1.5x, `binary_tree_path` 1.2x) involve pointer-chasing patterns where C's optimizer has more mature alias analysis.

### On "Faster Than C"

Where Salt significantly outperforms C, it's because the standard library provides arena allocators, Swiss-table hash maps, lock-free SPSC rings, and interpolated string handlers as defaults. The C baselines use standard `libc` (`malloc`, `free`, `fwrite`, `snprintf`). A C developer who hand-rolled equivalent data structures would close the gap. The point is: with Salt, you don't have to.

## Running

```bash
./benchmarks/benchmark.sh --all
```
