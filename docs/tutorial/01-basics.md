# Chapter 1: Hello, Salt

## Your First Program

Every Salt program starts with a `package` declaration and a `fn main()` entry point:

```salt
package main

fn main() -> i32 {
    println("Hello, Salt!");
    return 0;
}
```

`fn main() -> i32` declares a function that returns a 32-bit signed integer. Returning `0` means success (convention shared with C).

## Variables and Mutability

Variables are immutable by default. Use `let mut` when you need to change a value:

```salt
package main

fn main() -> i32 {
    let x: i32 = 42;        // immutable — type annotation
    let y = 10;             // immutable — type inferred as i32
    let mut counter = 0;    // mutable — can be reassigned

    counter = counter + 1;  // ✓ allowed: counter is mut
    // x = 43;              // ERROR: x is not mutable

    println(f"x = {x}, counter = {counter}");
    return 0;
}
```

## Basic Types

Salt's primitive types are explicit and unsurprising:

| Type | Description | Example |
|------|-------------|---------|
| `i8`, `i16`, `i32`, `i64` | Signed integers | `let x: i32 = -42;` |
| `u8`, `u16`, `u32`, `u64` | Unsigned integers | `let y: u64 = 1000;` |
| `f32`, `f64` | Floating point | `let pi: f64 = 3.14159;` |
| `bool` | Boolean | `let ok: bool = true;` |
| `char` | Character (stored as `i8`) | `let c: char = 'A';` |

Type inference works for locals — annotations are optional when the type is unambiguous:

```salt
package main

fn main() -> i32 {
    let x = 42;        // i32
    let y = 3.14;      // f64
    let ok = true;     // bool

    let sum = x + y as i32;  // explicit cast with 'as'
    println(f"sum = {sum}, ok = {ok}");
    return 0;
}
```

## Control Flow

Salt has standard `if`/`else`, `while`, `for`, and `loop`:

```salt
package main

fn main() -> i32 {
    // If-else
    let x = 10;
    if x > 0 {
        println("positive");
    } else if x == 0 {
        println("zero");
    } else {
        println("negative");
    }

    // While loop
    let mut count = 0;
    while count < 3 {
        count = count + 1;
    }

    // For loop over a range
    let mut sum = 0;
    for i in 0..5 {
        sum = sum + i;     // sum = 0+1+2+3+4 = 10
    }

    // Infinite loop with break
    let mut found = false;
    loop {
        if found {
            break;
        }
        found = true;
    }

    println(f"sum = {sum}");
    return 0;
}
```

## F-Strings

String interpolation uses `f"..."` syntax with `{expression}` placeholders:

```salt
package main

fn main() -> i32 {
    let name = "Salt";
    let year = 2026;

    println(f"Hello from {name} in {year}!");
    // Output: Hello from Salt in 2026!

    return 0;
}
```

## Comments

Only `//` line comments are supported:

```salt
// This is a comment
let x = 1;  // inline comment
```

## Summary

| Concept | Syntax |
|---------|--------|
| Entry point | `package main` + `fn main() -> i32` |
| Immutable binding | `let x = value;` |
| Mutable binding | `let mut x = value;` |
| Type annotation | `let x: i32 = 42;` |
| String interpolation | `f"text {expr}"` |
| Comments | `// comment` |

Next: [Chapter 2: Types & Values](02-types.md)
