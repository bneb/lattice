# ⚡ Salt Performance Benchmarks

Salt's compilation pipeline utilizes MLIR to lower to LLVM IR, giving it the potential to match highly optimized C and Rust. 

To evaluate our performance honestly, we use the following scale comparing Salt's execution time against C (`clang -O3`):
* **Parity**: Within 20% of C (80% – 120%)
* **Salt is faster**: Less than 80% of C's execution time
* **Salt is slower**: Greater than 120% of C's execution time

## 📊 Results (Core Algorithms)

All benchmarks use runtime-dynamic inputs to prevent constant folding. Measurements average 3 runs with cached binaries on macOS ARM64 (Apple Silicon M4).

| Benchmark | C (`clang -O3`) | Salt | % of C | Status | Notes |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `fib` | 175ms | 175ms | 100% | Parity | Pure arithmetic loop. MLIR lowers identically to LLVM. |
| `sieve` | 145ms | 149ms | 102% | Parity | Memory-bound bit/byte manipulation. |
| `http_parser` | 17ms | 16ms | 94% | Parity | Uses verified `intrin_find_byte` mapping directly to LLVM `memchr` for SIMD speed. |
| `matmul` | 150ms | 173ms | 115% | Parity | Affine tiling optimizations keep it competitive. |
| `hashmap_bench` | 21ms | 19ms | 90% | Parity | Salt uses Swiss-tables by default; C baseline uses standard hashing. |
| `vector_add` | 107ms | 83ms | 77% | Salt is faster | LLVM auto-vectorization (NEON) applies more aggressively on Salt's strongly-typed buffers. |
| `lru_cache` | 24ms | 11ms | 45% | Salt is faster | Salt uses zero-overhead Arena allocation; C relies on `malloc`/`free`. |
| `buffered_writer` | 556ms | 43ms | 7% | Salt is faster | C uses standard `stdio` (which locks); Salt's I/O uses unlocked SPSC ring buffers natively. |
| `sudoku_solver` | 28ms | 34ms | 121% | Salt is slower | Array-of-structs boundary checking adds slight overhead in tight recursive loops. |

### The "Faster Than C" Caveat

In scenarios where Salt runs significantly faster than C (e.g., `buffered_writer`, `lru_cache`), it is **not** because the Salt compiler is magically producing faster assembly than clang. 

It is because Salt's standard library provides high-performance data structures—like Arena allocators and lock-free SPSC rings—as ergonomic defaults. The C baselines utilize standard `libc` functions (`malloc`, `free`, `fwrite`) which incur heavy overhead for memory management and thread safety. If a developer wrote equivalent custom memory arenas in C, the performance would drop back to Parity. 

By prioritizing modern memory strategies natively, Salt achieves top-tier performance without forcing the developer to hand-roll allocators.

## 🛠 Running the Suite

```bash
# Run all benchmarks
./benchmark.sh -a

# Run specific benchmark
./benchmark.sh hashmap_bench
```
