# ADR 006: SPSC Zero-Trap Inter-Process Communication

**Status:** Accepted
**Date:** 2026-01 (retroactively documented 2026-06)
**Deciders:** KeuOS kernel architecture

## Context

Traditional microkernels suffer a performance penalty because moving data between user-space daemons requires a kernel trap (context switch), costing upwards of 1,000 CPU cycles. For KeuOS to be viable as a microkernel, IPC must approach the cost of a function call. POSIX sockets, pipes, and even `io_uring` all involve kernel transitions for data plane operations.

## Decision

**Use Single-Producer Single-Consumer (SPSC) ring buffers in shared memory for data-plane IPC, with zero kernel traps in the steady state.** The kernel maps physical pages into both producer and consumer address spaces via `sys_shm_grant`. Producers and consumers read/write directly to the ring — no kernel transition is needed for data transfer.

The SPSC ring design eliminates three traditional costs:
1. **No trap**: Shared memory access is a regular load/store
2. **No copy**: DMA buffers write directly into the SPSC ring page; the consumer reads from the same physical page
3. **No lock**: Single-producer, single-consumer semantics mean head and tail indices sit on separate 64-byte-aligned cache lines, eliminating cache contention

Control-plane operations (bind, accept, close) still use syscalls, but the data path is entirely in userspace.

## Consequences

- **Positive**: ~150-cycle IPC latency vs. ~1,000+ cycles for traditional kernel-mediated IPC
- **Positive**: Cache-line isolation (`@align(64)`) prevents MESI false sharing between producer and consumer
- **Positive**: Integrates directly with DMA — network packets land in the ring with zero intermediate copies
- **Negative**: SPSC topology is restrictive — each ring connects exactly one producer to one consumer; fan-out requires multiple rings
- **Negative**: Untrusted userspace can write malicious `capacity`/`tail` values; the kernel arbiter must clamp these (currently a known security gap)
- **Negative**: Ring capacity is fixed at creation time; no dynamic resize without re-negotiation
