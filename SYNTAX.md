# Salt — Syntax Reference

Salt looks like Rust but compiles through MLIR. The key difference: `requires` and `ensures` clauses are proved by Z3 at compile time.

---

## Basics

```salt
package main

fn main() -> i32 {
    let x: i32 = 42;
    let mut counter = 0;      // type inferred as i32
    counter += 1;
    println("hello world");
    return 0;
}
```

`let` for immutable, `let mut` for mutable. Type annotations are optional when the compiler can infer them. `//` for comments (single-line only).

---

## Types

| Type | What it is |
|------|------------|
| `i8`, `i16`, `i32`, `i64` | Signed integers |
| `u8`, `u16`, `u32`, `u64` | Unsigned integers |
| `f32`, `f64` | Floating point |
| `bool` | Boolean |
| `char` | Character (emitted as `i8`) |
| `Ptr<T>` | Typed pointer with provenance tracking |
| `&T`, `&mut T` | References |
| `[T; N]` | Fixed-size arrays |
| `(T, U)` | Tuples |
| `fn(T1, T2) -> R` | Function pointer (first-class type) |
| `String` | Heap-owning string (`{data, len, cap}`) |
| `StringView` | Non-owning string slice (`{ptr, len}`) |

### Strings

Two string types. `String` owns its memory. `StringView` borrows existing bytes without copying.

```salt
use std.string.String
use std.core.str.StringView

let s = String::with_capacity(16);
let view = s.as_view();               // zero-copy borrow

let owned = String::from_view(&view);  // allocates + copies

// String literals are StringView by default:
let sv = "hello";                     // StringView
let len = sv.length();                // 5
let byte = sv.byte_at(0);            // 72 ('H')
```

Convention: `as_*` is zero-cost/borrowing. `from_*` allocates.

### Characters

```salt
let a: i8 = 'A';       // 65
let nl: i8 = '\n';     // 10
```

Character literals become `i8` constants.

---

## Functions

```salt
fn add(a: i32, b: i32) -> i32 {
    return a + b;
}

pub fn public_function(x: i64) -> i64 {
    return x * 2;
}

// Generics use monomorphization
fn identity<T>(x: T) -> T {
    return x;
}
```

### Function pointers

```salt
let f: fn(u64, u64) -> u64 = add;
let result = f(3, 4);           // indirect call

// Get the raw address (for IDT vectors, dispatch tables):
let addr: u64 = fn_addr(add);

// Function pointers in structs:
struct Handler {
    on_read: fn(u64) -> u64,
    on_write: fn(u64, u64) -> u64,
}
```

### FFI

Salt uses the C ABI. `extern fn` declares a C function. `@export` stops name mangling so C code can call Salt.

```salt
extern fn malloc(size: i64) -> Ptr<u8>;
extern fn free(ptr: Ptr<u8>);

@trusted  // skip Z3 verification for FFI wrappers
fn allocate_buffer(size: i64) -> Ptr<u8> {
    return malloc(size);
}
```

```salt
@export
fn salt_compute(a: i32, b: i32) -> i32 {
    return a + b;
}
```

Only primitives, function pointers, and `Ptr<T>` can cross the FFI boundary. Passing a `String` by value is a compile error.

### Attributes

```salt
@inline     fn fast_path(x: i32) -> i32 { return x + 1; }
@pure       fn hash(x: i64) -> i64 { return x * 2654435761; }
@trusted    fn ffi_wrapper() -> i32 { return libc_call(); }

@yielding              // cooperative scheduling at loop back-edges
fn long_task() { ... }

@yielding(4096)        // custom heartbeat (iterations between yields)
fn compute_loop() { ... }

@pulse(60)             // 60Hz tick rate for interactive tasks
fn game_loop() { ... }

@derive(Clone, Hash, Eq, Ord)  // auto-generate trait impls from fields
pub struct Point {
    pub x: i64,
    pub y: i64
}
```

---

## Structs and methods

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

Four built-in traits: `Clone`, `Eq`, `Hash`, `Ord`. Implement them manually, or use `@derive` to generate field-wise implementations.

```salt
@derive(Clone, Eq, Hash, Ord)
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8
}
```

| Trait | Method |
|-------|--------|
| `Clone` | `clone(&self) -> Self` |
| `Eq` | `eq(&self, other: &Self) -> bool` |
| `Hash` | `hash(&self) -> u64` |
| `Ord` | `cmp(&self, other: &Self) -> i32` |

---

## Enums and pattern matching

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

Match guards:

```salt
match value {
    Result::Ok(x) if x > 0 => { println("positive"); },
    Result::Ok(_) => { println("zero or negative"); },
    Result::Err(_) => { println("error"); },
}
```

Let-else for early return on `Option`/`Result`:

```salt
let Some(val) = maybe_value else {
    println("was None");
    return -1;
};
```

Tuple destructuring:

```salt
let (a, b) = pair;
let (x, (y, z)) = nested;
```

---

## Control flow

```salt
if x > 0 {
    println("positive");
} else if x == 0 {
    println("zero");
} else {
    println("negative");
}

while count < 10 {
    count += 1;
}

for i in 0..10 {
    sum += i;
}

loop {
    if done { break; }
    continue;
}
```

---

## Verification — `requires` and `ensures`

This is what makes Salt different. You write contracts on functions. Z3 proves them at compile time.

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

When Z3 finds a violation, you get the counterexample:

```
VERIFICATION ERROR: could not prove '(< 15 10)'
  context: precondition check
  counterexample:
    x = 15
  hint: add 'requires(x < 10)' to the function signature
```

Three outcomes: Z3 proves it (check elided, zero cost), Z3 finds a counterexample (compile error with values), or Z3 times out (runtime assertion emitted, program still compiles). The timeout is 100ms per obligation. Most contracts resolve in under 10ms.

### What Z3 can prove today

Integer bounds and comparisons work reliably. String length comparisons work when the lengths are compile-time constants — the compiler folds `.length()` to an integer before Z3 sees it.

```salt
fn merge(a: StringView, b: StringView) -> i32
    requires(a.length() >= b.length())
{ return a.length() as i32; }

merge("hello world", "hi");   // ✅ 11 >= 2 — proved

let x = "hello";              // length 5, tracked through let-binding
let y = "hi";                 // length 2
merge(x, y);                  // ✅ 5 >= 2 — proved

merge("hi", "hello world");   // ❌ 2 >= 11 — contract violation caught
```

`.contains()`, `.starts_with()`, and `.ends_with()` fold to booleans with literal strings. String methods on runtime values (from I/O, network) fall back to runtime checks — Z3's string theory is incomplete.

Fixed-size arrays carry their length in the type (`[u8; 200]` is always 200 bytes). The compiler could use this to prove `requires(a.length() < 300)` for `a: [u8; 200]` without runtime cost. This is not yet implemented — the Z3 bridge doesn't query array lengths from the type system.

### Postconditions (`ensures`)

Postconditions are verified at every return site:

```salt
fn generate_cookie(input: u32) -> u32
    ensures(result != 0)
{
    let cookie = input ^ 0xDEADBEEF;
    if cookie == 0 { return 1; }  // guard required by Z3
    return cookie;
}
```

If the guard were omitted, Z3 would find `input = 0xDEADBEEF` producing `cookie = 0` and reject the compilation. This is the contract that forced the defensive guard in `kernel/net/tcp_syn_cookie.salt`.

**Concepts** are type constraints with verification backing (experimental):

```salt
concept Numeric(T) requires(T > 0)
```

---

## Syntactic sugar

### Pipe `|>`

```salt
let result = 5 |> square() |> double() |> add_one();
// Same as: add_one(double(square(5)))
```

### Railway `|?>`

Short-circuits on `Err`:

```salt
let result = input |?> parse() |?> validate() |?> transform();
```

### Matmul `@`

Uses `linalg.matmul`. Enables AMX acceleration on Apple Silicon:

```salt
let output = weights @ input;
```

### Placeholder `_`

The previous result in a method chain goes wherever you put `_`:

```salt
(w1 @ input).add_bias(_, HIDDEN, b1).relu(_, HIDDEN)
```

### F-strings

```salt
let msg = f"Hello from {name} in {year}!";

// Targeted f-strings stream directly to a writer:
buffer.f"Status: {code} - {message}\n"
```

### Force-unwrap `~`

```salt
let val = maybe_result~;    // panics if Err/None
```

### Hex literals

```salt
let magic = hex"DEADBEEF";
```

---

## Modules and imports

```salt
package mylib

use std.string.String
use std.core.ptr.*
use std.io.file.{File, BufferedReader}
```

Dot-separated paths only. No `::` style.

---

## Unsafe and memory regions

```salt
unsafe {
    let raw: Ptr<u8> = malloc(1024);
}

with region arena {
    // everything allocated here is freed at end of scope
}
```

`move value;` for explicit ownership transfer.

---

## Iterators

```salt
use std.core.iter.Range

let evens = Range::new(0, 100)
    .filter(is_even)
    .map(square)
    .sum();
```

Combinators: `.filter()`, `.map()`, `.sum()`, `.fold()`, `.count()`, `.any()`, `.all()`.

---

## Threading and synchronization

```salt
use std.thread.Thread
use std.sync.{Mutex, AtomicI64}

let handle = Thread::spawn(worker);
handle.join();

let counter = AtomicI64::new(0);
counter.fetch_add(1);
let val = counter.load();

let m = Mutex::new();
m.lock();
// ... critical section ...
m.unlock();
m.destroy();
```

### Cooperative concurrency

`@yielding` functions get yield checks injected at loop back-edges. `@pulse(N)` sets the tick rate in Hz.

```salt
@yielding
fn worker() {
    // compiler injects yield checks
}

@pulse(1000)  // 1kHz
fn high_frequency_task() { ... }
```

Yield checks use register-pinned deadlines with sub-cycle overhead.

### Channels

```salt
use std.channel.channel.{Channel, UnboundedChannel}

let ch = Channel::bounded(4);
ch.send(42);                  // blocks if full
let val = ch.try_recv();       // Option::Some(42) or Option::None

let uch = UnboundedChannel::new();
uch.send(1);
uch.send(2);
let v = uch.try_recv();        // FIFO
```

---

## Process execution

```salt
use std.process.Command

let status = Command::new("/bin/echo")
    .arg1("hello")
    .execute();
```

---

## HTTP client

```salt
use std.http.client

let fd = client::connect("127.0.0.1", 8080);
client::send(fd, request_bytes, request_len);
let n = client::recv(fd, response_buf, buf_size);
client::close(fd);

// Or just:
let n = client::get_raw("127.0.0.1", 8080, "/health", buf, 4096);
```

---

## JSON

```salt
use std.json.json.{JsonParser, JsonWriter, JsonArray, JsonObject}

let mut p = JsonParser::new("42" as Ptr<u8>, 2);
let val = p.parse_value();

let mut arr = JsonArray::new();
p.parse_array(&mut arr);

let mut obj = JsonObject::new();
p.parse_object(&mut obj);

let mut w = JsonWriter::new(buf, 4096);
w.write_object_start();
w.write_key("x" as Ptr<u8>, 1);
w.write_i64(42);
w.write_object_end();
// Result: {"x":42}
```

---

## Compiler flags

```bash
saltc my_program.salt                        # full verification
saltc my_program.salt --danger-no-verify     # skip Z3 (debug builds)
saltc my_program.salt --release --binary     # optimized native binary
```

---

## Why things are the way they are

A few design choices that come up often:

- **`requires`/`ensures` instead of runtime assertions.** The whole point. Checks that Z3 proves disappear from the binary entirely.

- **`Ptr<T>` instead of `*T`.** Typed pointers carry provenance information. The compiler tracks what they point to.

- **`char` is `i8`.** Systems code doesn't need Unicode complexity. If you're parsing UTF-8, use `StringView` and `byte_at()`.

- **`loop` instead of `while true`.** Cleaner control flow. Maps directly to `cf.br` in MLIR. The intent is clearer.

- **`_` placeholder in method chains.** Lets you write fluent pipelines without closures or lambda syntax. The previous result can fill any argument slot, not just the first one.

- **`@` matmul.** Machine learning workloads shouldn't need function calls for the hottest operation in the program. Compiles to `linalg.matmul`, which LLVM can map to AMX instructions on Apple Silicon.

- **MLIR backend.** Lets us use dialect-specific optimizations — `linalg` for tiling, `affine` for polyhedral transforms, `scf` for structured control flow. Writing our own codegen would mean reinventing all of this.

- **Explicit `return`.** Simpler to analyze. Expression-statement tail returns work too, but `return` is the canonical style.

- **`Result<T>` instead of `Result<T, E>`.** The error type is always `Status` — a gRPC-style canonical code plus a diagnostic string. One less generic parameter to thread through every function signature.

- **`@derive`.** Source-level expansion. No compiler magic. Read the generated code if you want to.

- **Unbounded channels use heap-backed doubling rings.** Senders never block. The cost is an occasional reallocation. For bounded channels (fixed ring buffer), senders block when full.
