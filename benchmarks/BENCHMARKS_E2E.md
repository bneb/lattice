# Rigorous E2E Benchmarks

Automated, multi-iteration, high-fidelity benchmarks across the Lattice macro-applications.

## Microbenchmarks (Algorithms & Data Structures)

| Target | Binary Size (KB) | Peak RSS (KB) | execution_time_s |
|---|---|---|---|
| **binary_tree_path (C)** | 32.9 | 1136 | 0.01 |
| **binary_tree_path (Rust)** | 432.5 | 1408 | 0.01 |
| **binary_tree_path (Salt)** | 84.9 | 2432 | 0.01 |
| **bitwise (C)** | 16.4 | 1008 | 0.02 |
| **bitwise (Rust)** | 431.4 | 1152 | 0.02 |
| **bitwise (Salt)** | 84.6 | 2336 | 0.02 |
| **buffered_writer_perf (C)** | 32.9 | 1008 | 0.56 |
| **buffered_writer_perf (Rust)** | 431.9 | 1216 | 0.06 |
| **chase_lev_bench (C)** | 32.7 | 1008 | 0.01 |
| **chase_lev_bench (Rust)** | 449.4 | 1216 | 0.01 |
| **coverage_gap (Salt)** | 84.7 | 2400 | 0.01 |
| **coverage_push (Salt)** | 84.6 | 2448 | 0.01 |
| **dll_salt (Salt)** | 85.1 | 2688 | 0.01 |
| **fannkuch (C)** | 32.8 | 1008 | 0.17 |
| **fannkuch (Rust)** | 431.4 | 1152 | 0.14 |
| **fannkuch (Salt)** | 84.7 | 2352 | 0.18 |
| **fib (C)** | 16.5 | 1008 | 0.18 |
| **fib (Rust)** | 431.9 | 1168 | 0.19 |
| **fib (Salt)** | 84.6 | 2384 | 0.18 |
| **forest (C)** | 32.9 | 50208 | 0.01 |
| **forest (Rust)** | 449.7 | 50432 | 0.02 |
| **forest (Salt)** | 85.3 | 100640 | 0.03 |
| **fstring_perf (C)** | 32.8 | 1008 | 1.11 |
| **fstring_perf (Rust)** | 450.0 | 1488 | 0.77 |
| **global_counter (C)** | 16.5 | 1008 | 0.09 |
| **global_counter (Rust)** | 431.5 | 1152 | 0.09 |
| **hashmap_bench (C)** | 33.0 | 1648 | 0.03 |
| **hashmap_bench (Rust)** | 449.6 | 1824 | 0.02 |
| **hashmap_bench (Salt)** | 86.9 | 2816 | 0.02 |
| **http_parser_bench (C)** | 32.8 | 1040 | 0.02 |
| **http_parser_bench (Rust)** | 449.3 | 1184 | 0.08 |
| **http_parser_bench (Salt)** | 86.5 | 2384 | 0.04 |
| **longest_consecutive (C)** | 33.1 | 6656 | 0.83 |
| **longest_consecutive (Rust)** | 450.4 | 11200 | 0.32 |
| **lru_cache (C)** | 33.0 | 113136 | 0.02 |
| **lru_cache (Rust)** | 432.0 | 33712 | 0.02 |
| **lru_cache (Salt)** | 85.5 | 2528 | 0.01 |
| **matmul (C)** | 32.8 | 25616 | 0.15 |
| **matmul (Rust)** | 431.9 | 25808 | 0.15 |
| **matmul (Salt)** | 84.6 | 27040 | 0.17 |
| **merge_sorted_lists (C)** | 32.9 | 1040 | 0.02 |
| **merge_sorted_lists (Rust)** | 432.4 | 1280 | 0.02 |
| **merge_sorted_lists (Salt)** | 85.0 | 2384 | 0.02 |
| **promotion_matrix (Salt)** | 84.6 | 2384 | 0.01 |
| **sieve (C)** | 32.8 | 2032 | 0.15 |
| **sieve (Rust)** | 431.4 | 2192 | 0.15 |
| **sieve (Salt)** | 84.9 | 3328 | 0.15 |
| **sliding_window_bench (C)** | 32.7 | 1008 | 0.01 |
| **sliding_window_bench (Rust)** | 449.4 | 1216 | 0.01 |
| **string_hashmap_bench (C)** | 33.3 | 2096 | 0.03 |
| **string_hashmap_bench (Rust)** | 450.7 | 2224 | 0.02 |
| **string_hashmap_bench (Salt)** | 88.2 | 15712 | 0.02 |
| **sudoku_solver (C)** | 32.9 | 1040 | 0.02 |
| **sudoku_solver (Rust)** | 429.0 | 1136 | 0.01 |
| **sudoku_solver (Salt)** | 84.8 | 2384 | 0.01 |
| **syntactic_chaos (Salt)** | 84.6 | 2496 | 0.01 |
| **trapping_rain_water (C)** | 32.8 | 5168 | 0.07 |
| **trapping_rain_water (Rust)** | 432.0 | 9440 | 0.08 |
| **trapping_rain_water (Salt)** | 84.7 | 3568 | 0.07 |
| **trie (C)** | 32.9 | 124992 | 0.03 |
| **trie (Rust)** | 432.7 | 125152 | 0.03 |
| **trie (Salt)** | 85.1 | 250464 | 0.06 |
| **vector_add (C)** | 16.5 | 1008 | 0.11 |
| **vector_add (Rust)** | 431.4 | 1232 | 0.11 |
| **vector_add (Salt)** | 84.6 | 2336 | 0.08 |
| **window_access (C)** | 32.7 | 118224 | 0.07 |
| **window_access (Rust)** | 431.6 | 118384 | 0.08 |
| **window_access (Salt)** | 84.7 | 119632 | 0.07 |
| **writer_perf (C)** | 16.5 | 1008 | 0.10 |
| **writer_perf (Rust)** | 429.4 | 1136 | 0.08 |
| **writer_perf (Salt)** | 85.3 | 2336 | 0.11 |
| **yield_validation (Salt)** | 84.7 | 2336 | 0.01 |


## Application Performance (TCP Echo)

| Implementation | Rate (conn/s) |
|---|---|
| C | 406 |
| Rust | 409 |
| Salt | 409 |


## Application Performance (ML Training)

```
5        0.9379       0.9652       0.9514
6        0.9673       0.9582       0.9628
7        0.9709       0.9426       0.9566
8        0.9702       0.9363       0.9530
9        0.9502       0.9455       0.9478
--------------------------------------------
Macro    0.9604       0.9603       0.9603

============================================================
Summary for Comparison with Salt
============================================================
Training Time:    34457 ms
Test Accuracy:    96.06%
Macro F1 Score:   0.9603
============================================================

[0;34m═══════════════════════════════════════[0m
[1mBenchmark Complete[0m
[0;34m═══════════════════════════════════════[0m

```
