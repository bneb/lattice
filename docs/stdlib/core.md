# Core Types & Primitives

## `std.core.option.Option<T>`

Salt's null-free optional type. Replaces null pointers.

```salt
use std.core.option.Option

enum Option<T> {
    Some(T),
    None,
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `is_some` | `(&self) -> bool` | True if the Option is `Some` |
| `is_none` | `(&self) -> bool` | True if the Option is `None` |
| `unwrap` | `(&self) -> T` | Extracts the value (panics if None) |

**Usage:**
```salt
let maybe = Option::Some::<i32>(42);
match maybe {
    Option::Some(v) => println(f"got {v}"),
    Option::None => println("nothing"),
}
let Option::Some(val) = maybe else { return; };
```

## `std.core.result.Result<T>`

Fallible computation result. Success carries a value; failure carries an error code.

```salt
use std.core.result.Result

enum Result<T> {
    Ok(T),
    Err(Status),   // canonical error status
}
```

| Operator/Method | Description |
|-----------------|-------------|
| `result?` | Extract `Ok(v)` or return `Err(e)` from enclosing function |
| `result~` | Force-unwrap: extract value or panic |
| `a \|?> f()` | Railway: chain fallible operations |

**Usage:**
```salt
fn parse(s: StringView) -> Result<i32> {
    // ... parsing logic ...
    return Result::Ok::<i32>(42);
}

fn process(input: StringView) -> Result<i32> {
    let val = parse(input)?;          // propagate on Err
    return Result::Ok::<i32>(val * 2);
}
```

## `std.core.str.StringView`

Non-owning string slice. Zero-copy borrow of existing bytes. String literals are `StringView` by default.

```salt
use std.core.str.StringView
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `length` | `() -> i64` | Number of bytes |
| `byte_at` | `(i64) -> u8` | Byte value at index |
| `as_ptr` | `() -> Ptr<u8>` | Raw pointer to underlying bytes |

**Usage:**
```salt
let s = "hello world";          // StringView literal
let len = s.length();           // 11
let b = s.byte_at(0);          // 104 ('h')

fn greet(name: StringView) {   // Preferred parameter type
    println(f"Hello, {name}!");
}
```

## `std.string.String`

Owning, heap-allocated string. Supports mutation and f-string interpolation.

```salt
use std.string.String
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `with_capacity` | `(i64) -> String` | Create with pre-allocated capacity |
| `as_view` | `(&self) -> StringView` | Zero-cost borrow as StringView |
| `from_view` | `(&StringView) -> String` | Allocate + copy from StringView |
| `length` | `() -> i64` | Number of bytes |

**Naming convention:** `as_*` = zero-cost/borrowing, `from_*` = allocating copy.

**Usage:**
```salt
let mut s = String::with_capacity(128);
s.f"Hello, {name}!";  // Writer protocol — stream to buffer
let view = s.as_view();  // zero-copy borrow
```

## `std.core.ptr.Ptr<T>`

Typed pointer with provenance tracking. Salt's low-level memory primitive.

```salt
use std.core.ptr.Ptr
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `null` | `() -> Ptr<T>` | Null pointer constant |
| `is_null` | `() -> bool` | Null check |
| `read` | `() -> T` | Dereference read |
| `write` | `(T) -> ()` | Dereference write |
| `offset` | `(i64) -> Ptr<T>` | Pointer arithmetic |

## `std.core.arena.Arena`

Bump-allocated memory region. O(1) allocation, O(1) bulk free.

```salt
use std.core.arena.Arena
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(i64) -> Arena` | Create arena with capacity in bytes |
| `mark` | `() -> i64` | Snapshot current position |
| `alloc` | `(T) -> Ptr<T>` | Bump-allocate a value |
| `reset_to` | `(i64) -> ()` | O(1) bulk free to mark |

## `std.core.clone.Clone`

Deep-copy trait. Auto-derived with `@derive(Clone)`.

```salt
use std.core.clone.Clone

impl Clone for MyType {
    fn clone(&self) -> MyType { /* field-wise copy */ }
}
```

## `std.eq.Eq`

Equality comparison trait. Auto-derived with `@derive(Eq)`.

```salt
use std.eq.Eq

impl Eq for MyType {
    fn eq(&self, other: &MyType) -> bool { /* field-wise equality */ }
}
```

## `std.ord.Ord`

Lexicographic ordering trait. Returns -1 (less), 0 (equal), 1 (greater).

```salt
use std.ord.Ord

impl Ord for MyType {
    fn cmp(&self, other: &MyType) -> i32 { /* ... */ }
}
```

## `std.hash.Hash`

Hash trait for HashMap keys. Uses WyHash internally. Auto-derived with `@derive(Hash)`.

```salt
use std.hash.Hash

impl Hash for MyType {
    fn hash(&self) -> u64 { /* combine field hashes */ }
}
```

## `std.core.iter`

Iterator combinators for functional-style data processing.

| Combinator | Description |
|-----------|-------------|
| `.filter(pred)` | Keep elements where predicate is true |
| `.map(f)` | Transform each element |
| `.sum()` | Sum all elements |
| `.fold(init, f)` | Left fold with initial value |
| `.count()` | Count elements |
| `.any(pred)` | True if any element matches |
| `.all(pred)` | True if all elements match |

```salt
use std.core.iter.Range

let evens = Range::new(0, 100)
    .filter(|x| x % 2 == 0)
    .map(|x| x * x)
    .sum();
```

## `std.core.buffer`

Growable byte buffer for I/O and serialization.

```salt
use std.core.buffer
```

## `std.core.mem`

Memory operations: size_of, alignment queries, zeroed allocation.

```salt
use std.core.mem
```

## `std.core.conv`

Type conversion helpers.

```salt
use std.core.conv
```
