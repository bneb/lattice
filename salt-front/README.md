# Salt Compiler (`salt-front`)

The Rust-based compiler that transforms Salt source code into native ARM64 binaries via MLIR.

## Pipeline

```
Source.salt → Parse → Type Check → Z3 Verify → MLIR Emit → mlir-opt → LLVM IR → clang → Binary
```

## Architecture

| Component | Role |
|-----------|------|
| [`src/grammar/`](./src/grammar/) | Custom recursive-descent parser for Salt syntax |
| [`src/types.rs`](./src/types.rs) | Type system — generics, traits, inference |
| [`src/codegen/`](./src/codegen/) | MLIR code generation (30+ modules) |
| [`src/codegen/verification/`](./src/codegen/verification/) | Z3 contract verification, arena escape analysis |
| [`src/codegen/passes/`](./src/codegen/passes/) | SSA optimization, loop invariant hoisting |
| [`std/`](./std/) | Standard library (70+ modules, written in Salt) |
| [`tests/`](../tests/) | End-to-end test suite |

## Key Invariants

> [!IMPORTANT]
> **The Alloca Hoisting Law**
> All stack allocations (`llvm.alloca`) are hoisted to function entry.
> This prevents stack overflows in deep recursion and makes stack usage statically predictable for Z3 verification.

### SSA-First Locals
- **Default**: Variables are immutable SSA values (`LocalKind::SSA`)
- **Mutation**: If a variable is reassigned, it is demoted to a stack allocation (`LocalKind::Ptr`) with `llvm.load`/`llvm.store`

### Multi-Dialect Emission
The compiler routes loops to different MLIR dialects based on analysis:
- **Tensor loops** (`A[i,j]` indexing) → `affine.for` (polyhedral tiling, vectorization)
- **Scalar loops** → `scf.for` (register pressure optimization)
- **Linear algebra** → `linalg` ops (hardware-specific lowering)

## Build & Test

**Prerequisites**: Rust 1.75+, Z3 4.12+ (`brew install z3`), LLVM 21+ (`brew install llvm@21`).

```bash
# Build the compiler
cargo build --release

# Run all tests
cargo test

# Compile a Salt program
./target/release/salt-front ../examples/hello_world.salt -o hello
DYLD_LIBRARY_PATH=/opt/homebrew/lib ../hello
```

> [!TIP]
> If the build fails with `ld: library not found for -lz3`, install Z3: `brew install z3`
> If `mlir-opt` is not found, add LLVM to PATH: `export PATH=/opt/homebrew/opt/llvm@21/bin:$PATH`

## Code Stats

- **~30,000 lines** of Rust
- **1,000+ unit tests** (`cargo test`)
- Compiles the full benchmark suite (22 programs) and LETTUCE server
