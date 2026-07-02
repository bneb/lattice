# Salt `unsafe` Blocks

## Overview

`unsafe` in Salt is a **restricted escape hatch** for operations that bypass Salt's memory safety guarantees. Unsafe blocks are only allowed in three package trees: `std.*`, `kernel.*`, and `basalt.*`. Any package outside this allowlist is rejected at compile time.

> [!IMPORTANT]
> This is a deliberate design choice. Salt's safety model guarantees that if your code compiles without `unsafe`, it is memory-safe. Since only approved packages can use `unsafe`, the attack surface for memory bugs is small and auditable.

## What `unsafe` Gates (Allowlisted Packages Only)

| Operation | Safe Alternative |
|-----------|-----------------|
| Raw pointer construction from integer (`42 as Ptr<u8>`) | `Vec<T>`, `Arena`, `File::open()` |
| `reinterpret_cast<T>(ptr)` | Pattern matching, `as` for numeric casts |
| Direct `sys_*` syscall FFI | `File`, `std.os.tcp`, `std.env` |
| Manual memory layout assumptions | `struct` with compile-time layout |

> [!NOTE]
> Inside an `unsafe` block, pointer bounds verification is deferred to the programmer. The compiler continues to verify `requires`/`ensures` contracts at call sites — only the pointer provenance checks are suppressed, not the Z3 contract verification.

## How It Works

```salt
// In stdlib code (e.g., std/core/mem.salt):
unsafe {
    let raw = salt_mmap(size) as Ptr<u8>;  // OK — stdlib can do this
    raw.write(0);
}

// In user code:
unsafe {  // ← COMPILE ERROR: unsafe blocks are not allowed in user code
    let raw = 42 as Ptr<u8>;
}
```

## Why Not Rust's Model?

Rust allows `unsafe` in any crate. Salt takes a stricter approach:

- **Smaller audit surface**: Only `~20 files` in `salt-front/std/` need safety review
- **Simpler mental model**: "If it compiles, it's safe" — no need to audit deps
- **Ecosystem safety**: Third-party Salt packages cannot introduce memory unsafety

## Why `basalt.*`?

[Basalt](/Users/kevin/projects/lattice/basalt/README.md) is the project's reference application — an LLM inference engine written entirely in Salt that targets both native binaries and WebAssembly. It needs `unsafe` for three operations that have no safe alternative in the current language:

1. **Memory-mapped weight loading** — `mmap` syscall for zero-copy model loading (no allocator overhead for multi-gigabyte weight files)
2. **Raw pointer arithmetic in SIMD kernels** — v128 vector intrinsics for WASM SIMD (`v_load`, `v_fma`, `v_hsum`) operate on raw `Ptr<f32>` with explicit strides
3. **f16→f32 bitcast** — hardware-level reinterpret of 16-bit floats without an intermediate arithmetic conversion

Basalt is the compiler's stress test: it ships with heavy compile-time verification (Z3 contracts on every kernel), is <1000 SLOC, and proves that Salt can replace C in performance-critical ML workloads. Keeping it in the `unsafe` allowlist avoids introducing a separate "trusted compute" mechanism while maintaining the same auditability standard as `std.*` and `kernel.*`.

## For Stdlib and Basalt Authors

When writing stdlib code that requires `unsafe`:
1. Keep the `unsafe` block as small as possible
2. Document the safety invariant in a comment
3. Expose a safe public API that upholds the invariant
