# ADR 005: Chase-Lev Work-Stealing Scheduler

**Status:** Accepted
**Date:** 2026-02 (retroactively documented 2026-06)
**Deciders:** KeuOS kernel architecture

## Context

KeuOS targets 16-core SMP systems. A single global run queue would create contention (all cores competing for the same lock/atomic) and cache-line ping-pong. Work-stealing schedulers (where each core has a local queue and steals from others when idle) are the established approach for user-space runtimes (Go, Tokio), but kernel adoption is rare. KeuOS needed a scheduler that minimizes cross-core synchronization.

## Decision

**Per-core Chase-Lev work-stealing deques with O(1) bitmap dispatch.** Each core owns a Chase-Lev deque (`DEQUE_BUFFERS[16][1024]`) for its local fiber pool. The owner core pushes/pops from the "bottom" (no atomic ops needed in steady state). Idle cores "steal" from the "top" of remote deques using atomic CAS.

The scheduler uses a hierarchical two-level bitmap with the hardware `TZCNT` (Trailing Zero Count) instruction to find the next runnable fiber in O(1). The bitmap eliminates linear queue scanning entirely.

## Consequences

- **Positive**: Zero cross-core contention in steady state — a core only touches its own deque
- **Positive**: O(1) dispatch via hardware `TZCNT` — no linear scanning of fiber tables
- **Positive**: Universal Task Pointer dispatch enables 3-instruction indirect calls (`invoke_task`)
- **Negative**: Chase-Lev deques are bounded (1024 entries per core); overflow requires fallback to a global overflow queue (not yet implemented)
- **Negative**: Memory ordering for cross-core stealing is noted as needing acquire/release hardening for production SMP
- **Negative**: The static `DEQUE_BUFFERS[16][1024]` allocation is 64KB per core and cannot grow at runtime
