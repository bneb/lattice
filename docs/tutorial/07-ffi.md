# Chapter 7: FFI, Extern, and Unsafe

## The Boundary

Salt compiles to native code, but it does not exist in a vacuum. Real programs talk to the operating system, to hardware registers, to C libraries, and to code written in other languages. Salt's Foreign Function Interface (FFI) lets you cross those boundaries — and `unsafe` blocks mark exactly where you do.

## Declaring Extern Functions

Use `extern fn` to declare a function implemented in C (or any language that exposes a C ABI). The declaration has no body — it is a promise to the linker:

```salt
package main

import std.core.ptr.Ptr

// C standard library functions, declared for Salt
extern fn malloc(size: u64) -> Ptr<u8>;
extern fn free(ptr: Ptr<u8>);

fn main() -> i32 {
    unsafe {
        let buf = malloc(64);
        free(buf);
        println("allocated and freed 64 bytes via C malloc/free");
    }
    return 0;
}
```

The `unsafe` block is required because the compiler cannot prove what `malloc` and `free` do — they might return null, write past the buffer, or have side effects invisible to Salt's Z3 verifier. `unsafe` tells the compiler: "I accept responsibility for this."

> **FFI-safe types**: Only primitive integers/float/bool, function pointers (`fn(...) -> R`), and raw pointers (`Ptr<T>`) may cross the FFI boundary. Passing `String` or `Vec<T>` by value to C is a compile-time error.

## Raw Pointer Operations

Salt's `Ptr<T>` gives you controlled access to raw memory. Inside an `unsafe` block you can read and write through a pointer:

```salt
package main

import std.core.ptr.Ptr

fn main() -> i32 {
    let mut val = 42i64;

    // Convert a reference to a raw pointer (intrinsic: ptrtoint)
    let ptr: Ptr<i64> = Ptr::from_ref(&val);

    unsafe {
        // Read through the pointer
        println(f"before: {ptr.read()}");

        // Write through the pointer
        ptr.write(100);

        println(f"after:  {ptr.read()}");
    }

    // The original variable is updated — same memory
    println(f"val:   {val}");

    // Pointer arithmetic: advance by N elements
    unsafe {
        let shifted = ptr.offset(1);
        // shifted points sizeof(i64) bytes past val.
        // Dereferencing it would be a buffer overrun.
        println(f"shifted addr: {shifted.addr()}");
    }
    return 0;
}
```

Key pointer methods:

| Method | What it does |
|--------|-------------|
| `Ptr::from_addr(u64)` | Cast an integer address to a pointer |
| `ptr.addr() -> u64` | Cast a pointer back to an integer |
| `ptr.read() -> T` | Load a value from the pointer's address |
| `ptr.write(T)` | Store a value at the pointer's address |
| `ptr.offset(n) -> Ptr<T>` | Advance pointer by `n` elements (GEP) |
| `ptr.index(i) -> T` | Read the `i`-th element (GEP + load) |
| `ptr.is_null() -> bool` | Check if pointer is address 0 |

## Calling Salt from C — `@no_mangle`

By default Salt **mangles** function names — `math::square` becomes `_ZN4math6squareE...` or similar. To expose a function to C with its original name, use `@no_mangle`:

```salt
package math

/// Called from C as: int result = math_square(7);
@no_mangle
pub fn math_square(x: i32) -> i32 {
    return x * x;
}
```

The function must be `pub` (visible outside its package) and only use FFI-safe types. A C program can now link against your Salt library and call `math_square` by name:

```c
// C caller
int math_square(int x);
int main() {
    int result = math_square(7);   // 49
}
```

Use `@export` for LLVM's `emit_c_interface` attribute — identical effect for linking, slightly different MLIR lowering. In practice `@export` is preferred for public library entry points and `@no_mangle` for internal symbol visibility.

## Memory-Mapped I/O

A classic kernel use case: hardware registers mapped into the address space. Salt's `Ptr::from_addr` combined with `unsafe` lets you read and write device memory directly:

```salt
package kernel.uart

import std.core.ptr.Ptr

// x86 COM1: 0x3F8 (standard PC serial port)
const UART_BASE: u64 = 0x3F8;

/// Write a single byte to the serial port.
fn uart_putb(b: u8) {
    unsafe {
        let reg: Ptr<u8> = Ptr::from_addr(UART_BASE);
        reg.write(b);
    }
}

/// Write a string to the serial port.
fn uart_puts(msg: StringView) {
    let mut i: i64 = 0;
    while i < msg.length() {
        uart_putb(msg.byte_at(i));
        i += 1;
    }
    uart_putb(b'\n');
}
```

The Z3 verifier cannot prove that address `0x3F8` contains a valid mapped register — that guarantee comes from the kernel's memory map, not from logic. `unsafe` marks the trust boundary.

## The `@trusted` Bridge

For functions that wrap FFI calls, annotate the wrapper `@trusted` instead of making every caller write `unsafe`:

```salt
package main

import std.core.ptr.Ptr

extern fn rand() -> i32;

/// Safe wrapper around C's rand().
/// Callers see a normal provable signature.
@trusted
fn random() -> i32 {
    return rand();
}

fn main() -> i32 {
    let r = random();
    println(f"random value: {r}");
    return 0;
}
```

`@trusted` pushes the safety obligation to the wrapper author. All callers see a normal, provable function signature — no `unsafe` block needed. This is the recommended pattern: contain `unsafe` in a thin `@trusted` layer at the boundary.

## Why Unsafe Exists

Not everything is provable — and that is okay. Hardware registers live at magic addresses. Foreign functions can do anything. Booting a kernel means writing to page tables and control registers. These operations are **not wrong** — they are simply **outside what a static verifier can model**.

Salt's philosophy:

1. **Default to safe** — normal Salt code stays within the verifier's reach.
2. **Isolate the unsafety** — `unsafe` blocks are explicit, visible, and grep-able.
3. **Wrap, don't leak** — expose unsafe internals through `@trusted` safe APIs.
4. **Audit the seams** — every `unsafe` block is a review target.

The goal is not zero `unsafe` — it is zero **unchecked** `unsafe`. Every boundary crossing should be deliberate, documented, and reviewed.

## Summary

| Concept | Syntax |
|---------|--------|
| Extern declaration | `extern fn name(...) -> T;` |
| Unsafe block | `unsafe { ... }` |
| Preserve symbol name | `@no_mangle fn ...` |
| C-callable entry point | `@export fn ...` |
| Safe FFI wrapper | `@trusted fn ... { unsafe { ... } }` |
| Raw pointer from address | `Ptr::from_addr(u64)` |
| Pointer read/write | `ptr.read()`, `ptr.write(val)` |
| Pointer arithmetic | `ptr.offset(n)` |
| Cast pointer to int | `ptr.addr() -> u64` |
| Null check | `ptr.is_null()` |
| FFI-safe types | `i32`, `u64`, `f64`, `bool`, `fn(...)`, `Ptr<T>` |

Next: [Chapter 8: Async, Yield, and State Machines](08-async.md)
