# Chapter 5: Arenas and Memory

## What Is an Arena?

An arena is the simplest possible memory allocator: a bump pointer over a pre-allocated block. You allocate by advancing the pointer. You free by rewinding it to an earlier position. No free list, no malloc metadata, no garbage collector pauses — just increment and compare.

```salt
import std.core.arena.Arena

fn main() -> i32 {
    // A 4 KiB arena (the kernel rounds up; this is an advisory capacity)
    let arena = Arena::new(4096);

    // Allocate integers from the arena (~2 ns per call)
    let x = arena.alloc::<i64>(10);
    let y = arena.alloc::<i64>(20);
    let z = arena.alloc::<i64>(30);

    // Free everything in O(1) — no individual frees needed
    let mark = arena.mark();
    let _ = arena.alloc::<i64>(99);
    arena.reset_to(mark);  // back to where we were after z

    let sum = *x + *y + *z;  // 60 — x, y, z are still valid
    println(f"sum = {sum}");
    return 0;
}
```

Key properties:
- **Bump allocation** — the allocator never searches for a free slot; it just advances a pointer.
- **O(1) free** — bulk-reclaim everything since a `mark` in constant time.
- **No per-object tracking** — you cannot free individual allocations; you reset the whole region.

## When Arenas Shine

Arenas excel when memory has a natural **epoch**: a request, a frame, a batch, or a single pass.

```salt
import std.core.arena.Arena

struct Row { id: i64, name: StringView }

fn read_rows(arena: &Arena, count: i64) -> Ptr<Row> {
    let rows = arena.alloc_array::<Row>(count);
    let mut i: i64 = 0;
    while i < count {
        rows[i] = Row { id: i, name: "item" };
        i += 1;
    }
    return rows;
}

fn process_batch(batch_size: i64) -> i64 {
    let arena = Arena::new(65536);
    let mark = arena.mark();
    let rows = read_rows(&arena, batch_size);
    let sum = rows[batch_size - 1].id;
    arena.reset_to(mark);  // all rows gone in O(1)
    return sum;
}
```

Every request, batch, or frame gets its own arena. When processing ends, the entire region is reclaimed at once — no `free()` loops, no double-free bugs, no memory leaks.

## When Arenas Do Not Work

Arenas cannot free individual objects. If your data structure needs to release memory at arbitrary points — a graph where nodes are deleted in any order, or a cache that evicts entries independently — a bump allocator is the wrong tool.

```salt
// BAD: arena can't free a single node
struct Node { value: i64, next: Ptr<Node> }

fn build_list(arena: &Arena, count: i64) -> Ptr<Node> {
    let head = arena.alloc::<Node>(Node { value: 0, next: Ptr::empty() });
    let mut cur = head;
    let mut i: i64 = 1;
    while i < count {
        let n = arena.alloc::<Node>(Node { value: i, next: Ptr::empty() });
        cur.next = n;
        cur = n;
        i += 1;
    }
    return head;
}
// Problem: no way to remove the 3rd node without resetting the whole arena.
```

For arbitrary graph structures with independent lifetimes, reach for a general-purpose allocator or reference-counted types instead.

## The Scope Ladder

Salt prevents arena pointers from outliving their region at compile time. Every pointer carries a **depth** equal to the arena it was allocated from. Three rules govern pointer movement:

- **Return rule**: `return x` requires `depth(x) <= 1`. An arena allocated on the stack cannot have its pointers returned.
- **Assignment rule**: `dst = src` requires `depth(src) <= depth(dst)`. A short-lived pointer cannot be stored in a longer-lived slot.
- **Transitivity**: `s.field` inherits `depth(s)`.

```salt
import std.core.arena.Arena
import std.core.ptr.Ptr

fn return_ok(arena: &Arena) -> Ptr<i64> {
    // arena has depth 1 (function parameter)
    return arena.alloc::<i64>(42);  // OK: depth(ptr) = 1
}

fn return_bad() -> Ptr<i64> {
    let local = Arena::new(256);   // depth 2
    let p = local.alloc::<i64>(1); // depth 2
    // return p;                   // REJECTED: depth 2 > 1
    return Ptr::empty();            // OK
}

fn store_bad(arena: &Arena, out: &mut Ptr<i64>) {
    let local = Arena::new(256);   // depth 2
    let p = local.alloc::<i64>(99);  // depth 2
    // out = p;                    // REJECTED: depth 2 > depth 1
}

fn transitivity(arena: &Arena) -> i64 {
    let p = arena.alloc::<i64>(42);  // p has depth 1
    // p.field inherits depth(p), so p.field is also depth 1
    return *p;  // OK
}
```

When a pointer is stored in a struct field, the field inherits the pointer's depth (Law III). This prevents storing a shallow pointer into a deeply nested struct that outlives it.

## Comparison: Salt vs. malloc/free vs. Rust

| Concern | C malloc/free | Rust borrow checker | Salt arena + ladder |
|---|---|---|---|
| Allocation | `malloc` (tens of ns) | `Box::new`, `Vec::push` | Bump pointer (~2 ns) |
| Deallocation | `free` (per-object) | `drop` on scope exit | `arena.reset_to` (O(1)) |
| Memory safety | Manual (leaks, use-after-free) | Static, proven by `borrowck` | Static depth analysis |
| Fragmentation | Yes | Depends on allocator | None (linear bump) |
| Free individual objects | Yes | Yes (drop) | No (region only) |

Salt trades per-object deallocation for speed and simplicity. The scope ladder makes the trade-off safe: the compiler proves every arena pointer stays within its region's lifetime, so arena-reset is always sound.

```salt
// C analogy: must free each allocation individually
// int *p = malloc(sizeof(int));
// int *q = malloc(sizeof(int));
// free(p); free(q);

// Salt: one reset for both
let p = arena.alloc::<i32>(1);
let q = arena.alloc::<i32>(2);
arena.reset_to(arena.mark());  // frees both
```

## Summary

- An arena is a bump allocator: allocate forward, reclaim by rewinding.
- Use arenas for request-scoped, frame-scoped, or batch-scoped work.
- Do not use arenas for graph structures with independent node lifetimes.
- The scope ladder enforces pointer-depth rules at compile time — no escapes.
- Arena allocation is ~2 ns; bulk reset is O(1). No fragmentation.

Next: [Chapter 6: Error Handling](06-error-handling.md)
