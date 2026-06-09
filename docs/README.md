# Salt Documentation

Salt is a systems programming language with Z3-verified safety, arena-based memory, and MLIR codegen.

**Prerequisites**: Rust 1.75+, Z3 4.12+ (`brew install z3`), LLVM 21+ (`brew install llvm@21`).

## Key Differentiators

1. **Z3 Verification**: `requires()` and `ensures()` contracts proven at compile time
2. **Arena Memory**: O(1) bulk free via mark/reset — no GC, no borrow checker
3. **MLIR Pipeline**: Source → MLIR → LLVM → binary, with affine tiling for tensor ops
4. **Performance**: Close to baseline C without sacrificing safety.

## Documentation Index

### Syntax & Language

| Doc | Description |
|-----|-------------|
| [SYNTAX.md](../SYNTAX.md) | **Canonical syntax reference** — types, control flow, traits, verification, sugar |
| [SPEC.md](SPEC.md) | Language & compiler architecture — MLIR dialect specification |

### Architecture & Design

| Doc | Description |
|-----|-------------|
| [ARCH.md](ARCH.md) | Compiler pipeline, components, Z3 verification strategy |
| [PILLARS.md](philosophy/PILLARS.md) | Design philosophy: Fast · Ergonomic · Verified |
| [Region Model](philosophy/region-model.md) | Why regions beat borrow checking for bare-metal |

### Language Features

| Doc | Description |
|-----|-------------|
| [Move Semantics](MOVE_SEMANTICS.md) | Ownership and move tracking |
| [Closures](CLOSURES.md) | Closure capture semantics (current status + roadmap) |
| [RAII / Drop](RAII.md) | Resource management and destructors |
| [Unsafe](UNSAFE.md) | Unsafe blocks and raw pointer rules (stdlib-only) |
| [Concepts](CONCEPTS.md) | Verification constraints (Z3-backed) |

### Deep Dives

| Doc | Description |
|-----|-------------|
| [Arena Safety](deep-dives/arena-safety.md) | Compile-time arena escape analysis (the repo's best doc) |
| [Stand-up](deep-dives/stand-up.md) | Technology stack overview |
| [Universal ABI](deep-dives/universal-abi.md) | KeuOS Universal ABI design |
| [VirtIO MOE Convergence](deep-dives/virtio-moe-convergence.md) | Network stack convergence and Virtue Driver |

### KeuOS Kernel & OS

| Doc | Description |
|-----|-------------|
| [System ABI](abi/KEUOS_ABI.md) | Definitive ABI specification for targeting KeuOS |
| [Driver Model](keuos_driver_model.md) | Device drivers in KeuOS |


### Real-World Systems

| Project | Description |
|---------|-------------|
| [LETTUCE](../lettuce/) | Redis-compatible server exploring memory performance |
| [Basalt](../basalt/) | Llama 2 inference — Z3-verified kernels, mmap loading |
| [Facet](../user/facet/) | GPU 2D compositor — rasterizer, Metal compute |

### Benchmarks & Measurement

| Doc | Description |
|-----|-------------|
| [Benchmarks](../benchmarks/BENCHMARKS.md) | Official performance results |
| [Measurement](benchmarks/science-of-measurement.md) | Benchmarking methodology |

### Tutorials

| Doc | Description |
|-----|-------------|
| [Zero to Kernel](tutorial/zero-to-kernel.md) | Boot a KeuOS kernel in QEMU |

## Quick Start

```salt
package main

fn main() -> i32 {
    let mut sum = 0;
    for i in 0..100 {
        sum = sum + i;
    }
    println(f"Sum: {sum}");
    return sum;
}
```

```bash
cd salt-front && cargo build --release
./target/release/salt-front examples/hello_world.salt -o hello
DYLD_LIBRARY_PATH=/opt/homebrew/lib ./hello
```

> [!TIP]
> If `cargo build` fails with `ld: library not found for -lz3`, install Z3: `brew install z3`
