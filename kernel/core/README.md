# Kernel Core

**The Mission:** The platform-independent nucleus of Lattice, orchestrating threads (fibers), memory, and system integrity.

## Invariants

> [!NOTE]
> **The Sovereign Invariants**
> These mathematical laws are enforced by the Salt compiler and verified by Z3.

### 1. The 24-Byte Fiber Stride
The `Fiber` struct mimics the strict packing of cache lines.
$$sizeof(Fiber) = 8 (id) + 8 (stack\_ptr) + 8 (active + padding) = 24 \text{ bytes}$$
This alignment ensures that iterating over fiber slots is cache-friendly and predictable for the prefetcher.

### 2. Memory Hoisting Law
**No dynamic allocation is permitted inside the scheduler loop.**
- `pmm.salt`: Uses a static stack (`[u64; 32768]`) for page tracking.
- `scheduler.salt`: Uses a fixed-size array (`[Fiber; 16]`) for process table.

### 3. Verification Contracts
The Physical Memory Manager (`pmm.salt`) uses formal pre/post-conditions:
```salt
concept StackBounded<T> {
    requires(top: T) { top != 0 }
}
```

## Components

| File | Role | Key Function |
|------|------|--------------|
| [`scheduler.salt`](./scheduler.salt) | **Round-Robin Scheduler.** Manages the 16 available fiber slots. | `sched_yield()`: The cooperative context switch. |
| [`context_switch.salt`](./context_switch.salt) | **Context Actuation.** The safe wrapper around the assembly switch. | `swap_next()`: Actuates the register swap. |
| [`pmm.salt`](./pmm.salt) | **Physical Memory Manager.** A verifying stack-based page allocator. | `alloc()`: Pops a page from the free stack. |
| [`context.salt`](./context.salt) | **Register State.** Defines the saved state of a paused thread. | `struct Context`: Must match `push` order in ASM. |

## Entry & Critical Paths

### The Context Switch Loop (~487 cycles on KVM)
The critical path for performance is defined in `scheduler.salt` -> `sched_yield`.
1. **Check Yield Pending:** `GLOBAL_SCHED.fibers[next].active`
2. **Select Next:** Round-robin logic.
3. **Actuate:** `kernel.core.context_switch.swap_next(old, new_sp)`

### Troubleshooting
**Symptom:** "Kernel hangs after 'Starting Scheduler...'"
- **Cause:** The `timer_isr` is not firing, or `enable_interrupts()` in `start()` failed.
- **Fix:** Verify `arch/x86_64/idt.S` is correctly mapped and `sti` was executed.

**Symptom:** "General Protection Fault (GPF) on Switch"
- **Cause:** Stack alignment violation. The `stack_init` in `scheduler.salt` must produce a 16-byte aligned stack pointer *after* the return address is pushed.
- **Check:** `let stack_top = stack_base - (slot as u64 * 0x8000);` Ensure `0x8000` stride prevents overlap.
