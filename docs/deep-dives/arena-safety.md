# Arena Safety Model — Design Deep-Dive

Salt's Arena memory system provides **compile-time temporal safety with zero runtime cost and zero annotations**. It catches dangling pointer bugs at compile time through a depth-based escape analysis called the **Scope Ladder**, complemented by runtime poison fills in debug builds and Z3 formal verification.

## The Safety Stack

| Layer | What It Catches | When | Cost |
|-------|----------------|------|------|
| **Escape Analysis** (Scope Ladder) | Dangling returns, cross-lifetime stores | Compile-time | Zero |
| **Poison Fills** (`SALT_DEBUG`) | Use-after-reset, stale reads | Debug runtime | Debug-only memset |
| **Z3 Verification** (ArenaVerifier) | Use-after-reset, epoch violations | Compile-time | Zero |

## The Scope Ladder

Every variable is assigned an integer **depth** based on its lexical scope:

| Depth | Meaning | Example |
|-------|---------|---------|
| 0 | Global / static | Module-level constants |
| 1 | Function arguments | `fn process(arena: Arena)` — outlives the body |
| 2 | Local variables (function body) | `let arena = Arena::new(4096)` |
| 3+ | Nested blocks | Arena inside `if`/`while`/`for` |

Arena pointers **inherit the depth of the arena they were allocated from**:

```salt
fn create_node(arena: Arena) -> Ptr<Node> {
    # arena: depth 1 (argument)
    let n = arena.alloc::<Node>(Node { val: 42 })
    # n: depth 1 (inherits from arena)
    return n  # ✅ depth 1 ≤ 1 — safe
}

fn create_dangling() -> Ptr<Node> {
    let arena = Arena::new(4096)
    # arena: depth 2 (local)
    let n = arena.alloc::<Node>(Node { val: 42 })
    # n: depth 2 (inherits from arena)
    return n  # ❌ depth 2 > 1 — REJECTED
}
```

### The Three Laws

1. **Return Rule**: `return x` is valid iff `depth(x) ≤ 1`. Local arena pointers cannot escape their function.

2. **Assignment Rule**: `a = b` is valid iff `depth(b) ≤ depth(a)`. Cannot store a short-lived pointer into a long-lived container.

3. **Transitivity Rule**: `s.field` inherits `depth(s)`. Struct fields carry the depth of their parent.

### What It Catches

```salt
# ❌ Law I violation: Return escape
fn bad_return() -> Ptr<Node> {
    let arena = Arena::new(4096)       # depth 2
    let n = arena.alloc::<Node>(...)   # depth 2
    return n                            # REJECTED: depth 2 > 1
}

# ❌ Law II violation: Store escape
fn bad_store(ctx: Ptr<Context>) {
    let local_arena = Arena::new(4096)       # depth 2
    let data = local_arena.alloc::<i64>(99)  # depth 2
    ctx.saved_ptr = data                      # REJECTED: depth 2 > depth 1
}
```

### What It Allows

```salt
# ✅ Output parameter pattern — the safe idiom
fn create_node(arena: Arena) -> Ptr<Node> {
    let n = arena.alloc::<Node>(Node { val: 42 })  # depth 1
    return n                                         # depth 1 ≤ 1 — safe
}

# ✅ Same-depth stores
fn process(arena: Arena) {
    let a = arena.alloc::<i64>(1)  # depth 1
    let b = arena.alloc::<i64>(2)  # depth 1
    # Both from same arena — same depth — stores between them are fine
}
```

## Comparison With Other Languages

| Dimension | C | C++ | Rust | Salt |
|-----------|---|-----|------|------|
| **Temporal safety** | None | `unique_ptr` (no arena tracking) | Lifetime annotations | Scope Ladder (zero annotations) |
| **Developer effort** | N/A | Manual | `'a` annotations on every signature | Zero — just write obvious code |
| **Runtime cost** | N/A | Zero | Zero | Zero |
| **Arena ergonomics** | Manual free | Destructor-based | Painful with self-referential types | First-class, simple |
| **Error quality** | Segfault at runtime | Segfault at runtime | Opaque lifetime errors | Clear depth-violation messages |

### Rust comparison in detail

Rust requires explicit lifetime annotations for arena patterns:

```rust
// Rust — lifetime annotations required
fn create_node<'a>(arena: &'a Arena) -> &'a Node {
    arena.alloc(Node { val: 42 })
}
```

```salt
# Salt — zero annotations, same safety guarantee
fn create_node(arena: Arena) -> Ptr<Node> {
    return arena.alloc::<Node>(Node { val: 42 })
}
```

Salt achieves the same guarantee for arena patterns without any lifetime syntax. The trade-off: Salt's analysis is intraprocedural and scope-based, while Rust's borrow checker handles arbitrary reference graphs. For arena-based allocation (the dominant pattern in ML, networking, and game engines), Salt's simpler model covers the critical cases.

## Spatial Safety

Arena allocation is always typed: `arena.alloc::<Node>(val)` returns `Ptr<Node>`, not `void*`. The compiler knows the layout, prevents type confusion, and verifies field accesses at compile time. Z3 verification extends to arena-allocated pointers for bounds checks and validity tracking.

## Runtime Cost

The escape analysis is purely compile-time — it gates compilation, never emits runtime checks. The generated MLIR/LLVM IR is identical whether escape analysis is enabled or not.

The arena allocator itself is the cheapest possible:
- **Allocation**: Single integer add (bump pointer)
- **Deallocation**: No-op
- **Reset**: Single pointer reset
- **Debug poison**: `memset(0xDD)` on reset (debug builds only, `#ifdef`-gated)

## The Mental Model

A developer needs to internalize exactly three rules:

1. **Don't return local arena pointers** — if the arena was created in this function, its pointers die here
2. **Don't store local arena pointers into argument structs** — the struct outlives the arena
3. **Everything else just works** — pass arena as argument, allocate, return, compose freely

If they write dangerous code, the compiler tells them exactly why:

```
Arena escape violation: pointer 'n' (depth 2) cannot be returned.
It was allocated from a local arena that dies when this function returns.
```

```
Arena escape violation: pointer 'data' (depth 2) stored into 'ctx' (depth 1).
The source has a shorter lifetime than the destination.
```

## Honest Gaps

| Gap | Description | Mitigation |
|-----|-------------|------------|
| **Intraprocedural only** | Cross-function pointer flows rely on the depth-1 heuristic for arguments | Conservative — allows safe patterns, may miss some cross-function escapes |
| **No raw pointer bounds** | `ptr.offset(100)` past an arena region isn't caught | Arena is a systems primitive; typed access prevents most misuse |
| **Turbofish noise** | `arena.alloc::<Node>(...)` requires explicit type | Future type inference improvement; not a safety issue |

## Vec<T, A> Allocator-Aware Tracking

The Scope Ladder extends to generic containers through **provenance chain tracking**:

```
Arena::new(4096)  →  ArenaAllocator { arena }  →  Vec::new(alloc, cap)
    depth 2              depth 2                      depth 2
```

- `Vec<T, ArenaAllocator>` with a **local** arena → depth 2+ → **cannot escape**
- `Vec<T, ArenaAllocator>` from an **argument** arena → depth 1 → **can escape**
- `Vec<T, HeapAllocator>` → **not tracked** → always allowed (global lifetime)

```salt
# ❌ Arena Vec escape — REJECTED
fn bad() -> Vec<i64, ArenaAllocator> {
    let arena = Arena::new(4096)              # depth 2
    let alloc = ArenaAllocator { arena: arena } # depth 2
    let v = Vec::new(alloc, 4)                # depth 2
    return v                                   # REJECTED: depth 2 > 1
}

# ✅ Heap Vec — ALLOWED
fn ok() -> Vec<i64, HeapAllocator> {
    let alloc = HeapAllocator {}              # not tracked
    let v = Vec::new(alloc, 4)                # not tracked
    return v                                   # ALLOWED
}
```

## Implementation

The analysis is implemented in 5 files across the compiler:

| File | Role |
|------|------|
| `verification/arena_escape.rs` | `ArenaEscapeTracker` — depth map + three laws |
| `codegen/stmt.rs` | Hooks: arena/allocator/Vec registration, return check |
| `codegen/expr/mod.rs` | Return check (expression path) + store check |
| `codegen/expr/binary_ops.rs` | Assignment-path escape checking |
| `codegen/mod.rs` | Function arg registration, tracker save/restore |
| `codegen/context.rs` | `arena_escape_tracker` field on `CodegenContext` |

**Test Results** (February 2026):
- 1,011 compiler unit tests pass
- 22/22 tracked benchmarks build and run, 19/22 Salt ≤ C
- Arena escape tests pass (including Vec<T, A> provenance chain)
- 0 false positives on real-world code
