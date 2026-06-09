# Rigorous E2E Benchmarks

Automated, multi-iteration, high-fidelity benchmarks across the KeuOS macro-applications.

## Microbenchmarks (Algorithms & Data Structures)

| Target | Binary Size (KB) | Peak RSS (KB) | execution_time_s |
|---|---|---|---|
| **binary_tree_path (C)** | 32.9 | 1296 | 0.01 |
| **binary_tree_path (Rust)** | 432.5 | 1520 | 0.01 |
| **binary_tree_path (Salt)** | 85.2 | 2688 | 0.01 |
| **bitwise (C)** | 16.4 | 1168 | 0.02 |
| **bitwise (Rust)** | 431.4 | 1328 | 0.02 |
| **bitwise (Salt)** | 84.9 | 2576 | 0.02 |
| **buffered_writer_perf (C)** | 32.9 | 1184 | 0.32 |
| **buffered_writer_perf (Rust)** | 431.9 | 1360 | 0.04 |
| **buffered_writer_perf (Salt)** | 86.0 | 2720 | 0.02 |
| **chase_lev_bench (C)** | 32.7 | 1168 | 0.01 |
| **chase_lev_bench (Rust)** | 449.4 | 1392 | 0.01 |
| **chase_lev_bench (Salt)** | 64.6 | 2576 | 0.01 |
| **coverage_gap (Salt)** | 85.0 | 2576 | 0.01 |
| **coverage_push (Salt)** | 84.9 | 2576 | 0.01 |
| **dll_salt (Salt)** | 85.4 | 3120 | 0.02 |
| **fannkuch (C)** | 32.8 | 1184 | 0.14 |
| **fannkuch (Rust)** | 431.4 | 1328 | 0.14 |
| **fannkuch (Salt)** | 85.1 | 2592 | 0.14 |
| **fib (C)** | 16.5 | 1168 | 0.18 |
| **fib (Rust)** | 431.9 | 1344 | 0.19 |
| **fib (Salt)** | 85.0 | 2624 | 0.18 |
| **forest (C)** | 32.9 | 99520 | 0.02 |
| **forest (Rust)** | 449.7 | 99744 | 0.02 |
| **forest (Salt)** | 85.4 | 100912 | 0.02 |
| **fstring_perf (C)** | 32.8 | 1168 | 1.14 |
| **fstring_perf (Rust)** | 450.0 | 1648 | 0.83 |
| **fstring_perf (Salt)** | 85.6 | 315680 | 0.20 |
| **global_counter (C)** | 16.5 | 1168 | 0.09 |
| **global_counter (Rust)** | 431.5 | 1328 | 0.09 |
| **global_counter (Salt)** | 85.0 | 2608 | 0.09 |
| **hashmap_bench (C)** | 33.0 | 1488 | 0.02 |
| **hashmap_bench (Rust)** | 449.6 | 2192 | 0.02 |
| **hashmap_bench (Salt)** | 87.2 | 2864 | 0.02 |
| **http_parser_bench (C)** | 32.8 | 1184 | 0.03 |
| **http_parser_bench (Rust)** | 449.2 | 1360 | 0.05 |
| **http_parser_bench (Salt)** | 86.6 | 2592 | 0.03 |
| **longest_consecutive (C)** | 33.1 | 7984 | 0.83 |
| **longest_consecutive (Rust)** | 450.4 | 11344 | 0.34 |
| **longest_consecutive (Salt)** | 87.1 | 19264 | 0.26 |
| **lru_cache (C)** | 33.0 | 112656 | 0.02 |
| **lru_cache (Rust)** | 432.0 | 33072 | 0.02 |
| **lru_cache (Salt)** | 85.8 | 2736 | 0.01 |
| **matmul (C)** | 32.8 | 25792 | 0.16 |
| **matmul (Rust)** | 431.9 | 25952 | 0.18 |
| **matmul (Salt)** | 85.0 | 27216 | 0.16 |
| **merge_sorted_lists (C)** | 32.9 | 1200 | 0.02 |
| **merge_sorted_lists (Rust)** | 432.4 | 1440 | 0.02 |
| **merge_sorted_lists (Salt)** | 85.3 | 2592 | 0.02 |
| **promotion_matrix (Salt)** | 84.9 | 2576 | 0.01 |
| **sieve (C)** | 32.8 | 2208 | 0.15 |
| **sieve (Rust)** | 431.4 | 2368 | 0.15 |
| **sieve (Salt)** | 85.2 | 4112 | 0.15 |
| **sliding_window_bench (C)** | 32.7 | 1168 | 0.01 |
| **sliding_window_bench (Rust)** | 449.4 | 1392 | 0.01 |
| **sliding_window_bench (Salt)** | 64.6 | 2576 | 0.01 |
| **string_hashmap_bench (C)** | 33.3 | 2160 | 0.03 |
| **string_hashmap_bench (Rust)** | 450.7 | 1888 | 0.02 |
| **string_hashmap_bench (Salt)** | 88.5 | 15552 | 0.02 |
| **sudoku_solver (C)** | 32.9 | 1184 | 0.02 |
| **sudoku_solver (Rust)** | 429.0 | 1312 | 0.02 |
| **sudoku_solver (Salt)** | 85.0 | 2976 | 0.03 |
| **syntactic_chaos (Salt)** | 84.9 | 2576 | 0.01 |
| **trapping_rain_water (C)** | 32.8 | 5760 | 0.07 |
| **trapping_rain_water (Rust)** | 432.0 | 4432 | 0.08 |
| **trapping_rain_water (Salt)** | 85.0 | 3984 | 0.08 |
| **trie (C)** | 32.9 | 125152 | 0.03 |
| **trie (Rust)** | 432.7 | 125328 | 0.03 |
| **trie (Salt)** | 85.3 | 126608 | 0.03 |
| **vector_add (C)** | 16.5 | 1168 | 0.11 |
| **vector_add (Rust)** | 431.4 | 1344 | 0.11 |
| **vector_add (Salt)** | 84.9 | 2672 | 0.09 |
| **window_access (C)** | 32.7 | 118384 | 0.09 |
| **window_access (Rust)** | 431.6 | 118560 | 0.09 |
| **window_access (Salt)** | 85.1 | 119776 | 0.08 |
| **writer_perf (C)** | 32.8 | 1184 | 0.13 |
| **writer_perf (Rust)** | 429.4 | 1344 | 0.07 |
| **writer_perf (Salt)** | 85.6 | 2992 | 0.12 |
| **yield_validation (Salt)** | 85.0 | 2576 | 0.01 |


## Application Performance (TCP Echo)

| Implementation | Rate (conn/s) |
|---|---|
| C | N/A |
| Rust | N/A |
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
Training Time:    7296 ms

[0;32m[Salt] Building...[0m

ML Benchmark Failed:

```
