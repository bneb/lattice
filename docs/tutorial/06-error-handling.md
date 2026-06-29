# Chapter 6: Error Handling, Result, and Pipe

## The Result Type

Salt uses `Result<T>` for operations that can fail. It is an enum with two variants: `Ok(T)` on success and `Err(Status)` on failure. The `Status` type carries an error code and detail payload.

```salt
package main

import std.core.result.Result
import std.status.Status

/// Parse a "key=value" line, returning Ok((key, value)) or Err
fn parse_kv(line: StringView) -> Result<(i64, i64)> {
    let eq = line.find_byte('=' as u8);
    if eq < 0 {
        return Result::Err(Status::with_detail(4, -1));  // INVALID_ARGUMENT
    }
    let key = line.slice(0, eq);
    let val = line.slice(eq + 1, line.length());
    return Result::Ok((key.length(), val.length()));
}

fn main() -> i32 {
    // match on Ok/Err with f-strings for formatted output
    match parse_kv("width=1024") {
        Result::Ok((k, v)) => println(f"ok: key_len={k}, val_len={v}"),
        Result::Err(s) => println(f"error code={s.code}"),
    }

    // let-else: extract or bail
    let Result::Ok((k, v)) = parse_kv("height=768") else {
        println("config parse failed");
        return 1;
    };
    println(f"parsed: {k}, {v}");
    return 0;
}
```

## The `?` Operator

The postfix `?` operator unwraps a `Result`: on `Ok(v)` it yields `v`; on `Err(s)` it returns the error from the enclosing function immediately. This is Salt's equivalent of Rust's `?`.

```salt
fn open_config(path: StringView) -> Result<i64> {
    // Simulated: always succeeds here
    return Result::Ok(4096);
}

fn load_and_validate(path: StringView) -> Result<i64> {
    let size = open_config(path)?;       // propagate error if open fails
    if size < 256 {
        return Result::Err(Status::with_detail(9, size as i32));  // FAILED_PRECONDITION
    }
    return Result::Ok(size);
}
```

## Pipe (`|>`) for Data Flow

The pipe operator feeds a value into a function: `x |> f()` becomes `f(x)`. Use it to build clear left-to-right transformation pipelines when every step is infallible.

```salt
fn trim_len(s: StringView) -> i64 {
    return s.length();
}

fn clamp(max: i64, val: i64) -> i64 {
    if val > max { return max; }
    return val;
}

fn main() -> i32 {
    // Chained: trim_len(clamp(255, ...))
    let capped = "hello world"
        |> trim_len(_)     // 11
        |> clamp(255, _);  // 11
    println(f"capped={capped}");
    return 0;
}
```

## Railway (`|?>`) for Error Propagation

The railway operator chains fallible operations. Each step only runs if the previous one produced `Ok`. On the first `Err`, the chain short-circuits and propagates the error.

```salt
fn validate(val: i64) -> Result<i64> {
    if val < 0 {
        return Result::Err(Status::with_detail(3, val as i32));  // INVALID_ARGUMENT
    }
    return Result::Ok(val);
}

fn double(val: i64) -> Result<i64> {
    return Result::Ok(val * 2);
}

fn main() -> i32 {
    let raw = 42;

    // Chain: validate → double, stop on first Err
    let result = raw
        |?> validate(_)
        |?> double(_);

    match result {
        Result::Ok(v) => println(f"result={v}"),
        Result::Err(s) => println(f"failed code={s.code}"),
    }
    return 0;
}
```

## How It Compares

| Concept | Salt | Rust | Go |
|---------|------|------|----|
| Wrapped result | `Result<T>` | `Result<T, E>` | `(T, err)` |
| Unwrap or return | `expr?` | `expr?` | `if err != nil { return err }` |
| Fallible chain | `x \|?> f() \|?> g()` | `x.and_then(f).and_then(g)` | nested `if err` blocks |
| Infallible chain | `x \|> f() \|> g()` | method chaining | `x = f(x); x = g(x)` |
| Force-unwrap | `expr~` | `expr.unwrap()` | implicit panic via runtime |

Salt's `?` maps directly to Rust's `?` — both return early on error. The railway operator `|?>` replaces Rust's `and_then` chains and Go's repetitive `if err != nil` boilerplate. The plain pipe `|>` covers the common case where every step succeeds, keeping nitpicky `unwrap()` calls out of business logic.

## Summary

| Operator | Behavior |
|----------|----------|
| `expr?` | Extract `Ok(v)` or return `Err(e)` from the enclosing function |
| `x \|> f()` | Pipe: call `f(x)` — infallible transformation |
| `x \|?> f()` | Railway: call `f(x)` if `Ok`, short-circuit on `Err` |
| `expr~` | Force-unwrap: panic on `Err` (use for verified invariants) |
| `let Ok(v) = expr else { ... }` | Extract or execute fallback |

Next: [Chapter 7: FFI, Extern, and Unsafe](07-ffi.md)
