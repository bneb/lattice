# The Region Memory Model

Salt's safety and performance are anchored in the **Region Memory Model** (RMM). While languages like Rust use a global borrow checker to manage memory safety, Salt uses hardware-aware regions to partition the system's address space.

## Comparison: Salt RMM vs. Rust Borrow Checker

| Feature | Salt Region Model | Rust Borrow Checker |
| :--- | :--- | :--- |
| **Primary Mechanism** | Spatial Segmentation | Temporal Ownership |
| **Complexity** | O(1) — Constant checking | O(N) — Complex lifetime resolution |
| **Bare-Metal Support** | Native (Hardware Regions) | Requires `no_std` + manual glue |
| **Stack Stability** | Enforced via Hoisting Law | Left to individual implementers |
| **Concurrency** | Lock-free by design | Based on `Send`/`Sync` traits |

## The Mechanics of Regions

In Salt, every memory access is scoped to a specific region. A region is a contiguous block of memory with dedicated permissions and alignment rules.

### 1. The Stack Region (`Stack`)
The stack is a linear segment where local variables reside. Salt enforces the **Memory Hoisting Law**, which mandates that all stack allocations (`alloca`) occur at the start of a function. This prevents stack-based side channels and heap-like fragmentation on the stack.

```salt
fn example() {
    let a: i32 = 42;     // Hoisted to function entry as alloca
    let b: i64 = 100;    // Hoisted to function entry as alloca
    // All allocas happen before any computation
}
```

### 2. The RAM Region (`RAM`)
Global data and long-lived structures reside in the `RAM` region. Access to `RAM` is mediated through the **Linear Resolution Principle**, ensuring that module-resident symbols resolve to fixed global addresses at compile time.

### 3. The I/O Region (`IO`)
Memory-mapped I/O is treated as a first-class region. Accessing peripherals like the `DEBUG_PORT` in the KeuOS kernel is done through zero-cost address flattening.

## Why Regions?

For a high-assurance kernel, predictability is as important as safety. Standard borrow checking can lead to complex code that is difficult to audit and verify. By using regions, Salt provides a simple, verifiable model that maps directly to the underlying hardware page tables.

> **Safety = Region Isolation + Invariants**

Each region enforces spatial isolation — a pointer allocated in one region cannot be dereferenced in another without explicit conversion, which the compiler tracks and the verifier can prove correct.
