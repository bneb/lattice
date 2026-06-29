# KeuOS Stable System ABI — v1.0.0

**Status:** Proposed for v1.0.0 freeze
**Last Updated:** 2026-06-18

## ABI Stability Policy

Once v1.0.0 is released, the syscall interface documented here is **frozen**. Changes require:

1. A new syscall number (never reuse old numbers)
2. Backward-compatible struct layout (add fields at end, never remove/reorder)
3. Deprecation notice for removed functionality (one full release cycle)
4. Synchronized updates to kernel, userspace wrappers, and this document

Before v1.0.0, this document reflects the **current experimental ABI** and may change.

---

## Syscall Convention

| Register | Purpose |
|----------|---------|
| RAX | Syscall number (input), return value (output) |
| RDI | Argument 0 (arg0) |
| RSI | Argument 1 (arg1) |
| RDX | Argument 2 (arg2) |
| R10 | Argument 3 (arg3) — fast path only |
| SYSCALL | Entry from Ring 3 |
| SYSRET | Return to Ring 3 |

Return values:
- `0` = success (unless returning data)
- Positive value = data (bytes read, PID, handle)
- `-1` (`0xFFFFFFFFFFFFFFFF`) = ENOSYS (not implemented) or error

---

## Syscall Table (Frozen for v1.0.0)

### I/O Syscalls

| Number | Name | Signature | Description |
|--------|------|-----------|-------------|
| **0** | `sys_read` | `(fd: u64, buf: u64, len: u64) -> u64` | Read from file descriptor. Returns bytes read or 0 on error. |
| **1** | `sys_write` | `(fd: u64, buf: u64, len: u64) -> u64` | Write to file descriptor. fd=1 is serial stdout. Returns bytes written. |
| **2** | `sys_open` | `(path: u64, flags: u64) -> u64` | Open a file. Returns fd number or 0 on error. |

### Memory Syscalls

| Number | Name | Signature | Description |
|--------|------|-----------|-------------|
| **9** | `sys_mmap` | `(addr: u64, len: u64) -> u64` | Map anonymous memory. Returns virtual address or 0. |
| **12** | `sys_brk` | `(addr: u64) -> u64` | Set program break. Returns new brk or 0. |

### Capability & IPC Syscalls

| Number | Name | Signature | Description |
|--------|------|-----------|-------------|
| **10** | `sys_cap_bind` | `(flags: u64) -> u64` | Allocate a capability handle. Returns capability ID. |
| **11** | `sys_ring_map` | `(cap_id: u64) -> u64` | Map SPSC ring buffer pages into calling process. Requires CAP_FLAG_RING_ABI. |
| **12** | `sys_core_acquire` | `(target_core_id: u64) -> u64` | Request spatial ownership of a CPU core. Returns 0 on success. |
| **14** | `sys_ipc_reg_send` | — | Fast-path register IPC (currently ENOSYS). |
| **15** | `sys_ipc_await` | — | Await fast-path IPC message (currently ENOSYS). |
| **200** | `sys_ipc_send` | `(target_pid: u64, msg0: u64, msg1: u64, msg2: u64) -> u64` | Send IPC message to target process. |
| **201** | `sys_ipc_recv` | `() -> u64` | Block until IPC message arrives. Returns message. |
| **202** | `sys_shm_grant` | `(target_pid: u64, src_vaddr: u64, dst_vaddr: u64, num_pages: u64) -> u64` | Map shared memory pages into target process. |

### Process Syscalls

| Number | Name | Signature | Description |
|--------|------|-----------|-------------|
| **60** | `sys_exit` | `(code: u64) -> void` | Exit current process. Handled in assembly fast path. |
| **119** | `sched_yield` | `() -> u64` | Yield CPU. Triggers I/O pump (RX poll, TX flush, VirtIO notify). |
| **400** | `sys_spawn` | `(path_ptr: u64, path_len: u64) -> u64` | Spawn a new process from ELF. Returns PID. |
| **401** | `sys_wait` | `(target_pid: u64) -> u64` | Wait for process to exit. Returns exit code. |

### System Control

| Number | Name | Signature | Description |
|--------|------|-----------|-------------|
| **99** | `sys_shutdown` | `() -> void` | ACPI power-off. |

### Reserved / Deprecated

| Number | Status | Notes |
|--------|--------|-------|
| **3–8** | Reserved | Available for future I/O syscalls |
| **13** | Reserved | Dispatched via arg0 in slow path |
| **16–98** | Reserved | Available for future syscalls |
| **100–118** | Reserved | Available for future scheduling syscalls |
| **120–127** | Reserved | Available for future syscalls |
| **128–131** | Deprecated | `sys_reactor_*` — replaced by ipc_await (14/15) |
| **132–199** | Reserved | Available for future syscalls |
| **203–299** | Reserved | Available for future IPC syscalls |
| **300–399** | Reserved | Available for future syscalls |
| **402–499** | Reserved | Available for future process syscalls |
| **500+** | Reserved | Future extensions |

---

## Capability System

### Capability Flags

```
CAP_FLAG_NONE     = 0  — No special permissions
CAP_FLAG_PRODUCER = 1  — Ring producer (may write head)
CAP_FLAG_CONSUMER = 2  — Ring consumer (may write tail)
```

### Capability States

```
CAP_STATE_FREE   = 0  — Unused slot
CAP_STATE_BOUND  = 1  — Allocated but not yet mapped
CAP_STATE_MAPPED = 2  — Active, pages mapped into process
```

### Capability Table

- Size: 64 entries (MAX_CAPABILITIES)
- Each capability tracks: state, owner PID, ring physical address, ring virtual address, flags

### VADDR Layout

Ring virtual addresses are allocated dynamically from the process's `mmap_base` bump allocator. There is no fixed base address.

---

## SPSC Ring Layout

Each ring occupies one 4KB page with cache-line isolation:

```
Offset  | Size | Field           | Cache Line
--------|------|-----------------|-----------
0x00    | 8    | head (u64)      | Line 0 (bytes 0-63)
0x08    | 8    | capacity (u64)  | 
0x40    | 8    | tail (u64)      | Line 1 (bytes 64-127)
0x80    | 8    | consumer_waiting | Line 2 (bytes 128-191)
0xC0    | ...  | data region     | Lines 3+ (3904 bytes usable)
```

Ring fields use `@align(64)` for cache-line isolation. SipHash-2-4 proof hints are validated at runtime by the IPC descriptor system.

---

## Error Codes

| Value | Name | Description |
|-------|------|-------------|
| `0` | Success | Operation completed |
| `0 - 1` (`-1`) | ENOSYS / EINVAL | Not implemented, invalid argument, or operation failed |

Extended error codes are planned for v1.1.0.

---

## Userspace Wrappers

Canonical userspace syscall wrappers live in `user/lib/syscall.salt`:

```salt
// User-space syscall wrappers
pub fn write(fd: u64, buf: u64, len: u64) -> u64 {
    // syscall 1 via inline assembly
}
pub fn exit(code: u64) {
    // syscall 60
}
pub fn mmap(addr: u64, len: u64) -> u64 {
    // syscall 9
}
```

---

## Changelog

| Date | Change |
|------|--------|
| 2026-06-18 | Document created. Syscall numbers frozen at current values. |
| 2026-03 | SYS_REACTOR_* (128-131) deprecated in favor of ipc_await. |
| 2026-01 | SYS_CAP_BIND (10), SYS_RING_MAP (11), SYS_CORE_ACQUIRE (12) added. |

---

**See also:** [KEUOS_ABI.md](KEUOS_ABI.md) — Architecture overview and design rationale.
