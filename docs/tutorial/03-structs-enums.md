# Salt by Example -- Chapter 3: Structs, Enums, and Pattern Matching

Structs group named fields into a single type. Enums model a choice between
variants, each of which may carry data. Pattern matching inspects and
destructures both.

## Structs

Define a struct with `struct`. Initialize it with named fields. Access
individual fields with dot notation:

```salt
package main

struct Color {
    r: u8,
    g: u8,
    b: u8,
}

fn main() -> i32 {
    let white = Color { r: 255, g: 255, b: 255 };
    let black = Color { r: 0, g: 0, b: 0 };

    println(f"white = ({white.r}, {white.g}, {white.b})");
    println(f"black = ({black.r}, {black.g}, {black.b})");
    return 0;
}
```

## Enums

An enum declares a type with one or more variants. Variants can carry
typed data:

```salt
package main

enum Shape {
    Circle(f64),          // radius
    Rectangle(f64, f64),  // width, height
}

fn area(s: Shape) -> f64 {
    match s {
        Shape::Circle(r) => return 3.14159 * r * r,
        Shape::Rectangle(w, h) => return w * h,
    }
}

fn main() -> i32 {
    let c = Shape::Circle(5.0);
    let r = Shape::Rectangle(4.0, 6.0);
    println(f"circle area = {area(c)}");
    println(f"rect area = {area(r)}");
    return 0;
}
```

## Pattern Matching

Match arms destructure enum variants and bind their data. The compiler
enforces exhaustiveness -- every variant must be covered:

```salt
package main

struct Point { x: f64, y: f64 }

enum MaybePoint {
    Some(Point),
    None,
}

fn magnitude(mp: MaybePoint) -> f64 {
    match mp {
        MaybePoint::Some(pt) => {
            return pt.x * pt.x + pt.y * pt.y;
        },
        MaybePoint::None => return 0.0,
    }
}

fn main() -> i32 {
    let p = MaybePoint::Some(Point { x: 3.0, y: 4.0 });
    println(f"magnitude = {magnitude(p)}");

    // let-else: bind on match, or bail out
    let q = MaybePoint::Some(Point { x: 1.0, y: 2.0 });
    let MaybePoint::Some(pt) = q else {
        return 0;
    };
    println(f"q = ({pt.x}, {pt.y})");

    // Match guard: extra condition per arm
    let r = MaybePoint::Some(Point { x: -5.0, y: 0.0 });
    match r {
        MaybePoint::Some(pt) if pt.x > 0.0 => {
            println("positive x");
        },
        MaybePoint::Some(_) => println("non-positive x"),
        MaybePoint::None => println("no point"),
    }
    return 0;
}
```

## Summary

| Concept | Syntax |
|---------|--------|
| Struct | `struct Name { field: Type }` |
| Init | `Name { field: value }` |
| Field access | `value.field` |
| Enum | `enum Name { Variant(Type) }` |
| Match | `match expr { Pattern => body }` |
| let-else | `let Pattern = expr else { ... };` |
| Match guard | `Pattern if condition => ...` |

Next: [Chapter 4: Generics](04-generics.md)
