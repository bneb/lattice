# Foreign Function Interface (FFI)

Salt uses the standard **C Application Binary Interface (ABI)** for its functions and primitive types. This makes it trivial to interoperate with C, C++, Rust, and Assembly without any translation layer.

## Calling C from Salt (`extern fn`)

To call a function compiled in another language, declare it using the `extern fn` syntax. 

```salt
// Declare the external C functions
extern fn malloc(size: i64) -> Ptr<u8>;
extern fn free(ptr: Ptr<u8>);
extern fn puts(str: Ptr<u8>) -> i32;

fn main() -> i32 {
    let memory = malloc(1024);
    free(memory);
    return 0;
}
```

### Skipping Z3 Verification (`@trusted`)

The Z3 theorem prover operates on Salt source code. Because `extern fn` calls jump into pre-compiled machine code, Z3 cannot mathematically prove their behavior. 

To prevent Z3 from emitting warnings or failures around external state mutations, use the `@trusted` attribute on wrapper functions:

```salt
@trusted
fn allocate_buffer(size: i64) -> Ptr<u8> {
    // Z3 will blind itself to the memory effects of this block.
    return malloc(size);
}
```

---

## Calling Salt from C (`@export`)

By default, the Salt compiler mangles function names to support namespaces and generics (e.g., `std__string__String__length`). 

To expose a Salt function to C or Rust, use the `@export` attribute. This guarantees the function symbol is unmodified and sets its MLIR linkage to `public`.

**salt_lib.salt:**
```salt
@export
fn compute_hash(data: Ptr<u8>, len: i64) -> i64 {
    // ... hash logic ...
    return 0;
}
```

**main.c:**
```c
#include <stdint.h>
#include <stdio.h>

// Forward declare the Salt function
extern int64_t compute_hash(uint8_t* data, int64_t len);

int main() {
    uint8_t payload[] = {1, 2, 3};
    int64_t hash = compute_hash(payload, 3);
    printf("Hash: %lld\n", hash);
    return 0;
}
```

### Compile & Link
To link them together, compile the Salt code to an object file using the LLVM backend, and link it with your C compiler:

```bash
salt-front salt_lib.salt -o salt_lib.mlir
mlir-translate --mlir-to-llvmir salt_lib.mlir > salt_lib.ll
clang -O3 -c salt_lib.ll -o salt_lib.o

clang main.c salt_lib.o -o my_app
```

---

## Type Mapping Across the Boundary

When designing FFI boundaries, **always use primitives or raw pointers**.

| Salt Type | C Type | Rust Type | Notes |
|-----------|--------|-----------|-------|
| `i8`, `i16`, `i32`, `i64` | `int8_t`, `int16_t`, `int32_t`, `int64_t` | `i8`, `i16`, `i32`, `i64` | Exact ABI match. |
| `u8`, `u16`, `u32`, `u64` | `uint8_t`, `uint16_t`, `uint32_t`, `uint64_t` | `u8`, `u16`, `u32`, `u64` | Exact ABI match. |
| `f32`, `f64` | `float`, `double` | `f32`, `f64` | Exact ABI match. |
| `bool` | `bool` (`<stdbool.h>`) | `bool` | ABI matches C boolean representation. |
| `Ptr<T>` | `T*` | `*mut T` | Raw memory pointer. |
| `fn(i32) -> i32` | `int32_t (*)(int32_t)` | `extern "C" fn(i32) -> i32` | First-class function pointer. |

### Enforced Safety: Complex Types

The Salt compiler **strictly blocks** passing complex native types (like `String` or `Vec<T>`) across the FFI boundary by value. Attempting to use them in an `@export` or `extern fn` signature will result in a compile-time error.

```salt
// COMPILE ERROR: `String` is not FFI-safe.
@export
fn process_string(s: String) {}

// GOOD: Primitives and pointers are FFI-safe.
@export
fn process_bytes(ptr: Ptr<u8>, len: i64) {}
```
