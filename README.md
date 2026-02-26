# Salt

**Systems programming, mathematically verified.**

Salt is an ahead-of-time compiled systems language that combines the performance characteristics of C with formal verification through an embedded Z3 theorem prover. Programs are compiled through an MLIR multi-dialect pipeline: polyhedral loop tiling, register-pressure-aware scheduling, and arena escape analysis operate at a level of granularity unavailable to traditional single-IR compilers.

[![Benchmarks](https://img.shields.io/badge/vs_C-19%2F22_Won_or_Parity-brightgreen?style=flat-square)](benchmarks/BENCHMARKS.md)
[![Z3 Verified](https://img.shields.io/badge/Safety-Z3_Verified-blue?style=flat-square)](docs/ARCH.md)
[![70+ Stdlib Modules](https://img.shields.io/badge/Stdlib-70%2B_Modules-orange?style=flat-square)](salt-front/std/README.md)

```salt
package main

use std.collections.HashMap

fn main() {
    let mut map = HashMap<StringView, i64>::new();
    map.insert("hello", 1);
    map.insert("world", 2);

    let result = map.get("hello") |?> println(f"Found: {_}");

    for entry in map.iter() {
        println(f"{entry.key}: {entry.value}");
    }
}
```

---

## Approach

Most systems languages force a choice between performance and safety. C gives you control but no guardrails. Rust introduces a borrow checker that prevents a class of memory errors at the cost of significant annotation burden and a steep learning curve. Neither embeds a general-purpose theorem prover.

Salt takes a different path. The compiler integrates Z3 as a first-class verification backend: developers write `requires` and `ensures` contracts on functions, and the compiler synthesizes proof obligations that must discharge before code generation proceeds. When Z3 cannot prove a postcondition, the compiler emits a counterexample: concrete input values that violate the contract, rather than a type error.

Memory is managed through arenas with compile-time escape analysis. No garbage collector, no lifetime annotations, no borrow checker. The `ArenaVerifier` proves statically that no reference outlives its region, giving you the performance profile of manual allocation with the safety properties of managed memory.

## Multi-Dialect Compilation

The compiler routes code through multiple MLIR dialects depending on the optimization opportunity:

| Pattern | Dialect | Optimization |
|---------|---------|-------------|
| Tensor/matrix loops | `affine.for` | Polyhedral tiling, loop fusion |
| Scalar-heavy loops | `scf.for` | Register pressure optimization |
| Branching control flow | `cf` + `llvm` | Standard LLVM backend |
| Arena operations | Custom lowering | Escape analysis, bulk free |

This is the mechanism behind Salt's performance results. When a matmul kernel is compiled through the affine dialect, MLIR can tile the iteration space for cache locality in a way that a flat LLVM IR representation cannot express. The compiler emits 120 unique MLIR operations across these dialects.

## Performance

All benchmarks use runtime-dynamic inputs to prevent constant folding, and results are printed to prevent dead code elimination. Each measurement averages 3 runs with cached binaries. Full methodology is documented in the [benchmark suite](benchmarks/BENCHMARKS.md).

*Verified February 21, 2026 on Apple M4*

| Benchmark | Salt | C (`clang -O3`) | Rust | vs. C |
|-----------|------|-----------------|------|-------|
| **matmul** (1024³) | **203ms** | 923ms | 970ms | 4.5× |
| **buffered_writer** | **43ms** | 363ms | 60ms | 8.4× |
| **fstring_perf** (10M) | **240ms** | 1,113ms | 773ms | 4.6× |
| **forest** (depth-22)\* | **60ms** | 237ms | 330ms | 4×\* |
| **longest_consecutive** | **260ms** | 803ms | 393ms | 3.1× |
| **http_parser** | **77ms** | 97ms | 153ms | 1.3× |
| **trie** | **83ms** | 107ms | 277ms | 1.3× |
| **vector_add** | **110ms** | 133ms | 147ms | 1.2× |
| **sudoku_solver** | **33ms** | 50ms | 37ms | 1.5× |
| **lru_cache** | **57ms** | 77ms | 80ms | 1.4× |
| **window_access** | **93ms** | 120ms | 140ms | 1.3× |
| **hashmap_bench** | **87ms** | 100ms | 93ms | 1.1× |
| sieve (10M) | 173ms | 200ms | 280ms | 1.2× |
| fib | 207ms | 247ms | 233ms | 1.2× |
| fannkuch | 177ms | 200ms | 200ms | 1.1× |
| global_counter | 147ms | 183ms | 123ms | 1.2× |
| binary_tree_path | 37ms | 40ms | 40ms | parity |
| string_hashmap | 77ms | 77ms | 83ms | parity |
| bitwise | 67ms | 67ms | 53ms | parity |
| trapping_rain_water | 103ms | 97ms | 107ms | 0.9× |
| merge_sorted_lists | 187ms | 167ms | 143ms | 0.9× |
| writer_perf | 153ms | 123ms | 117ms | 0.8× |

**Salt ≤ C in 19/22** head-to-head benchmarks. 28 total (including 6 Salt-only). 0 build failures. Binary size ~38KB (vs Rust ~430KB).

\* *Forest measures arena allocation strategy (O(1) bump + O(1) reset) vs individual malloc/free. The advantage is Salt's arena stdlib, not codegen.*

## Verified Safety

Contracts are proof obligations, distinct from assertions. The compiler does not insert runtime checks; it proves them unnecessary.

```salt
fn binary_search(arr: &[i64], target: i64) -> i64
    requires(arr.len() > 0)
    ensures(result >= -1)
{
    let mut lo: i64 = 0;
    let mut hi: i64 = arr.len() - 1;

    while lo <= hi {
        let mid = lo + (hi - lo) / 2;
        if arr[mid] == target {
            return mid;
        } else if arr[mid] < target {
            lo = mid + 1;
        } else {
            hi = mid - 1;
        }
    }
    return -1;
}
```

This function compiles **only if** Z3 can discharge the `ensures(result >= -1)` obligation across every execution path. If it cannot, compilation fails with a concrete counterexample.

## Arena Memory

```salt
fn process_request(request: &Request) -> Response {
    let arena = Arena::new(4096);       // 4KB region
    let mark = arena.mark();            // Save position

    let parsed = parse_headers(&arena, request);
    let response = build_response(&arena, parsed);

    arena.reset_to(mark);              // O(1) bulk free
    return response;
}
```

The `ArenaVerifier` proves at compile time that no reference escapes its arena. This provides the performance of `malloc`/`free` while ensuring safety through verification rather than runtime checks.

## Case Studies

### LETTUCE: Redis-compatible data store

[LETTUCE](lettuce/) is a Redis-compatible in-memory key-value store written in Salt.

| Metric | LETTUCE (Salt) | Redis (C) |
|--------|---------------|-----------|
| **Throughput** | **234,000 ops/sec** | 115,000 ops/sec |
| **Source** | 567 lines | ~100,000 lines |
| **Memory model** | Arena + Swiss-table | jemalloc + dict |

2× Redis throughput at 0.6% of the code size. [Architecture →](lettuce/)

### Basalt: Llama 2 inference

[Basalt](basalt/) is a ~600-line Llama 2 forward pass with BPE tokenizer, a direct port of [llama2.c](https://github.com/karpathy/llama2.c).

| Metric | Basalt (Salt) | llama2.c (C) |
|--------|--------------|--------------|
| **tok/s** (stories15M, M4) | **~870** | ~877 |
| **Source** | ~600 lines | ~700 lines |
| **Safety** | Z3-verified kernels | Manual |

C-parity inference speed with compile-time proofs on every matrix operation. [Architecture →](basalt/)

### Facet: GPU-accelerated 2D compositor

[Facet](user/facet/) is a full-stack 2D rendering engine: Bézier flattening, scanline rasterization, and Metal compute are implemented in Salt with Z3-verified bounds on every pixel write.

| Metric | Salt (MLIR) | C (`clang -O3`) |
|--------|-------------|-----------------|
| **Per frame** (512×512 tiger) | 2,186 μs | 2,214 μs |
| **Throughput** | 457 fps | 451 fps |

Salt's MLIR codegen matches `clang -O3` on a real rendering pipeline with ~160 cubic Bézier curves. [Architecture →](user/facet/)

## Syntax

```salt
// Pipe operator — Unix-style data flow
let result = data
    |> parse(_)
    |> validate(_)
    |> transform(_);

// Error propagation with fallback
let config = File::open("config.toml")? |?> default_config();

// Pattern matching
match response.status {
    200 => handle_success(response.body),
    404 => println("Not found"),
    err => println(f"Error: {err}"),
}

// Generics with arena allocation
struct Vec<T, A> {
    data: Ptr<T>,
    len: i64,
    cap: i64,
    arena: &A,
}

impl Vec<T, A> {
    fn push(&mut self, value: T) {
        if self.len == self.cap {
            self.grow();
        }
        self.data.offset(self.len).write(value);
        self.len = self.len + 1;
    }
}
```

## Standard Library

70+ modules with no external dependencies. [Reference →](salt-front/std/README.md)

| Package | Modules |
|---------|---------|
| `std.collections` | `Vec<T,A>`, `HashMap<K,V>` (Swiss-table), `Slab<T>` |
| `std.string` | `String`, `StringView`, f-string interpolation |
| `std.net` | `TcpListener`, `TcpStream`, `Poller` (kqueue) |
| `std.http` | HTTP client & server, zero-copy parsing |
| `std.sync` | `Mutex`, `AtomicI64` (C11 atomics) |
| `std.thread` | `Thread::spawn`, `Thread::join` |
| `std.json` | JSON parsing & value access |
| `std.io` | `File`, `BufferedWriter`, `BufferedReader` |
| `std.math` | Vectorized transcendentals, NEON SIMD |
| `std.nn` | `relu`, `sigmoid`, `softmax`, `cross_entropy` |
| `std.crypto` | TLS bridge |
| `std.fs` | File system operations |

## Getting Started

### Prerequisites

| Dependency | Version | Install (macOS) |
|:-----------|:--------|:----------------|
| **Rust** | 1.75+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| **Z3** | 4.12+ | `brew install z3` |
| **MLIR/LLVM** | 18+ | `brew install llvm@18` |

> [!IMPORTANT]
> Z3 is required. The compiler links against `libz3` for verification. If you see `ld: library not found for -lz3`:
> ```bash
> export DYLD_LIBRARY_PATH=/opt/homebrew/lib:$DYLD_LIBRARY_PATH
> ```

### With `sp` (recommended)

```bash
# Install the Salt package manager
cd tools/sp && cargo install --path . && cd ../..

# Create, build, and run
sp new hello_world && cd hello_world
sp run
# 🧂 Hello from hello_world!
```

`sp` provides content-addressed caching and cross-package Z3 contract verification. [Design →](tools/sp/)

### Without `sp`

```bash
cd salt-front && cargo build --release && cd ..
./salt-front/target/release/salt-front examples/hello_world.salt -o hello
DYLD_LIBRARY_PATH=/opt/homebrew/lib ./hello
```

> [!TIP]
> If `cargo build` fails with Z3 errors: `ls /opt/homebrew/lib/libz3.*`
> If MLIR tools are missing: `export PATH=/opt/homebrew/opt/llvm@18/bin:$PATH`

## Project Structure

```
lattice/
├── salt-front/           # Compiler: parser → typechecker → Z3 verifier → MLIR emitter
│   └── std/              # Standard library (70+ modules, written in Salt)
├── basalt/               # Llama 2 inference engine (~600 lines)
├── benchmarks/           # 22 benchmarks with C & Rust baselines
├── examples/             # 7 progressively complex Salt programs
├── kernel/               # Lattice microkernel (boots in QEMU)
├── lettuce/              # Redis-compatible data store
├── user/facet/           # GPU 2D compositor (raster, Metal, UI)
├── docs/                 # Spec, architecture, deep-dives
└── tools/
    ├── sp/               # Package manager
    ├── salt-lsp/         # LSP server (diagnostics, completions)
    └── salt-build/       # Legacy build tool
```

## Documentation

| Document | |
|----------|--|
| [Language Spec](docs/SPEC.md) | Complete language specification |
| [Architecture](docs/ARCH.md) | Compiler pipeline & MLIR design |
| [Benchmarks](benchmarks/BENCHMARKS.md) | Full results & methodology |
| [Arena Safety](docs/deep-dives/arena-safety.md) | Compile-time escape analysis |
| [Performance](docs/deep-dives/performance.md) | Why Salt beats C |
| [Design Pillars](docs/philosophy/PILLARS.md) | Fast · Ergonomic · Verified |
| [Syntax Reference](SYNTAX.md) | Complete syntax guide |

## Project Stats

*As of February 18, 2026 · commit `0b8cf69`*

| | |
|---|---|
| **Total lines of code** | 151,031 |
| **Languages** | 12 (Rust, Salt, C, x86 assembly, Python, Shell, HTML, CSS, JS, TOML, Markdown, linker scripts) |

### By language:

| Language | LOC | Files |
|----------|----:|------:|
| Rust | 76,948 | 217 |
| Salt | 41,469 | 513 |
| C / Headers | 11,040 | — |
| Python | 7,976 | — |
| Shell | 3,309 | — |
| HTML | 2,338 | — |
| Assembly (x86) | 841 | — |

### Compiler (`salt-front`):

| | |
|---|---|
| Compiler source | 57,456 lines across 87 codegen files |
| MLIR ops emitted | 120 unique operations |
| Z3 integration points | 1,284 references |
| `unsafe` blocks | 31 |
| Structs / Enums | 475 / 135 |

### Testing:

| | |
|---|---|
| Rust `#[test]` functions | 1,318 |
| Salt test files | 118 |
| Total test LOC | 22,294 |
| Test-to-source ratio | ~15% |

### Salt ecosystem:

| | |
|---|---|
| Functions defined | 1,530 |
| Structs defined | 313 |
| `requires`/`ensures` contracts | 118 |
| Distinct attributes | 26 |
| Stdlib modules | 14 (982 LOC) |
| Benchmark programs | 60 (4,352 LOC) |

> Regenerate with `./scripts/project_stats.sh` or `./scripts/project_stats.sh --json`.

## Status

Salt is pre-1.0 and under active development. The compiler, standard library, and tooling are functional and benchmarked. Expect breaking changes.

| Component | Status | Version |
|-----------|--------|---------|
| Compiler (`salt-front`) | ✅ Compiles all benchmarks and examples | v0.7.0 |
| Standard Library | ✅ 70+ modules, production-tested in LETTUCE | v0.7.0 |
| Z3 Verification | ✅ Contracts + @pulse async cycle-budget proofs | v0.7.0 |
| Benchmarks | ✅ 19/22 at C-parity or better | — |
| LSP Server | ✅ Diagnostics, go-to-definition, completions | v0.1.0 |
| Package Manager (`sp`) | 🚧 Builds from `salt.toml` | v0.1.0 |
| Basalt (LLM Inference) | ✅ Llama 2 forward pass, tokenizer, mmap | v0.3.0 |
| Lattice Kernel | ✅ 4-core SMP, Universal Task Pointer (invoke_task 29cy, async yield 111cy, preempt 430cy, spawn 99cy), Ring 3 SYSCALL (102cy), slab CAS (103cy), Ring 3 Isolation (SWAPGS, KPTI CR3, user_stack_init), SIP IPC (188cy) | v0.8.0 |
| Facet Compositor | ✅ Rasterizer, window, Metal, benchmarked vs C | v0.3.0 |

## License

MIT
