# Chapter 3: Functions

## Function Signatures

Salt functions use explicit return types with `->`:

```salt
package main

fn add(a: i32, b: i32) -> i32 {
    return a + b;
}

fn greet(name: StringView) {
    // No return type = returns unit (no value)
    println(f"Hello, {name}!");
}

fn main() -> i32 {
    let sum = add(3, 4);
    greet("world");
    println(f"sum = {sum}");
    return 0;
}
```

## Visibility

`pub` makes a function visible outside its package. Functions are private by default:

```salt
package math

// Visible to other packages
pub fn multiply(a: i32, b: i32) -> i32 {
    return a * b;
}

// Only visible within this package
fn helper(x: i32) -> i32 {
    return x + 1;
}
```

## Function Pointers

Salt supports first-class function pointers via the `fn(T1, T2) -> R` type:

```salt
package main

fn add(a: u64, b: u64) -> u64 {
    return a + b;
}

fn apply(op: fn(u64, u64) -> u64, x: u64, y: u64) -> u64 {
    return op(x, y);
}

fn main() -> i32 {
    let f: fn(u64, u64) -> u64 = add;
    let result = f(3, 4);           // indirect call through pointer

    let applied = apply(add, 10, 20);

    println(f"result={result}, applied={applied}");
    return 0;
}
```

Use `fn_addr` to get the raw address of a function (for dispatch tables, IDT vectors, etc.):

```salt
fn handler(x: u64) -> u64 {
    return x * 2;
}

fn main() -> i32 {
    let addr: u64 = fn_addr(handler);
    // addr is the raw function pointer
    println(f"handler at: {addr}");
    return 0;
}
```

## Foreign Function Interface (FFI)

### Calling C from Salt

Declare external C functions with `extern fn`. Wrap them in `@trusted` to skip Z3 verification:

```salt
package main

// C standard library functions
extern fn malloc(size: i64) -> Ptr<u8>;
extern fn free(ptr: Ptr<u8>);

use std.core.ptr.Ptr

@trusted  // Skip Z3 verification for FFI wrappers
fn allocate_buffer(size: i64) -> Ptr<u8> {
    return malloc(size);
}
```

### Calling Salt from C

Use `@export` to expose a Salt function with its unmangled name:

```salt
package main

@export
fn salt_compute(a: i32, b: i32) -> i32 {
    return a * a + b * b;
}
// C can now call: int result = salt_compute(3, 4);
```

> **FFI safety**: Only primitive types (`i32`, `f64`), function pointers, and raw pointers (`Ptr<T>`) may cross the FFI boundary. Passing complex types like `String` by value is a compile-time error.

## Attributes

Attributes modify function behavior. They appear on the line before the function:

```salt
package main

// @inline: hint to inline this function
@inline
fn fast_path(x: i32) -> i32 {
    return x + 1;
}

// @pure: modeled as a Z3 uninterpreted function — enables verification
@pure
fn hash(x: i64) -> i64 {
    return x * 2654435761;
}

// @yielding: enables cooperative scheduling with yield checks at loop back-edges
@yielding
fn long_task() {
    let mut i = 0;
    while i < 1000000 {
        i = i + 1;
        // Compiler inserts yield checks automatically
    }
}

// @trusted: skip Z3 verification (for FFI wrappers and verified-external code)
@trusted
fn ffi_wrapper() -> i32 {
    return extern_lib_call();
}

fn main() -> i32 {
    println(f"fast_path(41) = {fast_path(41)}");
    return 0;
}
```

## The Pipe Operator

The pipe operator `|>` enables left-to-right function composition:

```salt
package main

fn square(x: i32) -> i32 { return x * x; }
fn double(x: i32) -> i32 { return x * 2; }
fn add_one(x: i32) -> i32 { return x + 1; }

fn main() -> i32 {
    // These are equivalent:
    let a = add_one(double(square(5)));  // nested calls — read inside-out
    let b = 5 |> square() |> double() |> add_one();  // pipeline — read left-to-right

    // Both produce: 5 → 25 → 50 → 51
    println(f"a={a}, b={b}");
    return 0;
}
```

The placeholder `_` forwards the receiver into any argument position:

```salt
fn transform(data: i32, scale: i32, offset: i32) -> i32 {
    return data * scale + offset;
}

fn main() -> i32 {
    // _ fills in the first argument position
    let result = 10 |> transform(_, 2, 5);
    // result = transform(10, 2, 5) = 25
    println(f"result={result}");
    return 0;
}
```

## Summary

| Feature | Syntax |
|---------|--------|
| Function | `fn name(params) -> ReturnType { ... }` |
| Public | `pub fn ...` |
| Function pointer | `fn(i32, i32) -> i32` |
| FFI import | `extern fn name(params) -> Type;` |
| FFI export | `@export fn name(...) { ... }` |
| Inline hint | `@inline fn ...` |
| Co-op scheduling | `@yielding fn ...` |
| Skip verification | `@trusted fn ...` |
| Pipe | `value \|> fn()` |
| Placeholder | `_` in pipeline chains |

Next: [Chapter 4: Structs & Enums](04-structs-enums.md)
