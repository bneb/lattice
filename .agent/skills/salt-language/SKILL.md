---
name: salt-language
description: Salt programming language conventions, syntax rules, and development patterns for the KeuOS project
---

# Salt Language Skill

This skill teaches Antigravity about the Salt programming language — a systems language with formal verification, MLIR codegen, and C10M-scale concurrency.

## Core Design Principles

Salt has **three permanent bets** that apply to ALL code:

1. **`Result<T>` with `Status`** — All fallible operations return `Result<T>`. No exceptions, no panics in normal flow.
2. **Canonical `Status` codes** — Errors use `Status` (i32 code + i32 detail), not strings. Mapped from POSIX errno.
3. **Explicit `return`** — Every function with a return type MUST have an explicit `return` statement. No implicit returns.

## File Conventions

- **Extension**: `.salt`
- **Package declaration**: `package main` (first line)
- **Imports**: `use std.core.result.Result` (dot-separated, NOT `import`)
- **Module path**: Maps to filesystem: `std.fs.fs` → `std/fs/fs.salt`

## Syntax Reference

```salt
package main

use std.core.result.Result
use std.status.Status

// Function with explicit return
fn add(a: i32, b: i32) -> i32 {
    return a + b;
}

// Struct definition
struct Point {
    x: f64,
    y: f64,
}

// Enum with payloads
enum Option<T> {
    Some(T),
    None,
}

// Method implementation
impl Point {
    pub fn distance(&self) -> f64 {
        return (self.x * self.x + self.y * self.y).sqrt();
    }
}

// Formal verification
fn divide(a: i32, b: i32) -> i32
    requires { b > 0 }
    ensures { result >= 0 }
{
    return a / b;
}

// Pattern matching
match opt {
    Option::Some(val) => { println(f"got {val}"); }
    Option::None => { println("nothing"); }
}

// For-range loop
for i in 0..n {
    // i >= 0 && i < n is auto-verified by Z3
}

// F-strings
println(f"value = {x}, name = {name}");
```

## Abolished Patterns (ERRORS)

These will cause compilation failures:
- `import` keyword → Use `use` instead
- `NativePtr` / `NodePtr` types → Use `Ptr<T>` instead
- Identifiers with `__` → Reserved for symbol mangling
- Implicit returns → Always use explicit `return`

## Standard Library Modules

| Module | Purpose |
|--------|---------|
| `std.core.result` | `Result<T>` = `Ok(T) \| Err(Status)` |
| `std.core.option` | `Option<T>` = `Some(T) \| None` |
| `std.core.ptr` | `Ptr<T>` — raw pointer operations |
| `std.core.str` | `StringView` — zero-copy string slicing |
| `std.status` | `Status` — canonical error codes |
| `std.string` | `String` — heap-allocated growable string |
| `std.io.file` | File I/O — read, write, mmap |
| `std.fs.fs` | Filesystem — exists, create_dir, read_dir |
| `std.time` | `Instant`, `Duration`, elapsed |
| `std.collections.vec` | `Vec<T>` — dynamic array |
| `std.collections.hashmap` | `HashMap<K,V>` — Swiss table |
| `std.net.tcp` | TCP networking |
| `std.thread` | Threading |
| `std.sync` | Mutex, Channel |
| `std.json` | JSON parsing |
| `std.fmt` | Formatting |
| `std.nn` | Neural network intrinsics |

## Build & Test Commands

```bash
# Build the Salt compiler (debug)
./scripts/build.sh

# Build and run Rust unit tests
./scripts/build.sh --test

# Run a single Salt test
./scripts/run_test.sh tests/test_example.salt

# Run all Salt tests
./scripts/run_all_tests.sh

# Run benchmarks
./scripts/run_benchmark.sh benchmarks/example_bench.salt
```

## Common Patterns

### Error Handling
```salt
let result = fs::create_dir("/tmp/mydir\0");
if result.is_ok() {
    println("Created!");
} else {
    let status = result.status();
    if status.is_already_exists() {
        println("Already existed");
    }
}
```

### Generic Functions
```salt
fn max<T>(a: T, b: T) -> T {
    if a > b {
        return a;
    }
    return b;
}
```

### Extern FFI
```salt
extern fn malloc(size: i64) -> Ptr<u8>;
extern fn free(ptr: Ptr<u8>);
```

## Key Architecture Notes

- **Compiler**: `salt-front/` (Rust crate using syn for parsing, MLIR for codegen)
- **Runtime**: `salt-front/runtime.c` (C runtime with POSIX syscall wrappers)
- **Pipeline**: Salt → Parse → AST → MLIR → LLVM IR → Native binary
- **Verification**: Z3 theorem prover for `requires`/`ensures` contracts
- **Testing**: Tests in `tests/` directory, benchmarks in `benchmarks/`

## Facet Demo Video Pipeline

The Facet tiger animation is produced headlessly — no window required.

```bash
# Step 1: Build the compiler
./scripts/build.sh

# Step 2: Render 28 PPM frames to /tmp/facet_frames/
cd user/facet && make rec-tiger

# Step 3: Encode to WebM + MP4 (requires ffmpeg)
make video
# → site/video/facet_tiger.webm  (18KB, VP9)
# → site/video/facet_tiger.mp4   (36KB, H.264)
```

Source: `user/facet/raster/rec_tiger.salt`

### PPM Frame-Dumping Pattern

File I/O uses raw libc FFI declared as extern functions:

```salt
extern fn fopen(path: Ptr<u8>, mode: Ptr<u8>) -> Ptr<u8>;
extern fn fwrite(buf: Ptr<u8>, size: i64, count: i64, fp: Ptr<u8>) -> i64;
extern fn fclose(fp: Ptr<u8>) -> i32;
extern fn mkdir(path: Ptr<u8>, mode: i32) -> i32;
extern fn snprintf(buf: Ptr<u8>, size: i64, fmt: Ptr<u8>, n: i32) -> i32;
```

**Salt string/pointer capabilities for C FFI:**
- String literals auto-promote to `!llvm.ptr` when passed to `extern fn` expecting `Ptr<u8>`
- The compiler **auto-null-terminates** all string literals (emits `\00` suffix in MLIR)
- `as Ptr<u8>` cast syntax works: `"hello" as Ptr<u8>`, `malloc(64) as Ptr<u8>`
- `String.as_ptr()`, `Vec.as_ptr()`, `Slice.as_ptr()` all return `Ptr<T>`
- Octal literals supported: `0o755` (also `0x` hex, `0b` binary)
- F-strings: `f"frame_{n}.ppm"` via `InterpolatedStringHandler`
- `snprintf` can be declared as an extern and called normally
- `\0` suffix only needed for `push_cstr()` which scans for a sentinel byte

> **Note:** `rec_tiger.salt` constructs paths byte-by-byte (e.g., `buf.offset(0).write(47 as u8)`)
> because it was written before these idioms were established. Idiomatic Salt would use
> string literals directly for static paths, and `snprintf` or f-strings for dynamic
> filename construction.

PPM format is ideal for frame dumps: `P6\n<w> <h>\n255\n` header + raw RGB bytes.
Strip the alpha channel from RGBA pixels when writing (PPM is RGB only).
