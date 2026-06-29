# Rigorous E2E Benchmarks

Automated, multi-iteration, high-fidelity benchmarks across the KeuOS macro-applications.

## Microbenchmarks (Algorithms & Data Structures)

| Target | Binary Size (KB) | Peak RSS (KB) | execution_time_s |
|---|---|---|---|
| **binary_tree_path (C)** | 32.9 | 1296 | 0.01 |
| **binary_tree_path (Rust)** | 432.5 | 1504 | 0.02 |
| **binary_tree_path (Salt)** | 85.9 | 2688 | 0.01 |
| **bitwise (C)** | 16.4 | 1168 | 0.04 |
| **bitwise (Rust)** | 431.4 | 1312 | 0.04 |
| **bitwise (Salt)** | 85.6 | 2576 | 0.04 |
| **buffered_writer_perf (C)** | 32.9 | 1168 | 0.79 |
| **buffered_writer_perf (Rust)** | 431.9 | 1376 | 0.08 |
| **buffered_writer_perf (Salt)** | 86.8 | 2800 | 0.05 |
| **chase_lev_bench (C)** | 32.7 | 1168 | 0.01 |
| **chase_lev_bench (Rust)** | 449.4 | 1376 | 0.01 |
| **chase_lev_bench (Salt)** | 64.7 | 2800 | 0.01 |
| **coverage_gap (Salt)** | 85.7 | 2576 | 0.01 |
| **coverage_push (Salt)** | 85.6 | 2576 | 0.02 |
| **dll_salt (Salt)** | 86.1 | 2896 | 0.02 |
| **fannkuch (C)** | 32.8 | 1168 | 0.22 |
| **fannkuch (Rust)** | 431.4 | 1312 | 0.20 |
| **fannkuch (Salt)** | 85.8 | 2608 | 0.20 |
| **fib (C)** | 16.5 | 1168 | 0.29 |
| **fib (Rust)** | 431.9 | 1328 | 0.30 |
| **fib (Salt)** | 85.7 | 2576 | 0.26 |
| **forest (C)** | 32.9 | 99520 | 0.03 |
| **forest (Rust)** | 449.7 | 99728 | 0.04 |
| **forest (Salt)** | 86.1 | 100928 | 0.04 |
| **fstring_perf (C)** | 32.8 | 1216 | 1.86 |
| **fstring_perf (Rust)** | 450.0 | 1504 | 1.19 |
| **fstring_perf (Salt)** | 86.5 | 315856 | 0.40 |
| **global_counter (C)** | 16.5 | 1168 | 0.12 |
| **global_counter (Rust)** | 431.5 | 1312 | 0.12 |
| **global_counter (Salt)** | 85.8 | 2608 | 0.12 |
| **hashmap_bench (C)** | 33.0 | 1680 | 0.06 |
| **hashmap_bench (Rust)** | 449.6 | 1904 | 0.05 |
| **hashmap_bench (Salt)** | 87.9 | 3520 | 0.04 |
| **http_parser_bench (C)** | 32.8 | 1168 | 0.06 |
| **http_parser_bench (Rust)** | 449.2 | 1600 | 0.09 |
| **http_parser_bench (Salt)** | 87.3 | 2624 | 0.05 |
| **longest_consecutive (C)** | 33.1 | 13968 | 1.16 |
| **longest_consecutive (Rust)** | 450.4 | 17360 | 0.47 |
| **longest_consecutive (Salt)** | 87.9 | 25648 | 0.36 |
| **lru_cache (C)** | 33.0 | 112816 | 0.03 |
| **lru_cache (Rust)** | 432.0 | 33296 | 0.03 |
| **lru_cache (Salt)** | 86.5 | 2736 | 0.02 |
| **matmul (C)** | 32.8 | 25888 | 0.25 |
| **matmul (Rust)** | 431.9 | 25968 | 0.24 |
| **matmul (Salt)** | 85.7 | 27216 | 0.24 |
| **merge_sorted_lists (C)** | 32.9 | 1200 | 0.03 |
| **merge_sorted_lists (Rust)** | 432.4 | 1424 | 0.03 |
| **merge_sorted_lists (Salt)** | 86.0 | 2640 | 0.03 |
| **promotion_matrix (Salt)** | 85.6 | 2784 | 0.01 |
| **sieve (C)** | 32.8 | 2192 | 0.22 |
| **sieve (Rust)** | 431.4 | 2448 | 0.22 |
| **sieve (Salt)** | 85.9 | 3584 | 0.22 |
| **sliding_window_bench (C)** | 32.7 | 1232 | 0.01 |
| **sliding_window_bench (Rust)** | 449.4 | 1376 | 0.01 |
| **sliding_window_bench (Salt)** | 64.7 | 2816 | 0.01 |
| **string_hashmap_bench (C)** | 33.3 | 2496 | 0.05 |
| **string_hashmap_bench (Rust)** | 450.7 | 2144 | 0.03 |
| **string_hashmap_bench (Salt)** | 89.5 | 3712 | 0.03 |
| **sudoku_solver (C)** | 32.9 | 1168 | 0.04 |
| **sudoku_solver (Rust)** | 429.0 | 1296 | 0.03 |
| **sudoku_solver (Salt)** | 85.7 | 2576 | 0.03 |
| **syntactic_chaos (Salt)** | 85.6 | 2576 | 0.01 |
| **trapping_rain_water (C)** | 32.8 | 9056 | 0.11 |
| **trapping_rain_water (Rust)** | 432.0 | 10464 | 0.13 |
| **trapping_rain_water (Salt)** | 85.7 | 3760 | 0.12 |
| **trie (C)** | 32.9 | 125152 | 0.05 |
| **trie (Rust)** | 432.7 | 125312 | 0.05 |
| **vector_add (C)** | 16.5 | 1168 | 0.15 |
| **vector_add (Rust)** | 431.4 | 1328 | 0.15 |
| **vector_add (Salt)** | 85.6 | 2576 | 0.12 |
| **window_access (C)** | 32.7 | 118384 | 0.12 |
| **window_access (Rust)** | 431.6 | 118544 | 0.12 |
| **window_access (Salt)** | 85.8 | 119776 | 0.11 |
| **writer_perf (C)** | 32.8 | 1168 | 0.21 |
| **writer_perf (Rust)** | 429.4 | 1328 | 0.11 |
| **writer_perf (Salt)** | 86.4 | 3024 | 0.18 |
| **yield_validation (Salt)** | 85.8 | 2624 | 0.01 |


## Application Performance (TCP Echo)

| Implementation | Rate (conn/s) |
|---|---|
| C | 170 |
| Rust | 180 |
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
Training Time:    11762 ms

[0;32m[Salt] Building...[0m

ML Benchmark Failed:

```
