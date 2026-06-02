# Rigorous E2E Benchmarks

Automated, multi-iteration, high-fidelity benchmarks across the Lattice macro-applications.

## Microbenchmarks (Algorithms & Data Structures)

| Target | Binary Size (KB) | Peak RSS (KB) | execution_time_s |
|---|---|---|---|
| **bench_ecs_epoch_reclaim (C)** | 32.7 | 6672 | 0.01 |
| **bench_ecs_event_pipeline (C)** | 33.1 | 1024 | 0.05 |
| **bench_ecs_ipc_resolve (C)** | 33.0 | 1056 | 0.05 |
| **bench_ecs_ipc_resolve (Salt)** | 84.2 | 5072 | 0.01 |
| **bench_ecs_lookup (C)** | 32.7 | 1024 | 0.01 |
| **bench_ecs_lookup (Salt)** | 83.7 | 4992 | 0.01 |
| **bench_ecs_scheduler (C)** | 32.7 | 1024 | 0.02 |
| **bench_ecs_scheduler (Salt)** | 84.5 | 5232 | 0.06 |
| **bench_ecs_spawn (C)** | 32.8 | 1024 | 0.19 |
| **bench_ecs_spawn (Salt)** | 83.7 | 8224 | 0.01 |
| **binary_tree_path (C)** | 32.9 | 1264 | 0.01 |
| **binary_tree_path (Rust)** | 432.5 | 1424 | 0.01 |
| **binary_tree_path (Salt)** | 82.2 | 2432 | 0.01 |
| **bitwise (C)** | 16.4 | 1024 | 0.02 |
| **bitwise (Rust)** | 431.4 | 1168 | 0.02 |
| **bitwise (Salt)** | 65.8 | 2496 | 0.04 |
| **buffered_writer_perf (C)** | 32.9 | 1024 | 0.31 |
| **buffered_writer_perf (Rust)** | 431.9 | 1232 | 0.04 |
| **chase_lev_bench (C)** | 32.7 | 1024 | 0.01 |
| **chase_lev_bench (Rust)** | 449.4 | 1232 | 0.01 |
| **coverage_gap (Salt)** | 65.9 | 2336 | 0.01 |
| **coverage_push (Salt)** | 65.8 | 2368 | 0.01 |
| **dll_salt (Salt)** | 82.4 | 2688 | 0.01 |
| **fannkuch (C)** | 32.8 | 1024 | 0.17 |
| **fannkuch (Rust)** | 431.4 | 1168 | 0.14 |
| **fannkuch (Salt)** | 82.1 | 2416 | 0.22 |
| **fib (C)** | 16.5 | 1024 | 0.18 |
| **fib (Rust)** | 431.9 | 1184 | 0.19 |
| **fib (Salt)** | 65.8 | 2336 | 0.18 |
| **forest (C)** | 32.9 | 134448 | 0.19 |
| **forest (Rust)** | 449.8 | 134560 | 0.27 |
| **forest (Salt)** | 82.7 | 100704 | 0.04 |
| **fstring_perf (C)** | 32.8 | 1024 | 1.11 |
| **fstring_perf (Rust)** | 450.0 | 1504 | 0.83 |
| **fstring_perf (Salt)** | 82.8 | 320064 | 0.30 |
| **global_counter (C)** | 16.5 | 1024 | 0.09 |
| **global_counter (Rust)** | 431.5 | 1200 | 0.09 |
| **hashmap_bench (C)** | 33.0 | 1584 | 0.03 |
| **hashmap_bench (Rust)** | 450.4 | 2032 | 0.04 |
| **http_parser_bench (C)** | 32.8 | 1024 | 0.03 |
| **http_parser_bench (Rust)** | 449.3 | 1200 | 0.07 |
| **http_parser_bench (Salt)** | 84.4 | 2352 | 0.01 |
| **longest_consecutive (C)** | 33.1 | 6672 | 0.83 |
| **longest_consecutive (Rust)** | 450.4 | 10016 | 0.32 |
| **lru_cache (C)** | 33.0 | 113200 | 0.03 |
| **lru_cache (Rust)** | 432.0 | 33920 | 0.02 |
| **lru_cache (Salt)** | 82.8 | 2480 | 0.01 |
| **matmul (C)** | 32.8 | 25632 | 1.14 |
| **matmul (Rust)** | 431.9 | 25824 | 1.07 |
| **merge_sorted_lists (C)** | 32.9 | 1184 | 0.05 |
| **merge_sorted_lists (Rust)** | 432.5 | 1456 | 0.06 |
| **merge_sorted_lists (Salt)** | 82.3 | 2400 | 0.02 |
| **promotion_matrix (Salt)** | 65.8 | 2336 | 0.01 |
| **sieve (C)** | 32.8 | 5120 | 0.15 |
| **sieve (Rust)** | 431.4 | 5280 | 0.25 |
| **sieve (Salt)** | 82.2 | 3392 | 0.19 |
| **sliding_window_bench (C)** | 32.7 | 1024 | 0.01 |
| **sliding_window_bench (Rust)** | 449.4 | 1232 | 0.01 |
| **string_hashmap_bench (C)** | 33.3 | 2064 | 0.03 |
| **string_hashmap_bench (Rust)** | 452.3 | 2272 | 0.03 |
| **sudoku_solver (C)** | 32.9 | 1024 | 0.02 |
| **sudoku_solver (Rust)** | 429.0 | 1152 | 0.01 |
| **sudoku_solver (Salt)** | 82.2 | 2336 | 0.01 |
| **syntactic_chaos (Salt)** | 65.8 | 2400 | 0.01 |
| **trapping_rain_water (C)** | 32.8 | 8944 | 0.08 |
| **trapping_rain_water (Rust)** | 432.0 | 6016 | 0.08 |
| **trapping_rain_water (Salt)** | 82.0 | 3520 | 0.07 |
| **trie (C)** | 32.9 | 262496 | 0.08 |
| **trie (Rust)** | 432.6 | 262640 | 0.29 |
| **trie (Salt)** | 82.4 | 250288 | 0.06 |
| **vector_add (C)** | 16.5 | 1024 | 0.11 |
| **vector_add (Rust)** | 431.4 | 1184 | 0.11 |
| **vector_add (Salt)** | 65.8 | 2336 | 0.09 |
| **window_access (C)** | 32.7 | 118240 | 0.07 |
| **window_access (Rust)** | 431.6 | 118400 | 0.09 |
| **window_access (Salt)** | 82.1 | 119536 | 0.07 |
| **writer_perf (C)** | 16.5 | 1024 | 0.11 |
| **writer_perf (Rust)** | 429.4 | 1152 | 0.09 |
| **writer_perf (Salt)** | 82.7 | 2400 | 0.11 |
| **yield_validation (Salt)** | 65.9 | 2464 | 0.01 |


## Application Performance (TCP Echo)

| Implementation | Rate (conn/s) |
|---|---|
| C | 393 |
| Rust | 362 |
| Salt | 392 |


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
Training Time:    7316 ms

[0;32m[Salt] Building...[0m

ML Benchmark Failed:

```
