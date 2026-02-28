# Salt + Lattice

**A Sovereign Microkernel for High-Performance Distributed Workloads,**
**built in a systems language with embedded formal verification.**

Salt is an ahead-of-time compiled systems language that combines the performance of C with compile-time safety through an embedded Z3 theorem prover. Lattice is a microkernel operating system written entirely in Salt, achieving unikernel-level latency while maintaining hardware-enforced Ring 0 / Ring 3 isolation.

Together, they form a single system where the language's "superpowers" — formal verification and MLIR-based lowering — become the operating system's superpowers: zero-trap IPC, proof-carrying descriptors, and cache-line-deterministic data planes.

[![Benchmarks](https://img.shields.io/badge/vs_C-18%2F22_Won_or_Parity-brightgreen?style=flat-square)](benchmarks/BENCHMARKS.md)
[![Z3 Verified](https://img.shields.io/badge/Safety-Z3_Verified-blue?style=flat-square)](docs/ARCH.md)
[![70+ Stdlib Modules](https://img.shields.io/badge/Stdlib-70%2B_Modules-orange?style=flat-square)](salt-front/std/README.md)
[![Lattice Kernel](https://img.shields.io/badge/Kernel-Sovereign_Microkernel-purple?style=flat-square)](kernel/)

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

## Why Salt + Lattice?

Most operating systems are written in C (Linux, Xv6) or C++ (Fuchsia, seL4). They rely on extensive runtime checks, POSIX syscall conventions, and manual memory management. Salt replaces all three with **compile-time proofs**, **zero-trap shared memory**, and **arena-based allocation** — giving Lattice the performance of a unikernel with the isolation guarantees of a microkernel.

### The Three Pillars

#### 🔥 Pillar A: Zero-Trap Data Plane (SPSC + Shared Memory)

**Salt's Superpower:** High-performance, low-level memory control with MLIR-optimized lowering.

**Lattice's Derivative:** Instead of legacy POSIX syscalls (`read`/`write`) that trap into the kernel on every packet, Lattice uses Shared Memory SPSC (Single-Producer, Single-Consumer) Rings. The networking stack (NetD) and storage stack (LatticeStore) run as Ring 3 "System Daemons" that communicate with the kernel through lock-free ring buffers in shared pages.

```
Traditional OS:  App → syscall → trap → kernel copy → return    (~1000 cycles)
Lattice:         App → SPSC write → shared memory → NetD reads  (~150 cycles)
```

The kernel's **only** role in the data plane is pushing raw Ethernet frames into the SPSC ring and firing a wake notification. All protocol parsing (ARP, TCP, IP) happens in Ring 3, isolating the kernel from packet-parsing RCE vulnerabilities.

#### 🔒 Pillar B: The Formal Shadow (Z3-Verified Sovereignty)

**Salt's Superpower:** A built-in Z3 verification gate that proves memory safety and alignment at compile time.

**Lattice's Derivative:** Proof-Carrying IPC. The compiler "seals" a Z3 proof into a 64-bit `proof_hint` embedded in every SPSC descriptor. The NetD arbiter verifies this hint in *O(1)* time (two CPU instructions: alignment mask + bitwise compare).

```salt
// At compile time, Z3 proves @align(64) fields are on separate cache lines.
// The compiler seals this proof:
//   proof_hint = hash_combine(struct_id, field_offset, alignment)
// The arbiter validates the seal before touching any shared memory.

struct SpscDescriptor {
    ptr: u64,           // Must be 64-byte aligned (mechanical check)
    len: u32,
    proof_hint: u64,    // Z3-sealed "Right to Access" token
}
```

This eliminates the "Security Tax." We don't need expensive runtime bounds checks because the hardware (MMU page tables) and the math (Z3 SMT solver) have already validated the memory access before the binary is even loaded.

#### ⚡ Pillar C: Mechanical Sympathy (The Cache-Line Guarantee)

**Salt's Superpower:** First-class support for physical memory layout via the `@align(N)` attribute with Z3-verified struct padding.

**Lattice's Derivative:** False-sharing elimination. Lattice SPSC rings are formally proven to isolate Producer and Consumer indexes on separate L3 cache lines:

```salt
struct SpscRing {
    @align(64)
    head: u64,         // Producer-owned (cache line 0)
    capacity: u64,

    @align(64)
    tail: u64,         // Consumer-owned (cache line 1)
}
// Z3 PROVED: head at offset 0, tail at offset 64 (z3_align_verified)
```

This targets the **Cycles per Packet (Cpp)** KPI. We aren't just fast; we are *deterministic*. No cache-line "ping-pong" between cores, no prefetcher-induced jitter, no false-sharing invalidation storms.

---

## Approach

Salt takes a different path. The compiler integrates Z3 as a first-class verification backend: developers write `requires` preconditions and `ensures` postconditions on functions, and the compiler checks each contract using Z3. Preconditions are verified at every call site; postconditions are verified at every return site using Weakest Precondition (WP) generation with path-sensitive branch analysis. When Z3 proves the condition always holds, the check is elided entirely — zero runtime cost. When Z3 finds a concrete counterexample, it reports the violating values. When neither can be determined, the compiler emits a standard runtime assertion as a fallback.

Memory is managed through arenas with compile-time escape analysis. No garbage collector, no lifetime annotations, no borrow checker. The `ArenaVerifier` verifies statically that no reference outlives its region, giving you the performance profile of manual allocation with the safety properties of managed memory.

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

*Verified February 27, 2026 on Apple M4*

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

**Salt ≤ C in 18/22** head-to-head benchmarks. 28 total (including 6 Salt-only). 0 build failures. Binary size ~38KB (vs Rust ~430KB).

The "Abstraction Tax" is zero: Salt's Z3 verification, arena memory, and MLIR pipeline add **no runtime overhead**. The proofs discharge at compile time, the arenas free in O(1), and MLIR optimizes the same way LLVM does — or better, when polyhedral tiling applies.

\* *Forest measures arena allocation strategy (O(1) bump + O(1) reset) vs individual malloc/free. The advantage is Salt's arena stdlib, not codegen.*

## Verified Safety

Contracts are proof obligations checked by Z3 at compile time. When Z3 can prove a `requires` precondition holds at a call site, the check is elided entirely — zero runtime cost. When it cannot, the compiler emits a runtime assertion as a safe fallback.

```salt
fn binary_search(arr: &[i64], target: i64) -> i64
    requires(arr.len() > 0)
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

Z3 verifies `requires(arr.len() > 0)` at every call site. Passing an empty array is a compile-time error with a concrete counterexample. Passing a non-empty array causes the check to be elided — the binary contains no guard.

### Postconditions (v0.9.2)

`ensures` postconditions are verified at every return site using Weakest Precondition (WP) generation. The compiler tracks branch conditions through the control flow graph and provides Z3 with path-sensitive context at each exit point:

```salt
fn absolute_value(x: i32) -> i32
    ensures(result >= 0)
{
    if x < 0 {
        return -x;    // Z3 proves: given x < 0, -x >= 0  ✓
    }
    return x;         // Z3 proves: given !(x < 0), x >= 0  ✓
}

fn clamp_to_unit(val: i32) -> i32
    ensures(result >= 0 && result <= 100)
{
    if val < 0   { return 0; }
    if val > 100 { return 100; }
    return val;       // Z3 proves: given !(val < 0) && !(val > 100), 0 <= val <= 100  ✓
}
```

Every `return` site becomes a Z3 proof obligation. Guard clauses with early returns automatically narrow the path conditions — Z3 knows that surviving `if x < 0 { return -x; }` implies `x >= 0`.

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

The `ArenaVerifier` checks at compile time that no reference escapes its arena. This provides the performance of `malloc`/`free` while ensuring safety through static analysis rather than runtime checks.

## Lattice Kernel Architecture

Lattice is a **Sovereign Microkernel**: the kernel provides only memory management (PMM, VMO), scheduling (4-core SMP, preemptive), and IPC (SPSC rings via `sys_shm_grant`). Everything else — networking, storage, device drivers — runs in Ring 3 as isolated System Daemons.

```
┌─────────────────────────────────────────────────────┐
│                    Ring 3 (User)                     │
│  ┌──────────┐  ┌───────────┐  ┌──────────────────┐  │
│  │   NetD   │  │  LatticeFS│  │   User Programs  │  │
│  │ (TCP/IP) │  │ (Storage) │  │                  │  │
│  └────┬─────┘  └─────┬─────┘  └────────┬─────────┘  │
│       │              │                 │             │
│  ═════╪══════════════╪═════════════════╪═════════    │
│       │    SPSC Shared Memory Rings    │             │
│  ═════╪══════════════╪═════════════════╪═════════    │
│                                                      │
├──────────────────────────────────────────────────────┤
│                  Ring 0 (Kernel)                      │
│  ┌────────┐  ┌─────────┐  ┌────────┐  ┌──────────┐  │
│  │  PMM   │  │Scheduler│  │  IPC   │  │ VirtIO   │  │
│  │(Pages) │  │ (4-SMP) │  │ (SPSC) │  │(NIC/Blk) │  │
│  └────────┘  └─────────┘  └────────┘  └──────────┘  │
└──────────────────────────────────────────────────────┘
```

### Why Ring 3 Without the Speed Penalty?

Linux pays ~1000 cycles per syscall for context switching and kernel-to-user copies. Lattice pays ~150 cycles because:

1. **No trap:** The SPSC ring lives in shared memory (`sys_shm_grant`). Producers and consumers read/write directly — no kernel transition needed for data transfer.
2. **No copy:** The DMA buffer writes directly into the SPSC ring page. NetD reads from the same physical page mapped into its address space.
3. **No lock:** The ring is single-producer, single-consumer. Head and tail sit on separate cache lines (`@align(64)`), so there's no contention and no atomic CAS in the steady state.

### How Z3 Prevents Byzantine Corruption

A compromised Ring 3 process cannot corrupt the kernel because:

1. **Hardware gate (MMU):** Ring 3 cannot access Ring 0 memory. Period.
2. **Formal gate (Z3):** Every SPSC descriptor carries a `proof_hint` — a 64-bit seal generated at compile time by hashing the struct identity, field offset, and alignment. The NetD arbiter validates this seal in O(1) before touching any shared memory.
3. **Alignment gate:** Even if an attacker steals a valid `proof_hint`, the arbiter checks `(ptr & 0x3F) == 0` — the pointer must be physically 64-byte aligned. A shifted pointer is rejected regardless of the hint.

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
|--------|--------------| -------------|
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
├── kernel/               # Lattice Sovereign Microkernel
│   ├── core/             #   Scheduler (4-SMP), syscalls, process management
│   ├── net/              #   NetD bridge, TX bridge, ARP, TCP (Ring 3 daemons)
│   ├── lib/              #   IPC rings, arbiter, shared memory primitives
│   ├── mem/              #   PMM, VMO, slab allocator, user paging
│   ├── arch/             #   x86_64: GDT, IDT, TSS, SMP trampoline, APIC
│   └── drivers/          #   VirtIO (net, block), serial, PCI
├── basalt/               # Llama 2 inference engine (~600 lines)
├── benchmarks/           # 28 benchmarks with C & Rust baselines
├── examples/             # 7 progressively complex Salt programs
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
| [Lattice Benchmarks](docs/LATTICE_BENCHMARKS.md) | Kernel performance (syscall, SPSC, SHM) |
| [Benchmarks](benchmarks/BENCHMARKS.md) | Full Salt vs C/Rust results & methodology |
| [Arena Safety](docs/deep-dives/arena-safety.md) | Compile-time escape analysis |
| [Performance](docs/deep-dives/performance.md) | Why Salt beats C |
| [Design Pillars](docs/philosophy/PILLARS.md) | Fast · Ergonomic · Verified |
| [Syntax Reference](SYNTAX.md) | Complete syntax guide |

## Project Stats

*As of February 27, 2026*

| | |
|---|---|
| **Total lines of code** | 151,031+ |
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

Lattice is in the **v0.9.x "March to Sovereignty"** era — pursuing full formal verification (Z3-backed postconditions, loop invariants, and unified memory proofs) on the path to v1.0.0.

| Component | Version | Milestone |
| :--- | :--- | :--- |
| **Salt Compiler / Stdlib** | `v0.8.0` | Z3 Verification (requires + ensures), Multi-Dialect Codegen, Path-Sensitive WP |
| **Lattice Platform** (OS) | `v0.9.1` | Cache-Line IPC, SipHash-2-4 Proof Hints, Sovereign Reclaim |
| **Lattice Kernel** | `v0.9.1` | 4-Core SMP, Preemptive Scheduler, Ring 3 Isolation, Atomic Page Sweep |
| **Basalt** (LLM Inference) | `v0.3.0` | Proof-of-Concept (C-parity inference speed) |
| **Facet** (2D Compositor) | `v0.3.0` | Proof-of-Concept (Metal compute & verified rasterizer) |
| **Lettuce** (KV Store) | `v0.1.0` | Proof-of-Concept (234K ops/sec — 2x Redis throughput) |
| **Tooling** (LSP & `sp` Build) | `v0.1.0` | Foundation: diagnostics, completions, manifest parsing |

### Roadmap to v1.0.0

| Sprint | Objective | KPI |
|--------|-----------|-----|
| **v0.9.1** ✅ | Sovereign Foundation — Cache-line isolation, Proof-Carrying IPC, SipHash-2-4 Hardening, Sovereign Reclaim | Salt ≤ C 18/22, Reclamation < 1ms |
| **v0.9.2** ✅ | Postcondition Pivot — Z3-backed `ensures` for pure functions (Weakest Precondition generation, path-sensitive verification) | 6/6 postcondition tests GREEN |
| **v0.9.3** | Loop Sovereignty — `invariant` keyword, induction-based termination proofs | No unbounded loops in kernel |
| **v0.9.4** | Persistence — Block-VMO storage, NVMe SPSC bridge | Cold boot < 100ms |
| **v0.9.5** | Total Verification — Z3-unified arena bounds, SPSC pointer safety proofs | Zero algorithmic-only checks |
| **v1.0.0** | Lattice Sovereign — ABI freeze, self-hosting, incremental verification | All KPIs met, full proof |

## License

MIT
