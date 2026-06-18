# ADR 011: Per-Core Sharded Lock-Free Physical Memory Manager

**Status:** Accepted
**Date:** 2026-02 (retroactively documented 2026-06)
**Deciders:** KeuOS kernel architecture

## Context

In an SMP kernel, a global physical page allocator protected by a single lock becomes a contention point as core count scales. Under load, cores spin waiting for the allocator lock, destroying the benefit of parallelism. Lock-free algorithms exist (Treiber stacks, Michael-Scott queues) but a single shared lock-free structure still suffers from cache-line ping-pong as cores CAS on the same head pointer.

## Decision

**Per-core sharded Treiber stacks with cross-core stealing.** Each core maintains its own free-page stack using a lock-free Treiber stack (LIFO, CAS on head pointer). Core-local allocations and deallocations touch only the local stack — no atomics needed in the common path (the core is the only producer/consumer of its own stack).

If a core exhausts its local free list, it uses `cmpxchg16b` (128-bit CAS with ABA counter) to steal a batch of pages from a remote core's stack. The ABA counter prevents the classic ABA problem where a pop sees the same head pointer value but the stack state has changed.

Each stack is padded to 64-byte cache lines (`@align(64)`) to prevent false sharing. Interrupts are masked during local stack mutations to prevent preemption mid-CAS.

## Consequences

- **Positive**: Zero atomic operations in the common path (local alloc/free) — only non-atomic loads and stores
- **Positive**: Cross-core stealing with ABA-protected CAS eliminates the need for a global fallback allocator
- **Positive**: Cache-line isolation prevents MESI invalidation storms between cores
- **Negative**: Stealing is work-stealing in reverse — a starving core must probe remote cores sequentially (O(N) worst case)
- **Negative**: ABA-protected CAS requires `cmpxchg16b`, which is x86_64-specific; aarch64 port needs `LDXP`/`STXP` paired loads
- **Negative**: Per-core sharding means memory is not truly global — a core may hold free pages while another core is page-starved
