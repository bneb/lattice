# Tutorial: Zero to Kernel

This tutorial walks through writing, compiling, and running a first Salt kernel — from an empty file to a booting kernel that performs a simple computation and halts.

## 1. Environment Setup

Ensure you have the following installed:
- **Rust 1.75+** — builds the Salt compiler
- **Z3 4.12+** — `brew install z3`
- **LLVM 21+** — `brew install llvm@21` (provides `mlir-opt`, `mlir-translate`, and `clang`)
- **QEMU** — `brew install qemu` (for execution)

Build the compiler:
```bash
./scripts/build.sh
```

## 2. Writing the Kernel

Create a file named `hello.salt`:

```salt
package hello

fn main() -> i32 {
    let mut result = 0;

    for i in 1..11 {
        result = result + i;
    }

    // Result should be 55 (sum of 1 to 10)
    return result;
}
```

## 3. Compiling: The Full Pipeline

Salt compiles through a 4-stage pipeline: `salt-front` → `mlir-opt` → `mlir-translate` → `clang`.

### Stage 1: Salt → MLIR

The Salt compiler frontend emits textual MLIR to stdout:

```bash
./salt-front/target/debug/salt-front hello.salt > /tmp/salt_build/hello.mlir
```

### Stage 2: MLIR Lowering

Use LLVM's `mlir-opt` to lower Salt's multi-dialect MLIR to the LLVM dialect:

```bash
mlir-opt /tmp/salt_build/hello.mlir \
    --allow-unregistered-dialect \
    --convert-scf-to-cf \
    --convert-cf-to-llvm \
    --convert-arith-to-llvm \
    --convert-func-to-llvm \
    --reconcile-unrealized-casts \
    -o /tmp/salt_build/hello.opt.mlir
```

### Stage 3: MLIR → LLVM IR

```bash
mlir-translate --mlir-to-llvmir /tmp/salt_build/hello.opt.mlir -o /tmp/salt_build/hello.ll
```

### Stage 4: Link and Build

```bash
clang -O3 /tmp/salt_build/hello.ll salt-front/runtime.c -o /tmp/salt_build/hello -lm
```

### Running

```bash
/tmp/salt_build/hello
echo "Exit code: $?"
# Expected: Exit code: 55
```

> [!TIP]
> The `scripts/run_test.sh` script automates this entire pipeline. Use it for day-to-day development:
> ```bash
> ./scripts/run_test.sh hello.salt
> ```

## Next Steps

Explore the [**Region Memory Model**](../philosophy/region-model.md) to understand how Salt ensures safety for the KeuOS kernel.
