# KeuOS: Formal Microkernel Architecture

## Overview

KeuOS is a microkernel operating system written entirely in [Salt](salt-front/), a systems language with an embedded Z3 theorem prover. The architecture achieves unikernel-level latency (~150 cycles per packet) while maintaining hardware-enforced Ring 0 / Ring 3 isolation.

The key insight is that Salt's compile-time formal verification eliminates the need for runtime safety checks. This means KeuOS can move performance-critical subsystems (networking, storage) into Ring 3 user space without the "Security Tax" that normally makes microkernels slower than monolithic kernels.

## The Formal Shadow Model

```
                    ┌─────────────────────────────────────┐
                    │     Compile Time (Salt Compiler)     │
                    │                                      │
                    │  Z3 Theorem Prover                   │
                    │    ├── @align(64) → cache-line proof  │
                    │    ├── requires/ensures → contracts   │
                    │    └── ArenaVerifier → escape safety   │
                    │                                      │
                    │  Proof-Hint Generator                 │
                    │    └── hash_combine → 64-bit seal     │
                    │                                      │
                    │  MLIR Codegen                         │
                    │    └── salt.proof_hints attribute     │
                    └──────────────┬──────────────────────┘
                                   │ Binary with embedded proofs
                    ┌──────────────▼──────────────────────┐
                    │       Runtime (KeuOS Kernel)       │
                    │                                      │
                    │  Ring 0: PMM, Scheduler, IPC, VirtIO │
                    │  Ring 3: NetD, KeuOSStore, Apps    │
                    │                                      │
                    │  Arbiter validates proof_hint in O(1) │
                    └─────────────────────────────────────┘
```

## Kernel Components

### Ring 0 (Minimal Trusted Computing Base)

| Component | Location | Role |
|-----------|----------|------|
| **PMM** | `kernel/core/pmm.salt` | Physical page allocator (buddy system) |
| **Scheduler** | `kernel/core/scheduler.salt` | 16-core SMP, preemptive, Chase-Lev work-stealing |
| **Syscalls** | `kernel/core/syscall.salt` | `sys_shm_grant`, `sys_ipc_send`, `sys_yield` |
| **IPC** | `kernel/lib/ipc_shm.salt` | SPSC ring buffer (raw offset API) |
| **IPC Ring** | `kernel/lib/ipc_ring.salt` | `SpscRing` struct with `@align(64)` + `SpscDescriptor` |
| **Arbiter** | `kernel/lib/ipc_arbiter.salt` | O(1) SipHash-2-4 proof-hint validation |
| **VirtIO** | `kernel/drivers/virtio.salt` | NIC and block device (DMA) |
| **NetD Bridge** | `kernel/net/netd_bridge.salt` | VirtIO RX → SPSC ring pump |
| **SMP** | `kernel/arch/x86/smp.salt` | AP bootstrap, per-CPU state |
| **Process Reclaim** | `kernel/core/keuos_reclaim.salt` | 5-phase hardware-fenced process teardown |
| **Chase-Lev Deque** | `kernel/sched/chase_lev.salt` | Per-core lock-free work-stealing deque, static `DEQUE_BUFFERS[16][1024]` |
| **Epoch-Based Reclamation** | `kernel/lib/ebr_arena.salt` | Zero-pause concurrent memory compaction via `ebr_enter_epoch`/`exit_epoch` |
| **SIR Boundary** | `salt-front/src/codegen/sir/` | Versioned AST extraction: types, contracts, spans → SirModule JSON/Flatbuffer |
| **Page Sweep** | `kernel/mem/page_sweep.salt` | Non-recursive PML4 page table teardown |
| **Reclaim Histogram** | `kernel/core/reclaim_histogram.salt` | P99 reclamation telemetry (1024-entry) |

### Ring 3 (System Daemons)

| Daemon | Role | Communication |
|--------|------|---------------|
| **NetD** | Full TCP/IP stack: ARP, IP, TCP (SYN cookie–hardened) | SPSC ring from kernel bridge |
| **KeuOSStore** | Block storage via VMO | SPSC ring (planned v0.9.2) |
| **User Apps** | Application processes | Socket API via NetD |

## Data Plane: Zero-Trap SPSC

### Traditional Microkernel IPC

```
App calls write() → trap to Ring 0 → kernel copies data → schedules receiver
→ receiver calls read() → trap to Ring 0 → kernel copies data → return
Total: ~2000 cycles, 2 context switches, 2 copies
```

### KeuOS SPSC IPC

```
Producer writes to SPSC ring (shared memory) → Consumer reads from same page
Optional: sys_ipc_send wake notification (1 trap, only if consumer is sleeping)
Total: ~150 cycles, 0-1 context switches, 0 copies
```

### Memory Layout

The SPSC ring occupies a single 4KB page granted via `sys_shm_grant`:

```
Offset   Size   Field           Cache Line   Z3 Proof
──────   ────   ─────           ──────────   ────────
0x000    8B     head (u64)      Line 0       z3_align_verified @ 0
0x008    8B     capacity (u64)  Line 0       (same line as head)
0x040    8B     tail (u64)      Line 1       z3_align_verified @ 64
0x048    8B     consumer_wait   Line 1       @align(64) signal flag
0x080    3904B  data[]          Lines 2-63   (ring buffer payload)
```

The `@align(64)` attribute on `head` and `tail` is proven correct by Z3 at compile time:
- `(base + 0) % 64 == 0` → head is on cache line 0
- `(base + 64) % 64 == 0` → tail is on cache line 1

This prevents **false sharing**: the producer core's writes to `head` never invalidate the consumer core's L1 cache line for `tail`.

## Formal Shadow: Proof-Carrying IPC

### The Problem

A microkernel's security model depends on Ring 3 processes being unable to corrupt each other or the kernel. But shared memory IPC means the kernel must trust that descriptors point to valid, properly-aligned memory.

### The Solution: Seal and Verify

**At compile time**, the Salt compiler:
1. Proves `@align(64)` constraints via Z3 (proof by contradiction)
2. Generates a `proof_hint = hash_combine(struct_id, offset, align)` — a 64-bit SipHash-2-4 keyed hash
3. Embeds the hint in the MLIR output as `salt.proof_hints` module attribute
4. The hint survives lowering through LLVM and becomes a constant in the binary

**At runtime**, the NetD arbiter validates every descriptor:
```salt
// O(1) validation — fewer than 12 CPU cycles
pub fn validate_descriptor_fast(ptr: u64, hint: u64, authorized: u64) -> u64 {
    if (ptr & 0x3F) != 0 { return 0; }  // Gate 1: alignment
    if hint != authorized  { return 0; }  // Gate 2: proof match
    return 1;
}
```

### Attack Vectors Defended

| Vector | Attack | Defense |
|--------|--------|---------|
| **A: Alignment Subversion** | Pass unaligned ptr with valid hint | Gate 1: `(ptr & 0x3F) != 0` check |
| **B: Hint Forgery** | Guess or steal a proof_hint | Gate 2: hint must match compile-time seal |
| **C: Kernel Memory Access** | Point descriptor at Ring 0 memory | MMU page tables (hardware enforced) |

## Build & Boot

```bash
# Build kernel
cd kernel && make

# Boot in QEMU (4-core SMP, VirtIO networking)
python3 tools/runner_qemu.py kernel/build/keuos.elf

# Run benchmarks
./benchmarks/benchmark.sh -a
```

## Version History

| Version | Codename | Achievement |
|---------|----------|-------------|
| v0.9.0 | *Core Networking* | Ring 3 NetD, zero-trap sockets, SPSC IPC |
| v0.9.1 | *Core Foundation* | `@align(64)` cache-line isolation, proof-carrying IPC, SipHash-2-4 hardening, lattice reclaim, chaos testing |
| v0.9.2 | *Postcondition Pivot* | Z3-backed `ensures` for pure functions — path-sensitive WP verification, implicit guard negation, incompleteness gate (6/6 GREEN) |
| v0.9.3 | *Core Authority* | Chase-Lev work-stealing SMP, stateless SYN cookie hardening (SipHash-2-4), Epoch-Based Reclamation, SIR boundary decoupling (194/194 tests) |
| v1.0.0 | *Kernel Architecture* | Salt LSP v0.2.0 (zero-I/O, Z3 hover, Go-to-Definition), full 16-core SMP scale-out, adversarial NetD, 32/32 LSP tests |
| v1.1.0 | *Loop Validation* | `invariant` keyword, Z3 induction-based verification, havoc semantics (Completed) |
| v1.2.0 | *Persistence Pillar* | PCIe ECAM enumeration, NVMe MMIO CAP/VS extraction (Completed), Block-VMO storage, NVMe SPSC bridge (planned) |
