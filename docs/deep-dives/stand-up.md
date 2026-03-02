# Technology Stand-up: The Salt Toolchain

Salt is built on three core technologies that together achieve both performance and verified safety.

## 1. Rust (Frontend Compiler)

The entire compiler frontend is written in Rust (`salt-front`).

- **Purpose**: Parsing, type checking, Z3 verification, and MLIR code generation
- **Parser**: Custom recursive-descent parser using `syn`'s `ParseStream` infrastructure (handles Salt's operator syntax including `|>`, `|?>`, `@`, and `_` placeholder)
- **Output**: Textual MLIR targeting multiple dialects (`affine`, `scf`, `linalg`, `llvm`)

## 2. LLVM MLIR (The Dialect Bridge)

Salt targets MLIR's multi-dialect infrastructure, bypassing traditional C frontends entirely.

- **Purpose**: Progressive lowering and dialect-specific optimization
- **Key advantage**: Different loop types route to different dialects — `affine.for` for tensor code (polyhedral tiling), `scf.for` for scalar loops
- **Design decision**: By targeting MLIR, Salt accesses LLVM's full optimization pipeline plus dialect-specific passes that C cannot benefit from (e.g., `linalg.matmul` → AMX hardware acceleration)

## 3. Z3 (Formal Verification)

The Z3 theorem prover is integrated directly into `salt-front` to verify `requires`/`ensures` contracts at compile time.

- **Objective**: Prove that function contracts hold for all possible inputs
- **Mechanism**: At each call site, arguments are substituted into the callee's contracts and Z3 checks satisfiability
- **Impact**: Enables compile-time bounds check elision, division safety, and arena escape analysis — all with zero runtime cost

## Build System

**Prerequisites**: Rust 1.75+, Z3 4.12+ (`brew install z3`), LLVM 21+ (`brew install llvm@21`).

- **Primary**: `cargo build` for `salt-front`, then shell scripts for MLIR → LLVM → native compilation
- **Testing**: `cargo test` (1,200+ compiler unit tests) plus `bash benchmark_all.sh` for end-to-end benchmarks
- **Package manager**: [`sp`](../../tools/sp/) orchestrates the full pipeline from `salt.toml` manifests

> [!TIP]
> If Z3 is missing, `cargo build` will fail with `ld: library not found for -lz3`. Install with `brew install z3`.
