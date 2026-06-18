# ADR 007: Universal Task Pointer Dispatch

**Status:** Accepted
**Date:** 2026-02 (retroactively documented 2026-06)
**Deciders:** KeuOS kernel architecture

## Context

The scheduler must dispatch heterogenous task types: kernel threads, Ring 3 processes, async fibers, preemptive fibers, and SIP (Software-Isolated Process) instances. Each task type has a different entry point signature, stack layout, and context frame. A traditional switch-case dispatch over task type would add branching overhead to the hottest path in the kernel.

## Decision

**Universal Task Pointer (UTP): a single `invoke_task` function pointer stored in every Task Control Block.** All task types populate the same function pointer field. Dispatch is a 3-instruction sequence:

```asm
mov  rax, [rdi + TCB.utp_offset]   ; load task pointer
call rax                             ; indirect call
```

No type tag, no switch, no branch. Every task type provides its own entry point that handles its specific ABI (Ring 0 vs Ring 3, fiber vs process, async vs preemptive). The scheduler is completely agnostic to what it dispatches.

## Consequences

- **Positive**: 3-instruction dispatch — the scheduler hot path has no branches
- **Positive**: Extensible — new task types (SIP, vDSO-style fast paths) add their own UTP without modifying the scheduler
- **Positive**: The compiler can verify that every task type populates `utp` (no null pointer dereference)
- **Negative**: Indirect call prevents branch prediction warmup on first dispatch of a task
- **Negative**: Each task type must manage its own ABI transition (Ring 0→3, FXSAVE/FXRSTOR); errors are per-task-type, not caught centrally
- **Negative**: The TCB struct must be stable (field offsets are baked into assembly stubs)
