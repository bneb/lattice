# Salt Language — Syntax Reference

> A systems language with formal verification, designed for performance without compromise.

Salt compiles to MLIR → LLVM IR → native binary. It uses a Rust-like syntax with verification-first semantics.

---

## Basics

```salt
package main

fn main() -> i32 {
    let x: i32 = 42;
    let mut counter = 0;      // Type inferred as i32
    counter += 1;
    println("hello world");
    return 0;
}
```

- `let` for immutable bindings, `let mut` for mutable
- Type inference for locals — annotations optional when unambiguous
- `//` single-line comments (only style supported)

---

## Types

| Type | Description |
|------|-------------|
| `i8`, `i16`, `i32`, `i64` | Signed integers |
| `u8`, `u16`, `u32`, `u64` | Unsigned integers |
| `f32`, `f64` | Floating point |
| `bool` | Boolean |
| `char` | Character (emitted as `i8`) |
| `Ptr<T>` | Typed pointer with provenance |
| `&T`, `&mut T` | References |
| `[T; N]` | Fixed-size arrays |
| `(T, U)` | Tuples |
| `fn(T1, T2) -> R` | Function pointer type (first-class) |
| `String` | Heap-owning string (`{data, len, cap}`) |
| `StringView` | Non-owning string slice (`{ptr, len}`) |

### String Types

`String` owns its memory (heap-allocated). `StringView` borrows existing bytes (zero-copy).

```salt
use std.string.String
use std.core.str.StringView

// Owned → Borrowed (zero-copy, no allocation)
let s = String::with_capacity(16);
let view = s.as_view();               // StringView { ptr, len }

// Borrowed → Owned (allocates + copies)
let owned = String::from_view(&view);  // new String with copied bytes

// String literals are StringView by default (no cast needed)
let sv = "hello";                     // StringView { ptr: ..., len: 5 }
let len = sv.length();                // 5
let byte = sv.byte_at(0);            // 72 ('H')

// Explicit cast still works for backward compat
let sv2 = "hello" as StringView;      // Also valid (explicit)
```

**Naming convention**: `as_*` = zero-cost/borrowing, `from_*` = allocating copy.


### Character Literals

```salt
let a: i8 = 'A';       // 65
let nl: i8 = '\n';     // 10
let nul: i8 = '\0';    // 0
```

Character literals compile to `i8` constants representing the Unicode scalar value.

---

## Functions

```salt
fn add(a: i32, b: i32) -> i32 {
    return a + b;
}

pub fn public_function(x: i64) -> i64 {
    return x * 2;
}

// Generic functions
fn identity<T>(x: T) -> T {
    return x;
}
```

### Function Pointers

```salt
// First-class function pointer types
let f: fn(u64, u64) -> u64 = add;
let result = f(3, 4);           // Indirect call through function pointer

// Get raw address of a function
let addr: u64 = fn_addr(add);   // For IDT vectors, dispatch tables

// Function pointers in struct fields (SIP dispatch tables)
struct Handler {
    on_read: fn(u64) -> u64,
    on_write: fn(u64, u64) -> u64,
}
```

### Extern Functions

```salt
extern fn malloc(size: i64) -> Ptr<u8>;
extern fn free(ptr: Ptr<u8>);
```

### Attributes

```salt
@inline
fn fast_path(x: i32) -> i32 { return x + 1; }

@pure                    // Modeled as Z3 uninterpreted function
fn hash(x: i64) -> i64 { return x * 2654435761; }

@yielding                // Enables cooperative scheduling
fn long_task() { /* ... */ }

@yielding(4096)          // Custom heartbeat pulse (iterations between yields)
fn compute_loop() { /* ... */ }

@pulse(60)               // 60Hz tick rate for interactive tasks
fn game_loop() { /* ... */ }

@trusted                 // Skip Z3 verification (FFI wrappers)
fn ffi_wrapper() -> i32 { return libc_call(); }

@derive(Clone, Hash, Eq, Ord)  // Auto-generate trait impls from fields
pub struct Point {
    pub x: i64,
    pub y: i64
}
// Expands to: impl Clone, Hash, Eq, Ord for Point (field-wise)
```

---

## Structs & Methods

```salt
struct Point {
    x: f32,
    y: f32,
}

impl Point {
    fn new(x: f32, y: f32) -> Point {
        return Point { x: x, y: y };
    }

    fn distance_squared(&self) -> f32 {
        return self.x * self.x + self.y * self.y;
    }
}

let p = Point::new(3.0f32, 4.0f32);
let d2 = p.distance_squared();     // 25.0
```

### Traits

Salt provides built-in traits that types can implement:

```salt
use std.core.clone.Clone
use std.eq.Eq
use std.hash.Hash
use std.ord.Ord

// Manual implementation:
impl Clone for Color {
    fn clone(&self) -> Color {
        return Color { r: self.r, g: self.g, b: self.b };
    }
}

impl Eq for Color {
    fn eq(&self, other: &Color) -> bool {
        return self.r == other.r && self.g == other.g && self.b == other.b;
    }
}

impl Hash for Color {
    fn hash(&self) -> u64 {
        let mut h: u64 = self.r as u64;
        h = h ^ ((self.g as u64) << 16) ^ ((self.g as u64) >> 48);
        return h;
    }
}

impl Ord for Color {
    fn cmp(&self, other: &Color) -> i32 {
        let c = self.r.cmp(&other.r);
        if c != 0 { return c; }
        return self.g.cmp(&other.g);
    }
}

// Or use @derive to auto-generate all of the above:
@derive(Clone, Eq, Hash, Ord)
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8
}
```

| Trait | Method | Description |
|-------|--------|-------------|
| `Clone` | `clone(&self) -> Self` | Produce a copy of the value |
| `Eq` | `eq(&self, other: &Self) -> bool` | Field-wise equality |
| `Hash` | `hash(&self) -> u64` | WyHash-based hashing for HashMap keys |
| `Ord` | `cmp(&self, other: &Self) -> i32` | Lexicographic ordering (-1, 0, 1) |

---

## Enums & Pattern Matching

```salt
enum Shape {
    Circle(f32),
    Rect(f32, f32),
}

fn area(s: Shape) -> f32 {
    match s {
        Shape::Circle(r) => return 3.14159f32 * r * r,
        Shape::Rect(w, h) => return w * h,
    }
}
```

### Match Guards

```salt
match value {
    Result::Ok(x) if x > 0 => { println("positive"); },
    Result::Ok(_) => { println("zero or negative"); },
    Result::Err(_) => { println("error"); },
}
```

### Let-Else

```salt
let Some(val) = maybe_value else {
    println("was None");
    return -1;
};
```

### Tuple Destructuring

```salt
let pair = (42, 99);
let (a, b) = pair;              // a = 42, b = 99

let triple = (1, 2, 3);
let (x, y, z) = triple;         // x = 1, y = 2, z = 3

let nested = (10, (20, 30));
let (a, (b, c)) = nested;       // a = 10, b = 20, c = 30
```

---

## Control Flow

```salt
// If-else
if x > 0 {
    println("positive");
} else if x == 0 {
    println("zero");
} else {
    println("negative");
}

// While loop
while count < 10 {
    count += 1;
}

// For loop (range-based)
for i in 0..10 {
    sum += i;
}

// Infinite loop (with break/continue)
loop {
    if done {
        break;
    }
    continue;
}
```

---

## Formal Verification — `requires` / `ensures`

Salt integrates the **Z3 theorem prover** directly into the compiler. Preconditions are **proven at compile time** — not checked at runtime.

```salt
fn safe_div(a: i32, b: i32) -> i32
    requires(b != 0)
{
    return a / b;
}

fn bounded_access(arr: &[i32; 10], idx: i32) -> i32
    requires(idx >= 0 && idx < 10)
{
    return arr[idx as i64];
}
```

When verification fails, the compiler produces **actionable diagnostics** with counterexample values:

```
VERIFICATION ERROR: could not prove '(< 15 10)'
  context: precondition check
  counterexample:
    x = 15
  = hint: add 'requires (< 15 10)' to the function signature
```

### Concepts (Type Constraints)

```salt
concept Numeric(T) requires(T > 0)

// Like Rust traits but with formal verification backing
```

### Invariants

```salt
invariant x > 0;   // Statement-level assertion for verification
```

---

## Syntactic Sugar

### Pipe Operator `|>`

Left-to-right function composition:

```salt
let result = 5 |> square() |> double() |> add_one();
// Equivalent to: add_one(double(square(5)))
// 5 → 25 → 50 → 51
```

### Placeholder Forwarding `_`

Forward the receiver value into any argument position in a method chain:

```salt
// In method chains, _ represents the result of the previous expression
(w1 @ input).add_bias(_, HIDDEN, b1).relu(_, HIDDEN)

// _ can appear in any argument position, not just first
result.transform(x, _, y)    // receiver inserted as second arg
```

This enables fluent pipelines where the data flows through a chain of transformations.

### Matmul Operator `@`

Matrix multiplication using `linalg.matmul` (enables AMX on Apple Silicon):

```salt
let output = weights @ input;    // Compiles to linalg.matmul
```

### `?` Operator (Early Return)

Postfix operator for `Result<T>` — extracts `Ok(v)` or returns `Err(e)` from the enclosing function:

```salt
fn process(input: Result<i64>) -> Result<i64> {
    let val = input?;           // Extracts Ok value, or early-returns Err
    let doubled = transform(val)?;
    return Result::Ok(doubled);
}
```

### Railway Operator `|?>`

Error-propagating pipeline (like `?` but composable in chains):

```salt
let result = input |?> parse() |?> validate() |?> transform();
// Short-circuits on first Err
```

### F-Strings

String interpolation:

```salt
let name = "Salt";
let year = 2026;
let msg = f"Hello from {name} in {year}!";
```

### Targeted F-Strings (Writer Protocol)

Stream formatted output to a writer:

```salt
buffer.f"Status: {code} - {message}\n"
// Streams directly to buffer without intermediate allocation
```

### Force-Unwrap `~`

Postfix unwrap operator:

```salt
let val = maybe_result~;    // Panics if Err/None
```

### Hex Literals

```salt
let magic = hex"DEADBEEF";
```

---

## Modules & Imports

```salt
package mylib

// Dot-separated imports (the only style supported)
use std.string.String
use std.core.ptr.*
use std.io.file.{File, BufferedReader}
```

---

## Unsafe & Memory Regions

```salt
unsafe {
    let raw: Ptr<u8> = malloc(1024);
    // Raw pointer operations
}

with region arena {
    // Scoped memory region — allocations freed at end of scope
}
```

### Move Semantics

```salt
move value;              // Explicit ownership transfer
```

---

## Iterator Combinators

```salt
use std.core.iter.Range

let evens = Range::new(0, 100)
    .filter(is_even)
    .map(square)
    .sum();
```

Available combinators: `.filter()`, `.map()`, `.sum()`, `.fold()`, `.count()`, `.any()`, `.all()`

---

## Threading & Synchronization

```salt
use std.thread.Thread
use std.sync.{Mutex, AtomicI64}

fn worker() {
    println("running on thread");
}

fn main() -> i32 {
    // Spawn a thread
    let handle = Thread::spawn(worker);   // Auto Fn→i64 coercion
    handle.join();

    // Atomic operations
    let counter = AtomicI64::new(0);
    counter.fetch_add(1);                    // Atomic increment
    let val = counter.load();                // Atomic load

    // Mutex
    let m = Mutex::new();
    m.lock();
    // ... critical section ...
    m.unlock();
    m.destroy();

    return 0;
}
```

### Cooperative Concurrency

```salt
@yielding
fn worker() {
    // Compiler injects yield checks at loop back-edges
}

@pulse(1000)             // 1kHz tick rate
fn high_frequency_task() { /* ... */ }
```

Salt uses **register-pinned deadlines** and fixed-point call-graph propagation for C10M-scale concurrency with sub-cycle yield check overhead.

### Channels

```salt
use std.channel.channel.{Channel, UnboundedChannel}

// Bounded channel (fixed-capacity ring buffer)
let ch = Channel::bounded(4);
ch.send(42);                        // Blocks if full
let val = ch.try_recv();             // Option::Some(42) or Option::None

// Unbounded channel (heap-backed, doubles on overflow)
let uch = UnboundedChannel::new();
uch.send(1);
uch.send(2);
uch.send(3);
let v = uch.try_recv();              // Option::Some(1) — FIFO order
```

---

## Process Execution

```salt
use std.process.Command

let status = Command::new("/bin/echo")
    .arg1("hello")
    .execute();
// status = exit code (0 = success)
```

---

## HTTP Client

```salt
use std.http.client

// Low-level: connect, send, receive
let fd = client::connect("127.0.0.1", 8080);
client::send(fd, request_bytes, request_len);
let n = client::recv(fd, response_buf, buf_size);
client::close(fd);

// High-level: GET request
let n = client::get_raw("127.0.0.1", 8080, "/health", buf, 4096);
```

---

## JSON

```salt
use std.json.json.{JsonParser, JsonWriter, JsonArray, JsonObject}
use std.json.json.{JSON_NUMBER, JSON_BOOL, JSON_STRING, JSON_NULL}

// Parsing primitives
let mut p = JsonParser::new("42" as Ptr<u8>, 2);
let val = p.parse_value();           // JsonValue { type_tag: JSON_NUMBER, num_val: 42.0 }

// Parsing arrays
let mut p2 = JsonParser::new("[1, true, null]" as Ptr<u8>, 15);
let mut arr = JsonArray::new();
p2.parse_array(&mut arr);            // arr.len == 3
let first_type = arr.type_tags[0];   // JSON_NUMBER
let first_val = arr.num_vals[0];     // 1.0

// Parsing objects
let mut p3 = JsonParser::new("{\"name\":\"salt\"}" as Ptr<u8>, 15);
let mut obj = JsonObject::new();
p3.parse_object(&mut obj);           // obj.len == 1

// Writing JSON
let mut w = JsonWriter::new(buf, 4096);
w.write_object_start();              // {
w.write_key("x" as Ptr<u8>, 1);     // "x":
w.write_i64(42);                     // 42
w.write_object_end();                // }
// Result: {"x":42}
```

---

## Compiler Flags

```bash
# Fast iteration — skip Z3 verification
salt-front --no-verify my_program.salt

# Full verification (default)
salt-front my_program.salt
```

---

## Design Decisions

| Decision | Rationale |
|----------|-----------|
| `requires`/`ensures` | Compile-time, not runtime — zero overhead |
| `Ptr<T>` not `*T` | Typed pointers with provenance tracking |
| `char` → `i8` | Simple, no Unicode complexity for systems code |
| `loop` keyword | Cleaner than `while true`, direct `cf.br` codegen |
| `_` placeholder | Enables fluent method chains without closures |
| `@` matmul | Domain-specific syntax for ML — compiles to AMX |
| MLIR backend | Enables dialect-specific optimizations (linalg, scf, affine) |
| Explicit return | Simpler control flow analysis |
| Result<T>, Not Result<T, E> | Status uses canonical gRPC codes + diagnostic messages |
| `?` operator | Ergonomic error propagation with early return |
| `fn(T) -> R` types | First-class function pointers: `fn(u64, u64) -> u64`, indirect call, `fn_addr()` |
| `@derive` | Source-level expansion — zero magic, inspectable output |
| Unbounded channels | Heap-backed doubling ring buffer — send never blocks |

