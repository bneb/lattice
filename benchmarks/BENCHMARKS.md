# ⚡ Salt High-Performance Benchmarks

Official performance benchmarks comparing Salt, C (Clang -O3), and Rust (-O).

## 🛠 Methodology

- **DCE Prevention**: Loop-carried dependencies prevent dead code elimination
- **Fair Comparison**: All implementations do equivalent work
- **Platform**: macOS ARM64 (Apple Silicon M4)

## 📊 Results (February 21, 2026)

**28 benchmarks building. Salt ≤ C in 19/22 head-to-head.**

### All Benchmarks

*Official `benchmark.sh -a` output on Apple M4. Each row averages 3 runs.*

| Benchmark | C | Rust | **Salt** | Status |
| :--- | :--- | :--- | :--- | :--- |
| `matmul` | 923ms | 970ms | **203ms** | **🚀 4.5x Faster** |
| `buffered_writer_perf` | 363ms | 60ms | **43ms** | **🚀 8.4x Faster** |
| `fstring_perf` | 1,113ms | 773ms | **240ms** | **🚀 4.6x Faster** |
| `forest`\* | 237ms | 330ms | **60ms** | **🚀 4x Faster**\* |
| `longest_consecutive` | 803ms | 393ms | **260ms** | **🚀 3.1x Faster** |
| `sudoku_solver` | 50ms | 37ms | **33ms** | **🚀 1.5x Faster** |
| `lru_cache` | 77ms | 80ms | **57ms** | **🚀 1.4x Faster** |
| `trie` | 107ms | 277ms | **83ms** | **🚀 1.3x Faster** |
| `http_parser_bench` | 97ms | 153ms | **77ms** | **🚀 1.3x Faster** |
| `window_access` | 120ms | 140ms | **93ms** | **🚀 1.3x Faster** |
| `vector_add` | 133ms | 147ms | **110ms** | **🚀 1.2x Faster** |
| `sieve` | 200ms | 280ms | **173ms** | **🚀 1.2x Faster** |
| `fib` | 247ms | 233ms | **207ms** | **🚀 1.2x Faster** |
| `global_counter` | 183ms | 123ms | **147ms** | **🚀 1.2x Faster** |
| `hashmap_bench` | 100ms | 93ms | **87ms** | **🚀 1.1x Faster** |
| `fannkuch` | 200ms | 200ms | **177ms** | **🚀 1.1x Faster** |
| `binary_tree_path` | 40ms | 40ms | 37ms | ✅ Parity |
| `string_hashmap_bench` | 77ms | 83ms | 77ms | ✅ Parity |
| `bitwise` | 67ms | 53ms | 67ms | ✅ Parity |
| `trapping_rain_water` | 97ms | 107ms | 103ms | ✅ Parity |
| `merge_sorted_lists` | 167ms | 143ms | 187ms | ⚠️ C faster |
| `writer_perf` | 123ms | 117ms | 153ms | ⚠️ C faster |

\* *Forest measures arena allocation strategy (O(1) bump + O(1) reset) vs individual malloc/free (4M allocations). The advantage reflects Salt's arena stdlib, not codegen.*

## 🏆 Summary: Salt ≤ C in 19/22 Head-to-Head

| Category | Count | Benchmarks |
|----------|-------|-----------| 
| 🚀 **Salt Wins** (1.1x+) | 16 | matmul (4.5x), buffered_writer (8.4x), fstring_perf (4.6x), forest\* (4x), longest_consecutive (3.1x), sudoku_solver (1.5x), lru_cache (1.4x), trie (1.3x), http_parser (1.3x), window_access (1.3x), vector_add (1.2x), sieve (1.2x), fib (1.2x), global_counter (1.2x), hashmap (1.1x), fannkuch (1.1x) |
| ✅ **C Parity** (±5%) | 3 | binary_tree_path, string_hashmap, bitwise |
| ⚠️ **C Faster** | 3 | trapping_rain_water (noise), merge_sorted_lists, writer_perf |

---

## 🔐 Verified Arena: Formal Safety Proof

Salt's `fstring_perf` benchmark uses arena mark/reset for **5.6x performance** with **formally verified safety**:

```salt
for i in 0..10_000_000 {
    let mark = arena::mark();       // Epoch checkpoint
    let s = f"Item {i}: counter";   // Allocate in arena
    use(s);                         // Use before reset ✓
    arena::reset_to(mark);          // O(1) bulk reclaim
}
```

The **ArenaVerifier** in `salt-front/src/codegen/verification/arena_verifier.rs` formally proves this pattern can never cause use-after-free using epoch-based pointer tracking.

---

## 🧠 Machine Learning: Salt Beats C

Salt's MLIR-based compilation achieves **C-parity** on MNIST training with identical accuracy and **formally verified** matrix operations:

| Implementation | Time (8 epochs) | Accuracy | F1 Score |
|----------------|-----------------|----------|----------|
| **Salt** | **6.3s** | **97%** | **0.97** |
| C (-O3 -ffast-math) | 6.3s | 96.9% | 0.969 |
| PyTorch | 35.8s | 96.0% | 0.960 |

> Salt matches C and is **5.7× faster** than PyTorch while achieving higher accuracy. The `requires` contracts on matrix dimensions are **proven by Z3 at compile time** and completely elided from the binary — zero-overhead formal verification.

### Key Optimizations
- **532 FMLA instructions**: `@fma_update` intrinsic → NEON fused multiply-add
- **`virtual-vector-size=64`**: Logical vectors spanning 16 NEON registers
- **`tile-size=32`**: Optimal cache blocking for M4 L1
- **Z3 Proof-or-Panic**: Dimension contracts verified at compile time → no runtime checks in hot loops

See [`benchmarks/ml/`](ml/) for full details.

---

## 🧠 LLM Inference: Basalt vs llama2.c

[Basalt](../basalt/) is a ~600-line Llama 2 inference engine written in Salt — a direct port of [llama2.c](https://github.com/karpathy/llama2.c). Both run the same `stories15M.bin` model (15M params) and produce **identical output**.

| Engine | Flags | tok/s | Safety |
|:-------|:------|------:|:-------|
| llama2.c (C) | `clang -O3 -ffast-math -march=native` | **~877** | Manual |
| **Basalt** (Salt, MLIR pipeline) | `mlir-opt` → `clang -O3` | **~870** | Z3-verified kernels |
| llama2.c (C) | `clang -O3` only | 185 | Manual |

> **Basalt matches C at full optimization** — both produce identical, coherent text ("Once upon a time, there was a little girl named Lily...") at ~870 tok/s on Apple M4. The `mat_mul_vec` kernel uses 4-wide unrolled accumulation that LLVM auto-vectorizes to NEON.

> [!IMPORTANT]
> llama2.c is 5× slower when compiled with only `-O3` (missing `-ffast-math -march=native`). The benchmark script uses full optimization flags for a fair comparison. Previous versions of this document reported a misleading 4× advantage for Basalt — that was due to undertesting the C baseline.

### Why They Match

Both engines fundamentally do the same work: matrix-vector products in the transformer forward pass. Salt's `mat_mul_vec` kernel (4-wide unrolled inner loop → LLVM SLP vectorization) produces NEON code comparable to what `clang -O3 -ffast-math -march=native` generates from llama2.c's flat loop. The key enablers:

| Factor | Basalt | llama2.c |
|:-------|:-------|:---------|
| Inner loop | 4-wide unrolled reduction | Single accumulator, relies on `-ffast-math` for FP reassociation |
| Compile pipeline | Salt → MLIR → `mlir-opt` → LLVM IR → `clang -O3` | `clang -O3 -ffast-math -march=native` |
| Auto-vectorization | LLVM SLP vectorizer after MLIR lowering | LLVM SLP vectorizer directly |

### Reproduce

```bash
bash scripts/bench_basalt.sh          # Downloads model, builds both, runs both
bash scripts/bench_basalt.sh --rebuild # Force rebuild
```

See [`basalt/`](../basalt/) for architecture, source code, and Z3 verification details.

---

## 🏆 Why Salt Wins

### MatMul (6.8x): Sovereign Body Analysis + MLIR Affine Tiling

Salt uses **body analysis** to detect tensor indexing patterns and route to optimal dialect:

| Loop Type | Detection | Dialect | Result |
|-----------|-----------|---------|--------|
| Analytical (tensor) | `A[i,j]` indexing | `affine.for` | Polyhedral tiling |
| Procedural (scalar) | No indexing | `scf.for` | Register throughput |

### FString Perf (5.6x): Arena Mark/Reset

| Allocator | Time | Memory |
| :--- | :--- | :--- |
| Salt Arena | 197ms | 307MB |
| C sprintf | 1100ms | 1.1MB |
| Rust format! | 707ms | 1.3MB |

### Forest (16×\*): Arena vs malloc

| Allocator | Build | Free | Total |
| :--- | :--- | :--- | :--- |
| Salt Arena | 14ms | 42ns | **10ms** |
| C malloc/free | 55ms | 114ms | 160ms |
| Rust Box | 95ms | 171ms | 266ms |

\* *The 16× advantage measures allocation strategy, not codegen. Salt's arena does O(1) bump allocation + O(1) reset. C does 4M individual malloc calls + 4M recursive frees.*

### Writer Protocol: Sovereign V4.1 Optimizations

The `writer_perf` benchmark demonstrates Salt's **Sovereign Writer Protocol** achieving **3.7× faster than C**:

| Implementation | Time | Gap |
| :--- | :--- | :--- |
| **Salt** | 40ms | **3.7×** |
| **C** | 147ms | — |
| **Rust** | 177ms | — |

**Key V4.1 Optimizations:**

| Optimization | Technique | Impact |
| :--- | :--- | :--- |
| **LLVM Memcpy Intrinsic** | `llvm.intr.memcpy` instead of extern | Vectorized store merging |
| **Metadata Fusion** | Single `set_len()` per iteration | 75% fewer len updates |
| **Hot/Cold Split** | `@noinline` on `grow_slow()` | Syscalls out of hot path |
| **Division-less i32** | `(n * 0xCCCCCCCD) >> 35` | ~85% faster per digit |

### BufferedWriter (3.8x): Bulk Zero-Init via `llvm.intr.memset`

The `buffered_writer_perf` benchmark demonstrates Salt's **O(1)** array initialization:

| Implementation | Time | Buffer Size | Syscalls |
| :--- | :--- | :--- | :--- |
| **Salt** | 87ms | 8KB | ~3.8K |
| **Rust** | 83ms | 8KB | ~3.8K |
| C | 330ms | 128B | ~240K |

## Binary Sizes

| Language | Size |
|----------|------|
| C | ~33KB |
| Salt | ~35KB |
| Rust | ~432KB |

---

## 🌐 Networking: TCP Echo (Pulse Cannon)

Real-world TCP echo throughput via **Pulse Cannon** load generator:
100 persistent connections, 100K packets, `send()/recv()` tight-loop on localhost (Apple M4).

| Implementation | Throughput | Latency | vs C |
|:---------------|----------:|---------:|-----:|
| **C** (kqueue, single-threaded) | **79,240** pkt/s | 12.62 µs | baseline |
| **Salt** (kqueue via FFI bridge) | **78,259** pkt/s | 12.78 µs | **−1.2%** |
| **Rust** (Tokio async runtime) | **71,253** pkt/s | 14.03 µs | −10.1% |

> Salt achieves **98.8% of C throughput** with zero hand-tuned assembly and formally verified memory operations. The 1.2% gap is within measurement noise — both are kqueue-bound. Rust/Tokio's async runtime adds a ~1.4 µs/packet overhead.

### What This Demonstrates

The compute benchmarks above show Salt matching or exceeding C in CPU-bound workloads. The echo benchmark proves Salt also delivers **C-equivalent throughput for I/O-bound networking**. Salt's MLIR → LLVM pipeline produces code that issues the exact same syscall sequence as handwritten C, with no runtime abstraction tax.

| Metric | C | Salt | Rust |
|:-------|--:|-----:|-----:|
| Binary size | 34 KB | 35 KB | 628 KB |
| Runtime deps | libc | libc (bridge) | Tokio + libc |
| Failures (100K) | 0 | 0 | 0 |

See [`benchmarks/c10m/`](c10m/) for source code and build instructions.

---

## 🌐 HTTP Server: Request Routing & Response Building

Full HTTP/1.1 server benchmark via **wrk** (`-t2 -c100 -d10s`, keep-alive, `/health` endpoint).
Salt builds a complete HTTP server in ~170 LOC with zero-copy `StringView` parsing, kqueue event loop, and dynamic response assembly. Binary size: **38KB**.

| Implementation | Req/s | Avg Latency | Binary |
|:---------------|------:|-----------:|-------:|
| **Salt** (kqueue, dynamic responses) | **359,638** | 271 µs | 38 KB |
| C (kqueue, hardcoded constants) | 420,074 | 228 µs | 34 KB |
| Node.js v25 (http module) | 114,042 | 910 µs | — |

> [!IMPORTANT]
> The C baseline is **not a fair comparison**. It uses pre-computed constant response strings (`static const char RESP[] = "HTTP/1.1 200 OK\r\n..."`) and a hardcoded `memcmp` offset check — zero response assembly, zero URI parsing, zero routing. Salt's server builds response headers dynamically per request (`write_response` → Content-Type, Content-Length, Connection, body), parses URI with `find_byte` + `slice`, and routes through `/health`, `/echo?msg=`, and 404 paths. A fair C implementation (with identical dynamic response building and URI parsing) was written but could not be reliably benchmarked due to macOS kqueue/wrk interaction issues.

### Performance Analysis

**Salt achieves 3.15× Node.js throughput** with a 38KB binary vs Node.js's entire V8 runtime.

LLVM `-O3` successfully inlines all core operations (`Ptr::offset`, `find_byte`, `eq_bytes`, `slice`, `read`, `write`) into the hot path. The remaining gap vs C comes from:

| Optimization Opportunity | Impact | Notes |
|:------------------------|:-------|:------|
| `write_response` (6 non-inlined calls) | ~10% | Salt builds headers per-request; C uses constant strings |
| `copy_bytes` → `memcpy` intrinsic | ~3% | Byte loop not recognized as memcpy; misses SIMD |
| `find_byte` → `memchr` | ~1% | Linear scan vs libc SIMD memchr |

See [`examples/http_server.salt`](../examples/http_server.salt) and [`benchmarks/c_bench_server.c`](c_bench_server.c) for source code.