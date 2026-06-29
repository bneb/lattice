# Chapter 4: Generics and Monomorphization

## Generic Functions

Type parameters use C++/Java-style angle brackets. Write `<T>` after the function name to introduce a type variable:

```salt
package main

// T is a type parameter — this works for any type
fn identity<T>(x: T) -> T {
    return x;
}

// Multiple type parameters let you express relationships
fn swap<A, B>(a: A, b: B) -> (B, A) {
    return (b, a);
}

fn main() -> i32 {
    // Turbofish ::<T> — explicit type argument
    let x = identity::<i32>(42);

    // When the compiler can infer the type, omit turbofish
    let y = identity("hello");           // T = StringView
    let z = identity::<f64>(3.14);       // explicit — T = f64

    // Multiple inferred type parameters
    let pair = swap::<i32, bool>(1, true);
    println(f"x={x}, y={y}, z={z}, pair=({pair.0}, {pair.1})");
    return 0;
}
```

Use `::<T>` (turbofish) when the compiler cannot deduce a type parameter. Omit it when inference succeeds.

## Generic Structs

Structs can carry type parameters too:

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

    // Method return types can vary independently
    fn swap(self) -> Pair<B, A> {
        return Pair { first: self.second, second: self.first };
    }
}

fn main() -> i32 {
    // Turbofish on constructor
    let p = Pair::new::<i32, StringView>(1, "one");
    // Method call: self has Pair<i32, StringView>, returns Pair<StringView, i32>
    let q = p.swap();

    println(f"({q.first}, {q.second})");
    return 0;
}
```

Every `<A, B>` used in the `impl` block refers to the same type parameters as the struct — they are scoped to the implementation.

## How Monomorphization Works

Salt uses **monomorphization**: the compiler generates a separate concrete copy of each generic function for every distinct set of type arguments at each call site.

```salt
// Source — one generic function:
fn first<T>(pair: Pair<T, T>) -> T {
    return pair.first;
}

// Compiled output — one copy per used type:
// fn first_i32__i32(pair: Pair<i32, i32>) -> i32 { ... }
// fn first_f64__f64(pair: Pair<f64, f64>) -> f64 { ... }
// (never used with StringView → no copy generated)
```

When the same generic is called with different types, the compiler produces one copy per type:

```salt
package main

fn describe<T>(val: T) -> StringView {
    // Each monomorphization gets its own copy of this function body
    return "a value";
}

fn main() -> i32 {
    // These two calls cause two monomorphized copies of describe:
    let a = describe::<i32>(42);
    let b = describe::<f64>(3.14);
    // describe<StringView> — never called, never compiled

    println(f"i32: {a}, f64: {b}");
    return 0;
}
```

Key properties:

- **Zero runtime cost**: generic code is as fast as hand-written specialized code.
- **Lazy specialization**: only the type combinations actually used are compiled — unused instantiations cost nothing.
- **Per-variant verification**: Z3 checks contracts separately for each monomorphization, catching type-specific edge cases.
- **No type erasure**: unlike Java generics, every concrete type is preserved in the generated code.

## Salt vs. Rust Generics

Both Salt and Rust use monomorphization, but the syntax and philosophy differ:

| Aspect | Salt | Rust |
|--------|------|------|
| Syntax | `fn foo<T>(x: T)` — C++/Java angle brackets | `fn foo<T>(x: T)` — same |
| Turbofish | `foo::<i32>(x)` | `foo::<i32>(x)` |
| Trait bounds | `where T: Concept` (Z3-verified) | `where T: Trait` (type-checked) |
| Const generics | In development | `const N: usize` |

The most visible difference is **bounds**. The same identity function in each language:

```salt
// Salt — no bounds needed at the definition site:
fn identity<T>(x: T) -> T {
    return x;
}
// Verifies at each call site;
// a missing method on T is caught at monomorphization time.
```

```rust
// Rust — must declare bounds up front:
fn identity<T>(x: T) -> T {
    return x;
}
// No bounds needed here either — no method is called on T.
// But calling x.foo() would need `where T: HasFoo`.
```

Salt accepts unconstrained type parameters and verifies at each call site; a missed method call becomes an error at monomorphization time. Rust requires explicit trait bounds at the definition site so the type checker can validate method calls immediately.

## Summary

| Concept | Syntax |
|---------|--------|
| Generic function | `fn name<T>(x: T) -> T` |
| Multiple params | `fn name<A, B>(a: A, b: B) -> B` |
| Turbofish | `func::<i32>(42)` |
| Inferred call | `func(42)` — compiler deduces `T` |
| Generic struct | `struct Pair<A, B> { ... }` |
| impl on generics | `impl Pair<A, B> { fn method(self) { ... } }` |

Next: [Chapter 5: Arenas and Memory](05-arena-memory.md)
