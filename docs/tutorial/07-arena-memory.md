# Chapter 7: Arena Memory

## Why Arenas?

Salt manages memory through **arena-based allocation** instead of manual `malloc`/`free` or garbage collection. An arena is a fixed-size memory region. You allocate objects from it, and when you're done, you free the entire region in O(1) time — no per-object deallocation, no fragmentation, no GC pauses.

```salt
package main

use std.arena.Arena

fn main() -> i32 {
    // Allocate a 4KB arena
    let mut arena = Arena::new(4096);

    // Save the current position
    let mark = arena.mark();

    // Allocate from the arena (bump pointer — very fast)
    let x = arena.alloc::<i64>(42);
    let y = arena.alloc::<i64>(99);

    let sum = *x + *y;  // 141

    // Free everything allocated since mark in O(1)
    arena.reset_to(mark);

    println(f"sum = {sum}");
    return 0;
}
```

## The Scope Ladder

Salt's **Scope Ladder** performs compile-time escape analysis. Every variable has a **depth** based on its lexical scope:

| Depth | Meaning | Example |
|-------|---------|---------|
| 0 | Global / static | Module-level constants |
| 1 | Function arguments | `fn process(arena: Arena)` |
| 2 | Local variables | `let arena = Arena::new(4096)` |
| 3+ | Nested blocks | Arena inside `if`/`while`/`for` |

Arena pointers **inherit the depth of the arena they were allocated from**.

### The Three Laws

**Law I (Return Rule):** `return x` is valid only if `depth(x) ≤ 1`. A local arena's pointers cannot escape the function:

```salt
use std.arena.Arena
use std.core.ptr.Ptr

struct Node { val: i64 }

fn create_safe(arena: Arena) -> Ptr<Node> {
    // arena: depth 1 (argument)
    let n = arena.alloc::<Node>(Node { val: 42 });
    // n: depth 1 (inherits from arena)
    return n;  // ✓ depth 1 ≤ 1 — safe
}

fn create_dangling() -> Ptr<Node> {
    // This will be REJECTED at compile time:
    let local_arena = Arena::new(4096);  // depth 2
    let n = local_arena.alloc::<Node>(Node { val: 1 });  // depth 2
    // return n;  // ✗ REJECTED: depth 2 > 1 — would dangle!
    return Ptr::<Node>::null();
}
```

**Law II (Assignment Rule):** `a = b` is valid only if `depth(b) ≤ depth(a)`. Cannot store a short-lived pointer into a long-lived container:

```salt
struct Context {
    saved_ptr: Ptr<i64>,
}

fn bad_store(ctx: &mut Context) {
    let local = Arena::new(256);     // depth 2
    let data = local.alloc::<i64>(99);  // depth 2
    // ctx.saved_ptr = data;  // ✗ REJECTED: depth 2 > depth 1 (ctx)
}
```

**Law III (Transitivity Rule):** `s.field` inherits `depth(s)`. Struct fields carry the depth of their parent struct.

### What the Scope Ladder Catches

```salt
// ✗ Return escape — local arena pointer returned
fn bad_return() -> Ptr<i64> {
    let arena = Arena::new(256);
    return arena.alloc::<i64>(42);  // REJECTED
}

// ✗ Store escape — short-lived pointer stored in long-lived field
fn bad_store(global: &mut Context) {
    let arena = Arena::new(256);
    global.saved_ptr = arena.alloc::<i64>(1);  // REJECTED
}

// ✓ Output parameter pattern — arena passed in from above
fn good_output(arena: Arena, value: i64) -> Ptr<i64> {
    return arena.alloc::<i64>(value);  // OK: arena depth=1, ptr depth=1
}
```

## The Arena Pattern

The safe idiom for arena usage:

```salt
use std.arena.Arena

struct Request { /* ... */ }
struct Response { /* ... */ }

fn parse_headers(arena: Arena, raw: StringView) -> Ptr<Request> {
    return arena.alloc::<Request>(Request { /* ... */ });
}

fn build_response(arena: Arena, req: Ptr<Request>) -> Ptr<Response> {
    return arena.alloc::<Response>(Response { /* ... */ });
}

fn process_request(raw_request: StringView) -> Response {
    // Create a request-scoped arena
    let arena = Arena::new(4096);
    let mark = arena.mark();

    // Allocate and process
    let parsed = parse_headers(&arena, raw_request);
    let response = build_response(&arena, parsed);

    // Bulk-free everything in O(1)
    arena.reset_to(mark);

    return *response;  // Copy the result out before the arena dies
}
```

## The Safety Stack

Salt provides three layered safety mechanisms:

| Layer | What It Catches | When | Cost |
|-------|----------------|------|------|
| **Escape Analysis** (Scope Ladder) | Dangling returns, cross-lifetime stores | Compile-time | Zero |
| **Poison Fills** (`SALT_DEBUG`) | Use-after-reset, stale reads | Debug runtime | Debug-only |
| **Z3 Verification** (ArenaVerifier) | Use-after-reset, epoch violations | Compile-time | Zero |

## `unsafe` Blocks

For operations the verifier can't prove, use `unsafe`:

```salt
fn low_level_op(ptr: Ptr<u8>) {
    unsafe {
        // Raw pointer operations
        let raw: Ptr<u8> = ptr;
    }
}
```

> **Rule**: Minimize `unsafe` blocks. Every `unsafe` operation should have a `requires` contract explaining why it's safe (see Chapter 8).

## Move Semantics

Use `move` for explicit ownership transfer:

```salt
let data = compute_data();
let transferred = move data;  // data is no longer valid here
```

## Summary

| Feature | Syntax | Purpose |
|---------|--------|---------|
| Create arena | `Arena::new(size)` | Allocate a memory region |
| Save position | `arena.mark()` | Snapshot the bump pointer |
| Allocate | `arena.alloc::<T>(value)` | Bump-allocate from arena |
| Bulk free | `arena.reset_to(mark)` | O(1) free everything after mark |
| Raw operations | `unsafe { ... }` | Bypass verifier (justify in a comment) |
| Transfer ownership | `move value` | Explicit ownership transfer |

Next: [Chapter 8: Z3 Contracts](08-contracts.md)
