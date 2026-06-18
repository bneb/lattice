# Chapter 2: Types & Values

## Numeric Types

Salt distinguishes signed (`i`) and unsigned (`u`) integers at multiple widths. Each has a well-defined size:

```salt
package main

fn main() -> i32 {
    let a: i8 = -128;        // 8-bit signed, range: -128..127
    let b: u8 = 255;         // 8-bit unsigned, range: 0..255
    let c: i32 = -2_000_000; // underscores for readability
    let d: u64 = 18_446_744_073_709_551_615;  // max u64
    let pi: f64 = 3.141592653589793;

    println(f"a={a}, b={b}, c={c}, pi={pi}");
    return 0;
}
```

## Characters

Salt characters compile to `i8` (ASCII code point):

```salt
package main

fn main() -> i32 {
    let a: i8 = 'A';      // 65
    let nl: i8 = '\n';    // 10  (newline)
    let nul: i8 = '\0';   // 0   (null byte)

    println(f"A = {a}, newline = {nl}");
    return 0;
}
```

## Arrays

Fixed-size arrays use `[T; N]` syntax. Array access uses subscript notation:

```salt
package main

fn main() -> i32 {
    let arr: [i32; 4] = [10, 20, 30, 40];

    let first = arr[0];   // 10
    let last = arr[3];    // 40

    // Array length is a compile-time constant
    let len = 4;  // use the declared size

    // Iteration
    let mut sum = 0;
    for i in 0..4 {
        sum = sum + arr[i];
    }

    println(f"sum = {sum}");
    return 0;
}
```

## Tuples

Tuples group values of different types:

```salt
package main

fn main() -> i32 {
    let pair = (42, true);
    let (num, flag) = pair;     // destructuring: num=42, flag=true

    let triple: (i32, f64, bool) = (10, 3.14, false);

    // Nested destructuring
    let nested = (1, (2, 3));
    let (a, (b, c)) = nested;   // a=1, b=2, c=3

    println(f"pair: {num}, {flag}");
    return 0;
}
```

## String and StringView

Salt has two string types:

- **`String`** — owns its memory (heap-allocated). You allocate and free it.
- **`StringView`** — borrows existing bytes (zero-copy). String literals are `StringView`.

```salt
package main

use std.string.String
use std.core.str.StringView

fn main() -> i32 {
    // String literals are StringView by default
    let greeting = "hello";          // StringView, no allocation
    let len = greeting.length();     // 5

    // Create an owned String
    let mut s = String::with_capacity(16);
    // ... populate s ...

    // Convert String → StringView (zero-copy)
    let view = s.as_view();

    // Convert StringView → String (allocates + copies)
    let owned = String::from_view(&view);

    println(f"greeting length: {len}");
    return 0;
}
```

**Rule of thumb**: Use `StringView` for function parameters (borrow, no allocation). Use `String` when you need to own the data.

## References

References borrow data without taking ownership. Use `&T` for immutable references and `&mut T` for mutable ones:

```salt
package main

fn increment(val: &mut i32) {
    // dereference and modify through a mutable reference
    *val = *val + 1;
}

fn main() -> i32 {
    let mut x = 41;
    increment(&mut x);  // pass a mutable reference
    println(f"x = {x}");   // x = 42
    return 0;
}
```

## Pointers

`Ptr<T>` is Salt's typed pointer type for low-level memory access:

```salt
package main

use std.core.ptr.Ptr

fn main() -> i32 {
    let x: i32 = 42;
    let p: Ptr<i32> = &x as Ptr<i32>;  // reference to pointer
    // Raw pointer operations available via std.core.ptr
    return 0;
}
```

## Hex Literals

For binary data:

```salt
package main

fn main() -> i32 {
    let magic = hex"DEADBEEF";
    // hex literals are useful for magic numbers, protocol constants, etc.
    return 0;
}
```

## Type Casting

Use the `as` keyword for numeric conversions:

```salt
package main

fn main() -> i32 {
    let x: i32 = 42;
    let y: i64 = x as i64;     // widen
    let z: u32 = x as u32;     // sign conversion
    let f: f64 = x as f64;     // int to float

    println(f"y={y}, z={z}, f={f}");
    return 0;
}
```

## Summary

| Type | Syntax | Example |
|------|--------|---------|
| Signed int | `i8`, `i16`, `i32`, `i64` | `let x: i32 = -42;` |
| Unsigned int | `u8`, `u16`, `u32`, `u64` | `let y: u64 = 1000;` |
| Float | `f32`, `f64` | `let pi = 3.14;` |
| Boolean | `bool` | `let ok = true;` |
| Character | `char` (stored as `i8`) | `let c = 'A';` |
| Array | `[T; N]` | `let arr: [i32; 3] = [1, 2, 3];` |
| Tuple | `(T1, T2)` | `let pair = (42, true);` |
| Owned string | `String` | `String::with_capacity(16)` |
| String slice | `StringView` | `"hello"` (literals) |
| Reference | `&T`, `&mut T` | `&x`, `&mut x` |
| Raw pointer | `Ptr<T>` | `&x as Ptr<i32>` |

Next: [Chapter 3: Functions](03-functions.md)
