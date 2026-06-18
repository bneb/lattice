# ADR 014: Iron Driver Compilation Pipeline

**Status:** Accepted
**Date:** 2025-11 (retroactively documented 2026-06)
**Deciders:** Salt compiler design

## Context

Salt emits MLIR using standard dialects. To produce a native binary, this MLIR must be lowered through multiple tools: `mlir-opt` (dialect lowering), `mlir-translate` (MLIR→LLVM IR), `llc` (LLVM IR→object code), and a linker. Manually running these steps is error-prone and makes the compiler fragile. A single driver that orchestrates the full pipeline is needed.

## Decision

**The Iron Driver (`driver.rs`): a 4-step pipeline that takes MLIR text as input and produces a native binary.**

1. **`mlir-opt`**: Lowers from high-level dialects to LLVM dialect via a fixed sequence of passes: `--canonicalize --cse --lower-affine --convert-scf-to-cf --convert-vector-to-llvm --convert-cf-to-llvm --convert-arith-to-llvm --convert-math-to-llvm --convert-func-to-llvm --finalize-memref-to-llvm --reconcile-unrealized-casts`
2. **`mlir-translate --mlir-to-llvmir`**: Converts LLVM dialect MLIR to LLVM IR text
3. **`llc`**: Compiles LLVM IR to target-specific object code (`-O3`, with LSE atomics for Apple Silicon, x19 reservation for KeuOS kernel)
4. **Linker** (`clang` or `ld.lld`): Links object files with `keuos_rt.o` (runtime) in freestanding mode (no libc)

Target selection (DarwinArm64, LinuxArm64, KeuOSArm64, KeuOSX86_64) controls `llc` flags and linker invocation.

## Consequences

- **Positive**: Fully automated — the user runs `salt-front source.salt -o binary` and gets a native executable
- **Positive**: Target multiplexing enables cross-compilation for KeuOS from macOS
- **Negative**: Pipeline dependency on 4 external tools — each must be discoverable on `PATH`
- **Negative**: Pass sequence is fixed — no per-function or per-module pass customization
- **Negative**: The LLVM 21 toolchain is the only supported version; API changes in MLIR/LLVM could break the pipeline
