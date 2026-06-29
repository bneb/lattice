# KeuOS System ABI Specification (v0.3.0)

> **Status: Level 0, Experimental**
> This document defines the Application Binary Interface (ABI) for the KeuOS KeuOS Microkernel. It is the definitive reference for any language runtime (Rust, C, Zig, Go) targeting KeuOS as a compilation target.

KeuOS strictly diverges from the POSIX standard. The kernel acts exclusively as a **Control Plane** for resource multiplexing and spatial scheduling. All data movement occurs in the **Data Plane** via zero-copy shared memory or fast-path register injection. There are no generic `read` or `write` file descriptor operations.

---

## 1. The Calling Convention

KeuOS utilizes a hardware-optimized calling convention. System calls are executed via native hardware trap instructions. To support heterogeneous architectures without runtime overhead, the ABI defines strict register mappings for both invocation and fast-path return injection.

### x86_64 Architecture

| Property | Value |
|----------|-------|
| **Instruction** | `syscall` |
| **Vector Register** | `rax` |
| **Arguments 1–6** | `rdi`, `rsi`, `rdx`, `rcx`, `r8`, `r9` |
| **Return Value** | `rax` |
| **Fast-Path Injection** | Kernel writes payload into receiver's `rsi` and `rdx` prior to `SYSRET` |

### ARM64 (aarch64) Architecture

| Property | Value |
|----------|-------|
| **Instruction** | `svc #0` |
| **Vector Register** | `x8` |
| **Arguments 1–6** | `x0`, `x1`, `x2`, `x3`, `x4`, `x5` |
| **Return Value** | `x0` |
| **Fast-Path Injection** | Kernel writes payload into receiver's `x1` and `x2` prior to `ERET` |

> [!IMPORTANT]
> The HAL Router (`kernel/arch/mod.salt`) ensures the kernel core logic is architecture-blind. The correct register mappings are resolved at compile time via `#[cfg(target_arch)]` dispatch.

---

## 2. The Control Plane Vectors

The KeuOS kernel exposes exactly **five** core system call vectors. The ABI explicitly forbids generic read/write vectors.

### `SYS_EXIT` — Vector 0

Triggers the **Hardware-Fenced Reclaim** protocol. The kernel executes a strict 4-phase hardware-fenced teardown:

1. **SPSC Ring Drain** — Software fence; clears pending IPC descriptors.
2. **NIC DMA Fence** — Hardware fence; halts all DMA writes to this process's pages.
3. **MMU Flat Sweep** — Unmaps all pages via flat bitmap scan (no recursive page walks). Bulk TLB shootdown.
4. **PMM Bulk Free** — Returns all physical frames to the per-core Treiber stack.

| Parameter | Register | Type | Description |
|-----------|----------|------|-------------|
| `exit_code` | arg1 | `i32` | Process exit code |
| **Returns** | — | — | Never returns |

---

### `SYS_CAP_BIND` — Vector 10

Requests access to a hardware resource or userspace service.

| Parameter | Register | Type | Description |
|-----------|----------|------|-------------|
| `capability_hash` | arg1 | `u64` | 64-bit hashed identifier for the requested resource |
| **Returns** | `rax`/`x0` | `u64` | Capability handle on success, `0` on permission failure |

---

### `SYS_RING_MAP` — Vector 11

Upgrades a capability handle into an active memory mapping. The kernel allocates physical frames from the per-core sharded PMM and maps them into the caller's virtual address space.

| Parameter | Register | Type | Description |
|-----------|----------|------|-------------|
| `capability_handle` | arg1 | `u64` | A previously bound capability |
| **Returns** | `rax`/`x0` | `u64` | Page-aligned virtual address of the `SpscRing`, or `0` on failure |

---

### `SYS_CORE_ACQUIRE` — Vector 12

Requests **spatial scheduling**. The kernel masks the local APIC timer (x86) or Generic Timer (ARM), evicts all other threads, and permanently yields the target CPU core to the caller for tickless execution.

| Parameter | Register | Type | Description |
|-----------|----------|------|-------------|
| `target_core_id` | arg1 | `u32` | Logical CPU core ID to acquire |
| **Returns** | `rax`/`x0` | `u64` | `1` on success, `0` on failure |

> [!WARNING]
> Once acquired, the caller owns the core until process exit. The kernel will not preempt the process on that core.

---

### `SYS_IPC_REG_SEND` — Vector 13

Executes the **fast-path register IPC**. The kernel reads the payload words and writes them directly into the saved execution frame of the target process (via `arch::cpu::inject_ipc_payload`), then wakes the target via the O(1) bitmap scheduler.

| Parameter | Register | Type | Description |
|-----------|----------|------|-------------|
| `target_cap` | arg1 | `u64` | Capability handle identifying the receiver |
| `payload_1` | arg2 | `u64` | First payload word |
| `payload_2` | arg3 | `u64` | Second payload word |
| **Returns** | `rax`/`x0` | `i64` | `0` success, `-1` invalid cap, `-2` target not found |

> [!NOTE]
> The receiver resumes execution with `payload_1` and `payload_2` already loaded in its argument registers (`rsi`/`rdx` on x86_64, `x1`/`x2` on ARM64). No memory is touched.

---

## 3. The Data Plane Memory Layout

Bulk data transfer operates over zero-copy **Single-Producer, Single-Consumer (SPSC)** rings. Any language targeting KeuOS **must exactly replicate this memory layout** in its standard library.

The ABI strictly enforces **64-byte cache line alignment** for the `head` and `tail` pointers to prevent MESI false-sharing between CPU cores.

### Canonical C Definition (`keuos_abi.h`)

```c
#include <stdint.h>

#define KEUOS_CACHE_LINE 64

typedef struct {
    _Atomic uint32_t head __attribute__((aligned(KEUOS_CACHE_LINE)));
    uint32_t capacity;
    _Atomic uint32_t tail __attribute__((aligned(KEUOS_CACHE_LINE)));
    uint8_t* data_ptr;
} keuos_spsc_ring_t;
```

### Canonical Salt Definition

```salt
struct SpscRing {
    @align(64)
    head: u32,          // Cache line 0 — Producer-owned

    capacity: u32,

    @align(64)
    tail: u32,          // Cache line 1 — Consumer-owned

    data_ptr: Ptr<u8>,  // Raw data buffer follows the header
}
// Z3 PROVED: head at offset 0, tail at offset 64 (z3_align_verified)
```

### Memory Map

```
Offset   Size   Field         Owner        Cache Line
──────   ────   ─────         ─────        ──────────
0x00     4B     head          Producer     Line 0
0x04     4B     capacity      Immutable    Line 0
0x40     4B     tail          Consumer     Line 1
0x48     8B     data_ptr      Immutable    Line 1
0x80+    N      data[]        Shared       Line 2+
```

---

## 4. The Formal Verification Contract

KeuOS operates on a **proof-carrying architecture**. While standard C or Rust binaries can target this ABI and manage their own memory safety, binaries compiled via the Salt toolchain are bound by a Z3 formal verification contract.

When interacting with the `SpscRing` via the Salt standard library, the compiler proves the following obligations at compile time:

| Obligation | Z3 Proof |
|------------|----------|
| **Bounds** | `head` offset will never exceed `capacity` |
| **Overwrite Protection** | `(head + len) % capacity != tail` is guaranteed prior to any memory copy |
| **Linearity** | Pointers injected into the ring are marked `consume` and immediately invalidated in the caller's scope (prevents use-after-free) |

> [!TIP]
> If a developer writes a C or Rust program targeting KeuOS, they are responsible for their own memory bounds. If they write a Salt program, the ABI **guarantees** that no pointer will ever exceed capacity and the head will never overwrite an unread tail.

---

## 5. Vector Summary

| Vector | Name | Args | Description |
|--------|------|------|-------------|
| 0 | `SYS_EXIT` | 1 | 4-phase Hardware-Fenced Reclaim teardown |
| 10 | `SYS_CAP_BIND` | 1 | Allocate a capability handle |
| 11 | `SYS_RING_MAP` | 1 | Map SPSC ring to virtual address |
| 12 | `SYS_CORE_ACQUIRE` | 1 | Spatial core acquisition (tickless) |
| 13 | `SYS_IPC_REG_SEND` | 3 | Fast-path register IPC (2× u64 payload) |

---

*KeuOS System ABI v0.3.0 — March 2026*
