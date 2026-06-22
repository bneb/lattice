# Chapter 1: Variables, Types, and Printing

Every Salt program starts with `package main` and a `fn main() -> i32` entry point. Returning `0` signals success.

## Hello, Salt

```salt
package main

fn main() -> i32 {
    let name = "Salt";
    let year = 2026;

    println(f"Hello from {name}, v{year}!");
    return 0;
}
```

`println` prints a line of text. It is imported from `std.io`, but the compiler includes it automatically for convenience.

The `f"..."` syntax is a **format string** (or f-string). Expressions inside `{curly braces}` are evaluated and inserted into the string at that position. Here `{name}` becomes `Salt` and `{year}` becomes `2026`.

## Variables and Types

Variables are **immutable** by default -- once bound, their value cannot change. Use `let` to declare a variable:

```salt
package main

fn main() -> i32 {
    let x: i64 = 42;
    let pi: f64 = 3.14159;
    let ready: bool = true;

    println(f"x = {x}, pi = {pi}, ready = {ready}");
    return 0;
}
```

The type annotation (`: i64`, `: f64`, `: bool`) is optional when the type is unambiguous. Salt infers the type from the value:

```salt
let x = 42;   // i64
let pi = 3.14; // f64
let ok = true; // bool
```

The primitive types covered so far:

| Type     | Description              | Example              |
|----------|--------------------------|----------------------|
| `i64`    | Signed 64-bit integer    | `let x: i64 = -5;`   |
| `f64`    | 64-bit floating point    | `let pi: f64 = 3.14;`|
| `bool`   | Boolean                  | `let ok: bool = true;`|

## Mutability

When a value needs to change, declare the variable with `let mut`:

```salt
package main

fn main() -> i32 {
    let mut counter: i64 = 0;
    counter = counter + 10;

    let mut label: StringView = "start";
    label = "done";

    println(f"counter = {counter}, label = {label}");
    return 0;
}
```

`StringView` is Salt's string type -- a zero-copy view into a string literal. Assigning a new string literal to a mutable `StringView` replaces the view.

## Narrow Integers and `u8`

For small unsigned values, `u8` holds 0--255:

```salt
package main

fn main() -> i32 {
    let byte: u8 = 200;
    let max: u8 = 255;

    let mut steps: u8 = 0;
    steps = steps + 1;

    println(f"byte = {byte}, max = {max}, steps = {steps}");
    return 0;
}
```

## Type Inference in Practice

Salt infers types for local variables, so annotations are only needed when you want to be explicit or when inference would pick a different type than intended:

```salt
package main

fn main() -> i32 {
    let a = 42;           // inferred i64
    let b = 3.14;         // inferred f64
    let c = true;         // inferred bool
    let d: u8 = 10;       // explicit -- without annotation this would be i64
    let mut e = "world";  // inferred StringView

    e = "salt";

    println(f"a = {a}, b = {b}, c = {c}, d = {d}, e = {e}");
    return 0;
}
```

## Comments

Salt uses `//` for line comments -- everything from `//` to the end of the line is ignored:

```salt
// This is a comment
let x = 42;  // inline comment
```

There is no block comment (`/* */`) syntax.

## Rust Users' Quick Reference

Salt's syntax is close to Rust's, with a few differences:

| Salt                          | Rust                           |
|-------------------------------|--------------------------------|
| `package main`                | (implicit crate root)          |
| `println(...)`                | `println!(...)`                |
| `f"text {expr}"`              | `format!("text {expr}")`       |
| `StringView`                  | `&str`                         |
| `i64` / `f64` / `bool` / `u8`| same types, same names         |
| `let` / `let mut`             | same syntax                    |
| `// comments`                 | same syntax                    |
| `;` statement terminators     | same syntax                    |

The biggest visible difference is `println` as a plain function (no `!` macro marker) and native `f"..."` string interpolation.

Next: [Chapter 2: Functions, Contracts, and Verification](02-functions.md)
