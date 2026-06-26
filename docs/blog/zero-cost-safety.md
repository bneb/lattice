# Zero-Cost Safety: How Salt Proves Memory Safety at Compile Time

**Published:** June 2026 | **Author:** The KeuOS Team | **Reading time:** 12 minutes

---

Every CVE is a memory bug. Bounds overflows, use-after-free, null dereferences — year after year, the same classes of vulnerability dominate the CVE database. The industry has tried everything: static analysis (Coverity), sanitizers (ASAN, UBSAN), safer dialects (MISRA C). Each catches some bugs. None provides certainty.

Rust changed the conversation. Its borrow checker proves memory safety at compile time — no use-after-free, no data races. But the proof comes with a cost: lifetime annotations on every reference, `Arc<Mutex<T>>` for shared state, `unsafe` blocks for FFI. The cognitive overhead is real.

Zig takes the opposite approach: no hidden control flow, no implicit allocations, full C interop. But its safety story is runtime checks in debug builds — stripped in release. You ship without the net.

Salt asks a different question: **what if the compiler could prove your safety properties mathematically, and simply not emit the check?**

---

## The Idea: Proof-or-Panic

Salt embeds the [Z3 theorem prover](https://github.com/Z3Prover/z3) directly in the compiler. Every `requires` clause on a function becomes a Z3 proof obligation at compile time. The outcome is binary:

1. **Z3 proves it** → The check is **completely elided**. Zero instructions emitted. Zero runtime cost.
2. **Z3 can't prove it** → A runtime assertion fires if the contract is violated. Safe fallback, no UB.

There is no third path. Every contract is either mathematically proven or runtime-enforced.

Here's the simplest example:

```salt
fn safe_div(a: i32, b: i32) -> i32
    requires(b != 0)
{
    return a / b;
}

fn main() -> i32 {
    let x = safe_div(100, 7);    // ✓ Z3 proves 7 != 0 — check elided
    // let y = safe_div(100, 0); // ✗ COMPILE ERROR: Z3 finds counterexample b=0
    return 0;
}
```

When you call `safe_div(100, 7)`, Z3 proves `7 != 0` is always true, so the generated binary contains **no branch, no assertion, no overhead**. The `requires` clause evaporates.

When you call `safe_div(100, 0)`, the compiler reports:

```
VERIFICATION ERROR: could not prove '(b != 0)'
  context: precondition check at call site
  counterexample:
    b = 0
  hint: the argument 'b' must be non-zero
```

This isn't a runtime panic. It's a **compile error** — your program never ships with a provably-violated contract.

---

## How It Works: From `requires` to Z3 to MLIR

The verification pipeline has three stages:

```
Salt source with requires(b != 0)
        │
        ▼
Translate to Z3 formula: (assert (not (= b 0)))
        │
        ▼
Z3 checks satisfiability (100ms timeout):
        │
        ├── UNSAT (no counterexample exists)
        │       → Condition ALWAYS holds
        │       → ELIDE CHECK — emit nothing
        │
        ├── SAT (counterexample found: b = 0)
        │       → Condition CAN be violated
        │       → COMPILE ERROR with counterexample
        │
        └── UNKNOWN (Z3 timeout)
                → Cannot determine
                → Emit runtime assertion as safe fallback
```

The critical insight: when Z3 returns UNSAT, it has **mathematically proven that no input can violate the contract**. There is no counterexample in the entire input space. The check is redundant — and the compiler removes it.

When Z3 returns UNKNOWN (typically a 100-millisecond timeout on complex path conditions), the compiler emits a standard MLIR runtime assertion:

```mlir
%violated = arith.xori %cond, %true : i1
scf.if %violated {
    func.call @__salt_contract_violation() : () -> ()
    scf.yield
}
```

This uses only standard MLIR dialects — `arith`, `scf`, `func`. No custom dialect ops. No special runtime. The fallback is a plain conditional branch that any LLVM backend can optimize.

---

## Concrete Example: Bounds-Checked Binary Search

Here's a binary search with a `requires` contract that Z3 can prove:

```salt
fn binary_search(arr: &[i32], target: i32) -> Result<i32>
    requires(arr.len() > 0)
{
    let mut lo: i64 = 0;
    let mut hi: i64 = arr.len() - 1;

    while lo <= hi {
        let mid = lo + (hi - lo) / 2;
        if arr[mid] == target {
            return Result::Ok::<i32>(mid as i32);
        }
        if arr[mid] < target {
            lo = mid + 1;
        } else {
            hi = mid - 1;
        }
    }
    return Result::Err::<i32>(-1);
}

fn main() -> i32 {
    let sorted: [i32; 5] = [1, 3, 5, 7, 9];
    let found = binary_search(&sorted, 5);
    // Z3 proves: arr.len() == 5 > 0 ✓
    // Z3 proves: all array accesses are in-bounds ✓
    return 0;
}
```

At the call site `binary_search(&sorted, 5)`, Z3 proves two things:

1. **The precondition holds**: `arr.len()` is 5, and `5 > 0` is true for all possible execution paths reaching this call site.
2. **All array accesses are in-bounds**: every `arr[mid]` is accessed with `mid` in the range `[0, arr.len() - 1]`, which Z3 verifies by analyzing the loop bounds.

Neither check exists in the generated binary. The binary search compiles to the same machine code you'd write by hand — minus the safety annotations.

---

## Contracts on Kernel Operations

KeuOS uses Z3 contracts throughout the kernel. Here's a real example from the Physical Memory Manager:

```salt
// Prevents invalid memory regions at the compiler level
pub fn init(start: u64, end: u64)
    requires(start < end)
{
    // Z3 proves start < end at every call site
    // ... initialize the page allocator ...
}
```

And from the IPC subsystem:

```salt
// Prevents wrap-around and oversized rings
fn map_ring(descriptor: RingDescriptor)
    requires(descriptor.size > 0 && descriptor.size <= 0x100000)
{
    // Z3 proves the ring is non-empty and under 1MB
    // ... map SPSC ring pages into the caller's address space ...
}
```

These aren't examples from a whitepaper. They ship in the kernel. Every call site that passes a compile-time-known value gets the check elided. Every call site with a runtime value gets a branch — just like Rust's `assert!`, but with Z3 doing the work of removing the provably-redundant ones.

---

## The Limits: What Z3 Can't Prove

Z3 handles linear integer arithmetic, bounded multiplication, bitwise operations, and comparisons across conditional branches. What it can't prove:

- **Non-linear integer arithmetic**: `a * b` where both `a` and `b` are unbounded variables. Z3 can reason about multiplication when at least one operand is bounded.
- **Floating-point**: Z3's float theory is incomplete. Use integers for contract properties.
- **Deeply nested loops and pointer-chasing**: path explosion hits the 100ms timeout.
- **String operations**: not in the solver domain.

When Z3 can't decide, the compiler emits a runtime assertion. This is **progressive verification**: add contracts incrementally, prove what you can, runtime-check the rest.

[Full capability reference →](deep-dives/z3-capabilities.md)

---

## Comparison: Salt vs. Rust vs. C

| | Salt | Rust | C |
|---|---|---|---|
| **Memory safety guarantee** | Arena model + contracts | Borrow checker | None (manual) |
| **Lifetime annotations** | None (inferred) | Required on references | N/A |
| **Compile-time proofs** | Z3 on `requires`/`ensures` | Trait bounds, type system | None |
| **Bounds check cost** | Elided when Z3 proves | `unwrap()` panic path | Silent UB if missed |
| **FFI safety** | `@trusted` with explicit reason | `unsafe { }` blocks | Everything is unsafe |
| **What ships** | Proven code only | Safe + unsafe code | All code |

Rust's borrow checker is a remarkable achievement — it proves absence of aliasing bugs without a GC. But it proves them through a type system that requires explicit annotation. Salt's arena model achieves equivalent safety through region-based allocation with inferred lifetimes, and adds mathematical proofs on top.

C's approach — "trust the programmer" — has been empirically disproven by 30 years of CVEs.

---

## What This Enables

The Salt compiler is open source. The KeuOS kernel boots in QEMU. The `requires` and `ensures` contracts are documented and tested.

Here's what you can build with compile-time proofs:

- **Verified kernel operations**: PMM allocations, IPC ring mappings, interrupt handler registrations — all with `requires` clauses that Z3 proves at compile time
- **Bounds-checked data structures**: Binary search, ring buffers, hash tables — array accesses proven in-bounds before the binary ships
- **Contract-carrying libraries**: Publish a library with `requires` on its public API — consumers get compile-time verification of correct usage

---

## Try It

The compiler ships with a tutorial that walks through contracts in Chapter 9:

```bash
git clone https://github.com/bneb/keuos
cd keuos/docs/tutorial
cat 09-contracts.md
```

Read the [full tutorial](/docs/tutorial/), check out the [language specification](/docs/SPEC.md), and open a PR. We're building a language where safety is a proof, not a prayer.
