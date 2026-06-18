# ADR 013: Proof-Carrying IPC Descriptors

**Status:** Accepted
**Date:** 2026-02 (retroactively documented 2026-06)
**Deciders:** KeuOS kernel architecture

## Context

KeuOS separates the kernel from userspace daemons (NetD, KeuOSFS) via hardware rings, but they still communicate through shared memory. A malicious or buggy Ring 3 process could craft fake IPC descriptors attempting to read kernel memory or other processes' data. Traditional kernels rely on capability systems (seL4) or copying (Mach) to mediate IPC. Capabilities require kernel-managed capability spaces; copying defeats the zero-copy goal.

## Decision

**Proof-carrying descriptors: every IPC descriptor carries a compile-time-generated 64-bit proof hint that the kernel arbiter validates in O(1) before access.** The proof hint is a SipHash-2-4 hash of (struct identity, field offset, alignment). The arbiter verifies three properties:

1. **Proof hint validity**: `siphash(descriptor.identity, descriptor.offset, descriptor.alignment) == descriptor.proof_hint`
2. **Alignment constraint**: `(descriptor.ptr & 0x3F) == 0` — pointer must be 64-byte aligned
3. **Bounds constraint**: `descriptor.ptr + descriptor.size ≤ ring.end` — descriptor must fit within the ring

All three checks execute in O(1) with no memory allocation, no lock acquisition, and no table lookup. A descriptor that fails any check is rejected with the ring marked as compromised.

## Consequences

- **Positive**: Zero-copy IPC with cryptographic integrity — the data is never touched by the kernel
- **Positive**: O(1) validation — constant time regardless of descriptor count or ring size
- **Positive**: The proof hint is generated at compile time from the struct definition; changing the struct invalidates the hint, providing defense against layout confusion
- **Negative**: Proof hints are 64 bits — not formally proven to be collision-resistant (unlike Z3-verified contracts)
- **Negative**: A compromised Ring 3 process with read access to the kernel binary could extract valid proof hints (but cannot satisfy the alignment and MMU gates)
- **Negative**: SipHash adds ~140 cycles of overhead per descriptor validation (acceptable for control-plane ops, measurable on the data path)
