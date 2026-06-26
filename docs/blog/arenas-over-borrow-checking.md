# Why We Chose Arenas Over Borrow Checking

**Published:** June 2026 | **Author:** The KeuOS Team | **Reading time:** 14 minutes

---

Salt has no lifetime annotations. No `'a`, no `Box<dyn Future>`, no `Arc<Mutex<T>>`. Despite this, it catches use-after-free, dangling pointers, and cross-lifetime stores at compile time — without a borrow checker.

How? Arenas.

---

## The Memory Trilemma

Systems programming has three memory strategies, and each has a problem:

| Strategy | Example | Fast? | Safe? | Annotation burden? |
|----------|---------|-------|-------|--------------------|
| Manual (`malloc`/`free`) | C | Yes | No | None (but you pay in bugs) |
| Garbage collection | Go, Java | No (pause) | Yes | None (but you pay in latency) |
| Borrow checking | Rust | Yes | Yes | Yes (lifetime annotations everywhere) |

Rust solved the trilemma for most cases. But its solution requires explicit lifetime annotations on every reference, `Arc<Mutex<T>>` for shared mutable state, and `unsafe` blocks for FFI and self-referential structs. The cognitive overhead is real — and for systems code where allocation patterns are simple and predictable, it's more mechanism than the problem requires.

Arenas offer a fourth option. One that doesn't try to solve the general case.

---

## How Arenas Work

An arena is a fixed-size memory region with a bump pointer. Allocation increments the pointer. Freeing resets it. Everything in the arena lives for the arena's entire lifetime, then dies together.

```salt
let arena = Arena::new(4096);   // 4KB region
let x = arena.alloc(42);       // bump pointer moves forward
let y = arena.alloc(99);       // bump pointer moves again
// ... use x and y ...
arena.reset();                  // everything freed at once, O(1)
```

There is no `free(x)`. No per-object deallocation. No fragmentation. No free list. The allocator is four instructions: load, add, compare, store.

This is the same model video games have used for decades. A frame starts, everything allocates from the frame arena, the frame ends, the arena resets. No individual deallocations. No memory leaks. No garbage collector pauses.

The tradeoff is that individual objects can't outlive their arena. If you need a value to live longer, you either allocate it in a longer-lived arena or copy it.

---

## The Scope Ladder: Compile-Time Escape Analysis

The arena model works because Salt can prove, at compile time, that no arena pointer outlives its arena. This is the **Scope Ladder**.

Every variable gets an integer depth based on its lexical scope:

| Depth | Example |
|-------|---------|
| 0 | Module-level globals |
| 1 | Function arguments (outlive the body) |
| 2 | Function-local variables |
| 3+ | Block-scoped variables (`if`/`while`/`for`) |

Arena pointers inherit the depth of the arena they were allocated from. Three rules govern all assignments and returns:

**Rule 1: Return Rule.** `return x` is valid only if `depth(x) <= 1`. You can't return a pointer into a local arena.

```salt
fn create_dangling() -> Ptr<Node> {
    let arena = Arena::new(4096);   // depth 2 (local)
    let n = arena.alloc(Node{});    // depth 2 (inherits from arena)
    return n;                       // ❌ depth 2 > 1 — REJECTED at compile time
}
```

**Rule 2: Assignment Rule.** `a = b` is valid only if `depth(b) <= depth(a)`. You can't store a short-lived pointer in a long-lived container.

```salt
fn store_escape(bucket: &Bucket) {
    let arena = Arena::new(4096);   // depth 2 (local)
    let n = arena.alloc(Node{});    // depth 2
    bucket.node = n;                // ❌ depth(bucket) ≤ 1, depth(n) = 2 — REJECTED
}
```

**Rule 3: Transitivity Rule.** `s.field` inherits `depth(s)`. If you can't store `x` in `s`, you can't store `x` in `s.field` either.

Three rules. No annotations. The compiler infers depths from the AST and checks every assignment and return statement.

---

## What This Catches

The Scope Ladder catches the three classic memory bugs:

| Bug | Example | Caught by |
|-----|---------|-----------|
| Use-after-free | Read after `arena.reset()` | Z3 epoch tracking (debug) + poison fills |
| Dangling return | Return pointer to local arena | Rule 1 (compile time) |
| Cross-lifetime store | Store local pointer in global struct | Rule 2 (compile time) |

The compile-time checks have zero runtime cost. The debug checks (poison fills, epoch tracking) are enabled with `SALT_DEBUG` and add ~5% overhead — comparable to ASAN.

---

## When Arenas Don't Work

Arenas work when your allocation pattern is: allocate many objects, use them for a bounded period, free them all at once. This describes request handlers, frame renderers, compiler passes, and kernel operations.

Arenas don't work for:

- **Arbitrary graph structures with independent lifetimes.** A DOM tree where nodes are created and destroyed independently needs either a GC or manual memory management.
- **Long-lived caches.** If objects live for minutes or hours, an arena that can't be reset until the last object dies wastes memory.
- **Cyclic references.** Arenas don't collect cycles. If A points to B and B points to A, both live until the arena resets.

For these cases, Salt provides a separate `Heap` allocator with reference counting — `Rc<T>` and `Arc<T>`. It's slower but handles the general case. The convention is: use arenas by default, reach for `Rc` only when you need independent lifetimes.

---

## Comparison with Rust

A Rust function with arena-allocated return values:

```rust
fn create_node<'a>(arena: &'a Arena, val: i32) -> &'a Node {
    arena.alloc(Node { val })  // lifetime 'a tied to arena
}
```

The equivalent Salt:

```salt
fn create_node(arena: Arena, val: i32) -> Ptr<Node> {
    return arena.alloc(Node { val });  // depth inferred from arena
}
```

Rust requires an explicit lifetime parameter `'a` on the function, the argument, and the return type. Salt infers it from the depth of `arena` (depth 1, because it's an argument) and propagates it to the return value.

Neither approach is strictly better. Rust's lifetime annotations are more expressive — they can express partial borrows, non-lexical lifetimes, and complex ownership graphs that the Scope Ladder can't. Salt's inference is simpler for the common cases: arena allocation, request scoping, and buffer management. The trade is expressiveness for annotation burden.

For systems code where allocation patterns are simple — the kernel, a network server, a compiler — Salt's model covers ~90% of cases with zero annotations. For the remaining 10%, there's `Rc<T>` and `unsafe`.

---

## The Debug Layer: Poison Fills and Z3 Epochs

Compile-time checks catch escape. Debug checks catch use-after-reset.

When `SALT_DEBUG` is enabled, every `arena.reset()` fills the freed region with `0xAA`. Any subsequent read from a dangling pointer hits the poison value and traps. This is the same technique as ASAN's use-after-free detection, but scoped to arena boundaries.

Z3 verification adds a second layer: an epoch counter per arena, incremented on each `reset()`. Every pointer carries the epoch of its allocation. Before each pointer dereference, the compiler emits a Z3 proof obligation: `ptr.epoch == arena.epoch`. If Z3 can't prove it, you get a runtime assertion. If Z3 can prove it always holds (e.g., the pointer is used before the first `reset()`), the check is elided.

---

## The Bottom Line

Salt's arena model isn't a general-purpose replacement for borrow checking. It's a specialized tool for the allocation patterns that dominate systems code: allocate, use, reset. For those patterns, it provides equivalent safety with zero annotations.

The trade is real: arenas can't express the complex ownership graphs that Rust's lifetimes can. But for the kernel, the network stack, and the compiler, they don't need to.

[Read the arena deep-dive →](/docs/deep-dives/arena-safety.md)
