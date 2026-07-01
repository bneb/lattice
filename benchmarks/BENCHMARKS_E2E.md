# Rigorous E2E Benchmarks

Automated, multi-iteration, high-fidelity benchmarks across the KeuOS macro-applications.

## Microbenchmarks (Algorithms & Data Structures)

| Target | Binary Size (KB) | Peak RSS (KB) | execution_time_s |
|---|---|---|---|
| **binary_tree_path (C)** | 32.9 | 1296 | 0.01 |
| **binary_tree_path (Rust)** | 432.5 | 1552 | 0.02 |
| **binary_tree_path (Salt)** | 85.6 | 2848 | 0.02 |
| **bitwise (C)** | 16.4 | 1168 | 0.06 |
| **bitwise (Rust)** | 431.4 | 1312 | 0.06 |
| **bitwise (Salt)** | 85.3 | 2576 | 0.06 |
| **buffered_writer_perf (C)** | 32.9 | 1168 | 2.30 |
| **buffered_writer_perf (Rust)** | 431.9 | 1376 | 0.20 |
| **chase_lev_bench (C)** | 32.7 | 1264 | 0.02 |
| **chase_lev_bench (Rust)** | 449.4 | 1376 | 0.02 |
| **chase_lev_bench (Salt)** | 65.1 | 2576 | 0.02 |
| **coverage_gap (Salt)** | 69.2 | 2576 | 0.02 |
| **coverage_push (Salt)** | 69.2 | 2752 | 0.02 |
| **dll_salt (Salt)** | 85.7 | 3152 | 0.03 |
| **fannkuch (C)** | 32.8 | 1168 | 0.33 |
| **fannkuch (Rust)** | 431.4 | 1424 | 0.33 |
| **fannkuch (Salt)** | 85.4 | 2640 | 0.32 |
| **fib (C)** | 16.5 | 1168 | 0.45 |
| **fib (Rust)** | 431.9 | 1440 | 0.49 |
| **fib (Salt)** | 69.2 | 2624 | 0.46 |
| **forest (C)** | 32.9 | 99520 | 0.04 |
| **forest (Rust)** | 449.7 | 99728 | 0.04 |
| **forest (Salt)** | 85.8 | 101136 | 0.05 |
| **fstring_perf (C)** | 32.8 | 1168 | 2.85 |
| **fstring_perf (Rust)** | 450.0 | 1504 | 1.79 |
| **fstring_perf (Salt)** | 86.1 | 315648 | 0.64 |
| **global_counter (C)** | 16.5 | 1184 | 0.21 |
| **global_counter (Rust)** | 431.5 | 1312 | 0.21 |
| **global_counter (Salt)** | 85.4 | 2592 | 0.21 |
| **hashmap_bench (C)** | 33.0 | 2320 | 0.07 |
| **hashmap_bench (Rust)** | 449.6 | 2320 | 0.07 |
| **http_parser_bench (C)** | 32.8 | 1392 | 0.06 |
| **http_parser_bench (Rust)** | 449.2 | 1344 | 0.12 |
| **http_parser_bench (Salt)** | 87.0 | 2592 | 0.07 |
| **longest_consecutive (C)** | 33.1 | 13968 | 2.00 |
| **longest_consecutive (Rust)** | 450.4 | 19264 | 0.85 |
| **lru_cache (C)** | 33.0 | 112704 | 0.04 |
| **lru_cache (Rust)** | 432.0 | 33360 | 0.04 |
| **lru_cache (Salt)** | 86.2 | 2720 | 0.03 |
| **matmul (C)** | 32.8 | 25776 | 0.43 |
| **matmul (Rust)** | 431.9 | 25968 | 0.44 |
| **matmul (Salt)** | 85.4 | 27424 | 0.45 |
| **merge_sorted_lists (C)** | 32.9 | 1200 | 0.04 |
| **merge_sorted_lists (Rust)** | 432.4 | 1424 | 0.04 |
| **merge_sorted_lists (Salt)** | 85.7 | 2624 | 0.04 |
| **promotion_matrix (Salt)** | 69.2 | 2576 | 0.02 |
| **sieve (C)** | 32.8 | 2192 | 0.37 |
| **sieve (Rust)** | 431.4 | 2352 | 0.37 |
| **sieve (Salt)** | 85.6 | 3584 | 0.38 |
| **sliding_window_bench (C)** | 32.7 | 1168 | 0.01 |
| **sliding_window_bench (Rust)** | 449.4 | 1376 | 0.02 |
| **sliding_window_bench (Salt)** | 65.1 | 2576 | 0.02 |
| **string_hashmap_bench (C)** | 33.3 | 3584 | 0.09 |
| **string_hashmap_bench (Rust)** | 450.7 | 2432 | 0.05 |
| **sudoku_solver (C)** | 32.9 | 1168 | 0.06 |
| **sudoku_solver (Rust)** | 429.0 | 1376 | 0.05 |
| **sudoku_solver (Salt)** | 85.4 | 2704 | 0.06 |
| **syntactic_chaos (Salt)** | 69.2 | 2800 | 0.02 |
| **trapping_rain_water (C)** | 32.8 | 9088 | 0.18 |
| **trapping_rain_water (Rust)** | 432.0 | 9600 | 0.19 |
| **trapping_rain_water (Salt)** | 85.4 | 3984 | 0.20 |
| **trie (C)** | 32.9 | 125152 | 0.08 |
| **trie (Rust)** | 432.7 | 125312 | 0.06 |
| **vector_add (C)** | 16.5 | 1168 | 0.27 |
| **vector_add (Rust)** | 431.4 | 1328 | 0.27 |
| **vector_add (Salt)** | 69.2 | 2816 | 0.21 |
| **window_access (C)** | 32.7 | 118384 | 0.23 |
| **window_access (Rust)** | 431.6 | 118544 | 0.21 |
| **window_access (Salt)** | 85.4 | 120000 | 0.22 |
| **writer_perf (C)** | 32.8 | 1168 | 0.34 |
| **writer_perf (Rust)** | 429.4 | 1328 | 0.18 |
| **writer_perf (Salt)** | 86.1 | 2832 | 0.32 |
| **yield_validation (Salt)** | 69.3 | 2576 | 0.02 |


## Application Performance (TCP Echo)

| Implementation | Rate (conn/s) |
|---|---|
| C | 150 |
| Rust | 151 |
| Salt | N/A |


## Application Performance (ML Training)

```
  4    0.9704     0.9695     0.9699
  5    0.9415     0.9742     0.9576
  6    0.9757     0.9645     0.9701
  7    0.9772     0.9572     0.9671
  8    0.9768     0.9507     0.9636
  9    0.9602     0.9564     0.9583
------------------------------------
Macro  0.9687     0.9688     0.9687

=== Summary ===
Test Accuracy:    96.90%
Macro Precision:  0.9687
Macro Recall:     0.9688
Macro F1:         0.9687
Training Time:    17505 ms

[0;32m[Salt] Building...[0m

ML Benchmark Failed:

```
