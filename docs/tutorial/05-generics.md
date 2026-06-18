# Chapter 5: Generics

## Generic Functions

Type parameters use C++/Java-style angle brackets:

```salt
package main

// A generic identity function
fn identity<T>(x: T) -> T {
    return x;
}

// Generic with multiple type parameters
fn make_pair<A, B>(a: A, b: B) -> (A, B) {
    return (a, b);
}

fn main() -> i32 {
    let x = identity::<i32>(42);      // explicit type argument
    let y = identity("hello");         // type inferred: StringView

    let pair = make_pair(10, true);   // inferred: (i32, bool)

    println(f"x={x}, pair=({pair.0}, {pair.1})");
    return 0;
}
```

When the compiler can infer type arguments, you can omit them. When it can't, provide them explicitly.

## Generic Structs

Structs can be parameterized by type and allocator:

```salt
package main

struct Pair<A, B> {
    first: A,
    second: B,
}

impl Pair<A, B> {
    fn new(a: A, b: B) -> Pair<A, B> {
        return Pair { first: a, second: b };
    }

    fn swap(self) -> Pair<B, A> {
        return Pair { first: self.second, second: self.first };
    }
}

fn main() -> i32 {
    let p = Pair::new::<i32, bool>(42, true);
    let swapped = p.swap();  // Pair<bool, i32>

    println(f"swapped: ({swapped.first}, {swapped.second})");
    return 0;
}
```

## Generic Collections with Arena Allocation

The standard library's `Vec<T, A>` is parameterized by both element type and allocator:

```salt
package main

use std.collections.Vec
use std.vec.vec

fn main() -> i32 {
    // Vec of i64, with the default allocator
    let mut v = Vec::new::<i64>();

    v.push(10);
    v.push(20);
    v.push(30);

    let second = v.get(1);  // 20

    let len = v.len();      // 3
    println(f"vec[{len}], second={second}");
    return 0;
}
```

## Trait Bounds with Concepts

Salt uses `concept` to constrain type parameters with Z3-verifiable conditions:

```salt
package main

// A numeric concept requiring positive values
concept Numeric(T) requires(T > 0)

fn safe_sqrt<T>(val: T) -> T
    where T: Numeric
{
    // Z3 proves val > 0 at every call site
    // ... computation ...
    return val;
}
```

> **Note:** Concepts are an experimental feature. For most use cases, type parameters without explicit bounds are sufficient — the compiler monomorphizes and verifies at each call site.

## `@derive` with Generics

Auto-derived traits work on generic structs — the derivation expands per field:

```salt
package main

use std.core.clone.Clone
use std.eq.Eq
use std.hash.Hash

@derive(Clone, Eq, Hash)
pub struct Entry<K, V> {
    pub key: K,
    pub value: V,
}
// Expands to impl Clone, Eq, Hash for Entry where K, V implement those traits
```

## Iterator Combinators

Generic iterator methods enable functional-style data processing:

```salt
package main

use std.core.iter.Range

fn is_even(x: i32) -> bool { return x % 2 == 0; }
fn square(x: i32) -> i32 { return x * x; }

fn main() -> i32 {
    // Sum of squares of even numbers from 0..10
    let sum = Range::new(0, 10)
        .filter(is_even)       // 0, 2, 4, 6, 8
        .map(square)           // 0, 4, 16, 36, 64
        .sum();                // 120

    println(f"sum = {sum}");
    return 0;
}
```

Available combinators: `.filter()`, `.map()`, `.sum()`, `.fold()`, `.count()`, `.any()`, `.all()`.

## How Generics Work

Salt uses **monomorphization**: the compiler generates a specialized copy of each generic function for every concrete type it's called with. This means:

- **Zero runtime overhead** — generic code is as fast as hand-specialized code
- **Lazy specialization** — only used instantiations are compiled
- **Compile-time verification** — Z3 verifies contracts separately for each monomorphization

```salt
// You write:
fn first<T>(pair: Pair<T, T>) -> T { return pair.first; }

// The compiler generates for each usage:
// fn first_i32(pair: Pair<i32, i32>) -> i32 { ... }
// fn first_f64(pair: Pair<f64, f64>) -> f64 { ... }
```

## Summary

| Feature | Syntax |
|---------|--------|
| Generic function | `fn name<T>(x: T) -> T` |
| Explicit type arg | `func::<i32>(42)` |
| Inferred type arg | `func(42)` |
| Generic struct | `struct Pair<A, B> { ... }` |
| Concept bound | `concept C(T) requires(condition)` |
| Where clause | `where T: Concept` |
| Auto derive | `@derive(Clone, Eq) struct Foo<T> { ... }` |

Next: [Chapter 6: Error Handling](06-error-handling.md)
