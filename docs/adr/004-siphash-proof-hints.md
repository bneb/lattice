# ADR 004: SipHash-2-4 Proof Hints for IPC Descriptor Validation

**Status:** Accepted
**Date:** 2026-01 (retroactively documented 2026-06)
**Deciders:** KeuOS kernel architecture

## Context

KeuOS uses SPSC shared memory rings for zero-copy IPC between Ring 3 processes and kernel daemons. A compromised Ring 3 process could attempt to forge or corrupt SPSC descriptors to gain unauthorized access to kernel memory or other processes' data. The kernel needs an O(1) validation mechanism that cannot be forged by userspace.

## Decision

**Use SipHash-2-4 to generate 64-bit proof hints embedded in every SPSC descriptor.** The proof hint is computed at compile time by hashing the struct identity, field offset, and alignment of the descriptor. The kernel's IPC arbiter validates the proof hint in O(1) before touching any shared memory.

This is layered with two additional hardware checks:
1. **Alignment gate**: The arbiter verifies `(ptr & 0x3F) == 0` — the pointer must be 64-byte aligned
2. **MMU gate**: Ring 3 cannot access Ring 0 memory under any circumstances (hardware-enforced)

## Consequences

- **Positive**: O(1) validation with no tree walk, no lookup table, no lock acquisition
- **Positive**: SipHash-2-4 is cryptographically strong against forgery while being extremely fast (~140 cycles on x86_64)
- **Positive**: Defense in depth — even if an attacker steals a valid proof hint, they must also satisfy the alignment and MMU gates
- **Negative**: Proof hints must be regenerated if struct layouts change (compiler-managed, but a rebuild is required)
- **Negative**: Adds 8 bytes of overhead per descriptor
