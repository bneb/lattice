# Kernel Core

**The Mission:** The platform-independent nucleus of KeuOS, orchestrating threads (fibers), memory, and system integrity.

## Invariants

> [!NOTE]
> **The KeuOS Invariants**
> These mathematical laws are enforced by the Salt compiler and verified by Z3.

### 1. The Fiber Stride
The `Fiber` struct contains 7 fields for full context migration across cores:
- `id`, `stack_ptr`, `stack_base`, `active`, `step_fn`, `task_frame`, `ctx_ptr`
This layout supports both async state-machine fibers and preemptive IRETQ fibers with zero-branch dispatch.

### 2. Memory Hoisting Law
**No dynamic allocation is permitted inside the scheduler loop.**
- `pmm.salt`: Uses a static stack (`[u64; 32768]`) for page tracking.
- `scheduler.salt`: Uses a fixed-size per-core array (`SCHED_ARRAY[16] × [Fiber; 256]`) — zero malloc in the scheduler.
- `chase_lev.salt`: Uses static `DEQUE_BUFFERS[16][1024]` — zero malloc for work-stealing deques.

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
| [`scheduler.salt`](./scheduler.salt) | **O(1) Bitmap Scheduler with Chase-Lev Work-Stealing.** 256 fibers/core × 16 cores. Idle cores steal from sibling deques via full 7-field fiber migration. | `sched_yield()`: Unified dispatch via `invoke_task(step_fn, ctx_ptr)`. |
| [`context_switch.salt`](./context_switch.salt) | **Context Actuation.** The safe wrapper around the assembly switch. | `swap_next()`: Actuates the register swap. |
| [`pmm.salt`](./pmm.salt) | **Physical Memory Manager.** A verifying stack-based page allocator. | `alloc()`: Pops a page from the free stack. |
| [`context.salt`](./context.salt) | **Register State.** Defines the saved state of a paused thread. | `struct Context`: Must match `push` order in ASM. |

## Entry & Critical Paths

### The Context Switch Loop (~487 cycles on KVM)
The critical path for performance is defined in `scheduler.salt` -> `sched_yield`.
1. **Check Yield Pending:** `SCHED_ARRAY[cpu].yield_pending`
2. **Select Next:** O(1) bitmap scan via hardware TZCNT (`ctz_u64`). Chase-Lev `steal_scan` for idle cores.
3. **Dispatch:** `invoke_task(step_fn, ctx_ptr)` — zero-branch Universal Task Pointer.

### Troubleshooting
**Symptom:** "Kernel hangs after 'Starting Scheduler...'"
- **Cause:** The `timer_isr` is not firing, or `enable_interrupts()` in `start()` failed.
- **Fix:** Verify `arch/x86_64/idt.S` is correctly mapped and `sti` was executed.

**Symptom:** "General Protection Fault (GPF) on Switch"
- **Cause:** Stack alignment violation. The `stack_init` in `scheduler.salt` must produce a 16-byte aligned stack pointer *after* the return address is pushed.
- **Check:** `let stack_top = stack_base - (slot as u64 * 0x8000);` Ensure `0x8000` stride prevents overlap.
