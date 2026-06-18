# ADR 015: Microkernel Architecture with Ring 3 System Daemons

**Status:** Accepted
**Date:** 2025-12 (retroactively documented 2026-06)
**Deciders:** KeuOS kernel architecture

## Context

Operating system kernels fall on a spectrum from monolithic (Linux: all drivers and services in Ring 0) to microkernel (seL4, MINIX: minimal kernel, services in userspace). Monolithic kernels have better IPC performance (function calls vs. message passing) but larger trusted computing bases. Microkernels have smaller TCBs but historically suffer from IPC overhead (Mach's 2x performance penalty over monolithic Unix).

KeuOS needed to achieve microkernel isolation without the traditional IPC performance penalty.

## Decision

**Microkernel architecture where only memory management (PMM, VMO), scheduling (16-core SMP, preemptive, Chase-Lev), and IPC primitives (`sys_shm_grant`) run in Ring 0.** Everything else — networking (NetD), storage (KeuOSFS), device drivers — runs in Ring 3 as isolated system daemons.

The key insight that makes this viable is **zero-trap SPSC rings for data-plane IPC**. NetD and user programs communicate through shared memory rings mapped via `sys_shm_grant`. After initial setup (a few syscalls for bind/accept), all data transfer happens through direct memory access — no kernel traps, no copies, no locks.

Ring 3 processes are hardware-isolated by the MMU: no Ring 3 code can access Ring 0 memory under any circumstances. Proof-carrying IPC descriptors with SipHash-2-4 validation and alignment checks provide defense-in-depth against descriptor forgery.

## Consequences

- **Positive**: Minimal TCB — the kernel is only scheduling, memory management, and IPC primitives (~15,000 LOC of Salt)
- **Positive**: System daemons can crash and restart without taking down the kernel (NetD crash = lost packets, not kernel panic)
- **Positive**: Z3 contracts at daemon boundaries provide formal guarantees about cross-process data safety
- **Negative**: NetD currently runs in Ring 0 as a kernel thread — the architecture's central promise is not yet fully realized
- **Negative**: SPSC topology limits connectivity — complex service meshes require multiple rings or a ring-broker daemon
- **Negative**: No dynamic service discovery — daemon capabilities are statically configured at boot
