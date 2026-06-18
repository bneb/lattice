# ADR 012: Epoch-Based Reclamation for Lock-Free Memory

**Status:** Accepted
**Date:** 2026-03 (retroactively documented 2026-06)
**Deciders:** KeuOS kernel architecture

## Context

Lock-free data structures (SPSC rings, Treiber stacks) face a reclamation problem: when a thread removes a node from a lock-free structure, it cannot immediately free the memory because another thread may still hold a reference to it. Classic solutions include hazard pointers (per-thread retired lists), reference counting (atomic overhead), and RCU (complex integration with the scheduler). KeuOS needed a reclamation scheme that balances simplicity with performance for kernel use.

## Decision

**Epoch-Based Reclamation (EBR) with three epochs.** The system maintains a global epoch counter and per-thread epoch announcements. Threads announce which epoch they are currently in before accessing lock-free structures. When a thread wants to reclaim a retired object:

1. It places the object on a retirement list for the current epoch
2. It increments the global epoch (mod 3)
3. Objects retired two epochs ago (when all threads have moved past that epoch) are safe to free

The key property: if a thread is in epoch E, no thread can still hold a reference to objects retired in epoch E-2 (because every other thread has moved to at least epoch E-1 since those objects were retired).

## Consequences

- **Positive**: No per-object reference counting overhead — retirement is a list append, reclamation is a bulk sweep
- **Positive**: Simple mental model compared to RCU (no grace periods, no quiescent state tracking through the scheduler)
- **Negative**: Three-epoch window means memory is held for at least two epoch transitions before being freed — memory overhead proportional to allocation rate × epoch duration
- **Negative**: A thread that stalls in an old epoch blocks all reclamation — one misbehaving thread can cause unbounded memory growth
- **Negative**: Epoch transitions require scanning all thread announcements, which is O(N) in thread count
