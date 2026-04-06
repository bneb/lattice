---
description: Run Salt language benchmarks comparing Salt, C, and Rust performance
---

# Salt Benchmark Workflow

## Quick Commands

// turbo
1. Run all benchmarks:
```bash
cd /Users/kevin/projects/keuos/benchmarks && ./benchmark.sh -a
```

// turbo
2. Run specific benchmark(s):
```bash
cd /Users/kevin/projects/keuos/benchmarks && ./benchmark.sh <name1> [name2] ...
```

Examples:
- `./benchmark.sh lru_cache` - Single benchmark
- `./benchmark.sh writer_perf fstring_perf` - Multiple benchmarks
- `./benchmark.sh fannkuch fib matmul` - Compare multiple

## Available Benchmarks

| Name | Category | Key Metric |
|------|----------|------------|
| `matmul` | Compute | Matrix multiplication |
| `fstring_perf` | String | F-string formatting |
| `writer_perf` | String | Direct buffer writes |
| `forest` | Memory | Tree allocation |
| `lru_cache` | Memory | Linked list LRU |
| `trie` | Memory | Trie operations |
| `sudoku_solver` | Algorithm | Backtracking |
| `fannkuch` | Algorithm | Permutation benchmark |
| `fib` | Algorithm | Recursive fib |
| `sieve` | Algorithm | Prime sieve |
| `vector_add` | Compute | Vector operations |
| `binary_tree_path` | Algorithm | Tree traversal |
| `merge_sorted_lists` | Algorithm | List merging |
| `trapping_rain_water` | Algorithm | Dynamic programming |
| `global_counter` | Memory | Global state |
| `bitwise` | Compute | Bit operations |
| `window_access` | Memory | Sliding window |

## Rebuild Salt Compiler

// turbo
3. If changing compiler code, rebuild first:
```bash
cd /Users/kevin/projects/keuos/salt-front && Z3_SYS_Z3_HEADER=/opt/homebrew/include/z3.h LIBRARY_PATH=/opt/homebrew/lib cargo build --release
```

## View MLIR Output

4. Check generated MLIR for a Salt file:
```bash
cd /Users/kevin/projects/keuos/salt-front && DYLD_LIBRARY_PATH=/opt/homebrew/lib ./target/release/salt-front "../benchmarks/<name>.salt" -o /tmp/<name>.mlir
```

## Benchmark Script Internals

The script performs:
1. **Salt**: salt-front → MLIR → mlir-opt → mlir-translate → opt -O3 → clang
2. **C**: `clang -O3 -march=native`
3. **Rust**: `rustc -C opt-level=3`

Each benchmark runs 3 times with 0.1s thermal pause between runs.

## Update Benchmark Documentation

Results are stored in:
- `/Users/kevin/projects/keuos/benchmarks/BENCHMARKS.md`
