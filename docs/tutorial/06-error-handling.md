# Chapter 6: Error Handling

## The `Result<T>` Type

Salt uses `Result<T>` for fallible operations. Success wraps a value of type `T`. Failure carries an error code and diagnostic message:

```salt
package main

use std.core.result.Result

fn safe_div(a: i32, b: i32) -> Result<i32> {
    if b == 0 {
        return Result::Err::<i32>(1);  // error code 1 = division by zero
    }
    return Result::Ok::<i32>(a / b);
}

fn main() -> i32 {
    let good = safe_div(10, 2);   // Result::Ok(5)
    let bad = safe_div(10, 0);    // Result::Err(1)

    match good {
        Result::Ok(val) => println(f"result = {val}"),
        Result::Err(e) => println(f"error code: {e}"),
    }
    return 0;
}
```

## The `?` Operator

The postfix `?` operator extracts `Ok(v)` or returns `Err(e)` from the enclosing function:

```salt
package main

use std.core.result.Result

fn parse_int(s: StringView) -> Result<i32> { /* ... */ return Result::Ok::<i32>(0); }
fn validate_range(val: i32) -> Result<i32> { /* ... */ return Result::Ok::<i32>(val); }
fn store(val: i32) -> Result<i32> { /* ... */ return Result::Ok::<i32>(val); }

fn process(input: StringView) -> Result<i32> {
    // If parse_int returns Err, propagate it immediately
    let val = parse_int(input)?;

    // If validate_range returns Err, propagate it immediately
    let checked = validate_range(val)?;

    // Both succeeded — proceed
    return store(checked);
}

fn main() -> i32 {
    let result = process("42");
    match result {
        Result::Ok(v) => println(f"processed: {v}"),
        Result::Err(_) => println("processing failed"),
    }
    return 0;
}
```

## The Railway Operator `|?>`

The `|?>` operator chains fallible operations — it short-circuits on the first `Err`:

```salt
package main

use std.core.result.Result

fn parse(s: StringView) -> Result<StringView> {
    return Result::Ok::<StringView>(s);
}
fn validate(s: StringView) -> Result<StringView> {
    return Result::Ok::<StringView>(s);
}
fn transform(s: StringView) -> Result<StringView> {
    return Result::Ok::<StringView>(s);
}

fn main() -> i32 {
    let raw = "hello";

    // Each step only runs if the previous one succeeded.
    // If any step returns Err, the chain stops and propagates the error.
    let processed = raw
        |?> parse(_)
        |?> validate(_)
        |?> transform(_);

    match processed {
        Result::Ok(v) => println(f"ok: {v}"),
        Result::Err(_) => println("chain failed"),
    }
    return 0;
}
```

The `|?>` pipeline reads left-to-right: "take `raw`, try parsing, try validating, try transforming."

## `|?>` vs `|>` — When to Use Each

```salt
// |> (pipe): use when every step succeeds unconditionally
let result = data |> normalize() |> format() |> output();

// |?> (railway): use when any step can fail
let result = input |?> parse() |?> validate() |?> process();
```

## Force-Unwrap with `~`

The postfix `~` operator unwraps a `Result<T>`, panicking if it's `Err`:

```salt
package main

use std.core.result.Result

fn main() -> i32 {
    let ok_val = Result::Ok::<i32>(42);
    let x = ok_val~;  // x = 42

    // let bad = Result::Err::<i32>(1);
    // let y = bad~;  // PANICS at runtime

    println(f"x = {x}");
    return 0;
}
```

Use `~` for invariants that should never fail (like the Z3 `requires`/`ensures` pattern) — when you know a `Result` must be `Ok`, `~` makes that assertion explicit.

## Combining `?` with Pattern Matching

```salt
package main

use std.core.result.Result

fn main() -> i32 {
    let maybe_value = Result::Ok::<i32>(42);

    // Let-else: handle the error case inline
    let Result::Ok(val) = maybe_value else {
        println("was Err — using default");
        return -1;
    };
    // val is available here

    println(f"val = {val}");
    return 0;
}
```

## Error Handling Flow

```
Input data
    |
    v
parse()       --|?>--> if Err → short-circuit, return Err
    | Ok
    v
validate()    --|?>--> if Err → short-circuit, return Err
    | Ok
    v
process()     --|?>--> if Err → short-circuit, return Err
    | Ok
    v
Success result
```

## Summary

| Operator | Behavior |
|----------|----------|
| `expr?` | Extract `Ok(v)` or return `Err(e)` from the enclosing function |
| `a \|?> f()` | Chain: pass `Ok(v)` to `f`, short-circuit on `Err` |
| `expr~` | Force-unwrap: panic if `Err` (use for provable invariants) |
| `let Ok(v) = expr else { ... }` | Extract or execute fallback block |

Next: [Chapter 7: Arena Memory](07-arena-memory.md)
