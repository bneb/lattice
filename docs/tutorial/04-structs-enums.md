# Chapter 4: Structs & Enums

## Structs

Salt structs are named collections of fields:

```salt
package main

struct Point {
    x: f64,
    y: f64,
}

fn main() -> i32 {
    // Create a struct
    let p = Point { x: 3.0, y: 4.0 };

    // Access fields
    let distance = p.x * p.x + p.y * p.y;

    println(f"distance² = {distance}");
    return 0;
}
```

## Methods via `impl` Blocks

Add methods to a struct with `impl` blocks. Use `&self` for read-only access:

```salt
package main

struct Point {
    x: f64,
    y: f64,
}

impl Point {
    // Constructor (convention: new)
    fn new(x: f64, y: f64) -> Point {
        return Point { x: x, y: y };
    }

    // Read-only method
    fn distance_squared(&self) -> f64 {
        return self.x * self.x + self.y * self.y;
    }

    // Method with parameters
    fn offset(&self, dx: f64, dy: f64) -> Point {
        return Point {
            x: self.x + dx,
            y: self.y + dy,
        };
    }
}

fn main() -> i32 {
    let p = Point::new(3.0, 4.0);
    let d2 = p.distance_squared();     // 25.0
    let moved = p.offset(1.0, 2.0);    // (4.0, 6.0)

    println(f"d²={d2}, moved=({moved.x}, {moved.y})");
    return 0;
}
```

## Enums

Salt enums can carry data (like Rust enums or algebraic data types):

```salt
package main

enum Shape {
    Circle(f64),          // radius
    Rectangle(f64, f64),  // width, height
    Triangle(f64, f64),   // base, height
}

fn area(s: Shape) -> f64 {
    match s {
        Shape::Circle(r) => return 3.14159 * r * r,
        Shape::Rectangle(w, h) => return w * h,
        Shape::Triangle(b, h) => return 0.5 * b * h,
    }
}

fn main() -> i32 {
    let c = Shape::Circle(5.0);
    let r = Shape::Rectangle(4.0, 6.0);

    println(f"circle area = {area(c)}");
    println(f"rectangle area = {area(r)}");
    return 0;
}
```

## Pattern Matching

`match` is exhaustive — the compiler checks that every variant is handled:

```salt
package main

enum Result<T> {
    Ok(T),
    Err(i32),
}

fn describe(r: Result<i32>) -> StringView {
    match r {
        Result::Ok(val) => {
            if val > 0 {
                return "positive success";
            }
            return "non-positive success";
        },
        Result::Err(code) => return "error occurred",
    }
}

fn main() -> i32 {
    let ok = Result::Ok::<i32>(42);
    println(f"{describe(ok)}");
    return 0;
}
```

### Match Guards

Add conditions to match arms with `if`:

```salt
match value {
    Result::Ok(x) if x > 0 => println("positive"),
    Result::Ok(_)           => println("zero or negative"),
    Result::Err(_)          => println("error"),
}
```

### Let-Else

Extract a value or execute a fallback block:

```salt
let Result::Ok(val) = maybe_result else {
    println("operation failed");
    return -1;
};
// val is available here — we know it's Ok
```

### Tuple Destructuring

```salt
let pair = (42, 99);
let (a, b) = pair;              // a=42, b=99

let nested = (10, (20, 30));
let (x, (y, z)) = nested;       // x=10, y=20, z=30
```

### Struct Destructuring in Match

```salt
struct Point3 { x: f64, y: f64, z: f64 }

match point {
    Point3 { x: 0.0, y, z } => println("on the yz-plane"),
    Point3 { x, y, z }      => println(f"({x}, {y}, {z})"),
}
```

## Traits

Salt has built-in traits that types can implement. Use `@derive` for automatic derivation:

```salt
package main

use std.core.clone.Clone
use std.eq.Eq
use std.hash.Hash
use std.ord.Ord

// @derive auto-generates trait implementations from fields
@derive(Clone, Eq, Hash, Ord)
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

fn main() -> i32 {
    let red = Color { r: 255, g: 0, b: 0 };
    let also_red = red.clone();    // Clone::clone

    if red.eq(&also_red) {
        println("colors match via Eq");
    }
    return 0;
}
```

Alternatively, implement traits manually:

```salt
impl Eq for Color {
    fn eq(&self, other: &Color) -> bool {
        return self.r == other.r
            && self.g == other.g
            && self.b == other.b;
    }
}
```

| Trait | Method | Purpose |
|-------|--------|---------|
| `Clone` | `clone(&self) -> Self` | Deep copy |
| `Eq` | `eq(&self, other: &Self) -> bool` | Equality comparison |
| `Hash` | `hash(&self) -> u64` | HashMap key hashing |
| `Ord` | `cmp(&self, other: &Self) -> i32` | Lexicographic ordering (-1, 0, 1) |

## Modules & Imports

Salt uses `package` declarations and `use` imports:

```salt
package mylib

// Import specific types
use std.string.String
use std.collections.HashMap

// Wildcard import
use std.core.ptr.*

// Grouped import
use std.io.file.{File, BufferedReader}
```

The package name becomes the namespace. Public items from `package geometry` are accessed as `geometry::Point`.

## Summary

| Concept | Syntax |
|---------|--------|
| Struct | `struct Name { field: Type }` |
| Constructor | `Name { field: value }` |
| Method | `fn method(&self) -> Type { ... }` |
| Enum | `enum Name { Variant(Type) }` |
| Match | `match expr { Pattern => body, }` |
| Match guard | `Pattern if condition => ...` |
| Let-else | `let Pattern = expr else { ... };` |
| Auto traits | `@derive(Clone, Eq, Hash, Ord)` |
| Import | `use package.module.Type` |

Next: [Chapter 5: Generics](05-generics.md)
