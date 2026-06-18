# ADR 010: Hardware Abstraction Layer via Compile-Time Dispatch

**Status:** Accepted
**Date:** 2026-03 (retroactively documented 2026-06)
**Deciders:** KeuOS kernel architecture

## Context

KeuOS targets both x86_64 (QEMU/KVM, cloud instances) and aarch64 (Apple Silicon, AWS Graviton). The kernel core (scheduler, IPC, memory management) must remain architecture-agnostic to avoid forking the codebase. Traditional kernels use `#ifdef` preprocessor conditionals or runtime dispatch tables to handle architecture differences. Both approaches have downsides: `#ifdef` creates spaghetti, runtime dispatch adds overhead to hot paths.

## Decision

**Compile-time HAL router using Salt's `#[cfg(target_arch)]` attribute.** Architecture-specific implementations live in `kernel/arch/x86_64/` and `kernel/arch/aarch64/`. A single router module (`kernel/arch/mod.salt`) uses `#[cfg]` to select the correct implementation at compile time:

```salt
#[cfg(target_arch = "x86_64")]
use kernel::arch::x86_64 as impl;

#[cfg(target_arch = "aarch64")]
use kernel::arch::aarch64 as impl;
```

The router exposes three abstractions: `cpu` (GPR context, register ops), `mmu` (page tables, TLB), and `timer` (PIT/GIC timers). All kernel core code imports `kernel::arch` and uses `arch::cpu::context_switch()`, never `arch::x86_64::context_switch()`.

The HAL Mandate enforces this: `kernel/core/`, `kernel/mem/`, and `kernel/sched/` are **strictly forbidden** from importing `kernel::arch::x86_64` directly.

## Consequences

- **Positive**: Zero runtime dispatch overhead — architecture selection happens at compile time
- **Positive**: The HAL Mandate makes architecture violations visible in code review (any import of `arch::x86_64` in core/ is a reject)
- **Positive**: Adding RISC-V support requires implementing the three HAL traits, not modifying any core code
- **Negative**: The aarch64 port is structurally present (vector table, GICv3 driver, context frame) but not yet bootable — FIQ, SError, and AArch32 vectors are infinite-loop stubs
- **Negative**: Testing requires per-architecture QEMU instances (no cross-architecture simulation)
- **Negative**: Compile-time dispatch means separate kernel binaries per architecture (not a multi-arch binary)
