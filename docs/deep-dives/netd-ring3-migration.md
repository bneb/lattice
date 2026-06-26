# NetD Ring 3 Migration — Design & Implementation Plan

**Target:** v1.0.0

## Summary

NetD currently runs as a Ring 0 kernel thread spawned via `exec_spawn_kernel_thread`.
This document describes the migration to a proper Ring 3 process communicating
over zero-trap SPSC shared memory rings — fulfilling the KeuOS microkernel
architecture's central promise.

## Current vs Target Architecture

```
CURRENT (Ring 0 NetD):                   TARGET (Ring 3 NetD):
                                         
  VirtIO ISR                               VirtIO ISR
     │                                        │
     ▼                                        ▼
  virtio_net_isr() [IF=0]                 virtio_net_isr() [IF=0]
     │                                        │
     ▼                                        ▼
  net_poll()                              netcore_poll_all()
     │                                        │
     ▼                                        ▼
  netcore_poll_all()                      netd_bridge.push_frame()
     │                                        │
     ▼                                        ▼
  [inline kernel stack]                   sys_ipc_send(NETD_PID, CMD_RX)
     │                                        │
     ▼                                        ▼
  NetD kernel thread                      [context switch to Ring 3]
  (direct fn call, no switch)                │
     │                                        ▼
     ▼                                     NetD Ring 3 process
  daemon.process_frame()                  daemon.process_frame()
     │                                        │
     ▼                                        ▼
  salt_yield_check()                      syscall.ipc_recv() [block]
  (calls do_dispatch inline)              
```

## Key Design Decisions

### 1. Spawn Mechanism

Replace `exec_spawn_kernel_thread` with `exec_spawn_process` for NetD.
NetD is compiled as a separate ELF binary (`netd.elf`), loaded from the
filesystem or embedded in the kernel image, and spawned as a proper Ring 3
process with its own PML4 and user address space.

### 2. Data Plane: SPSC Rings via sys_shm_grant

The RX bridge ring (one physical page) is shared between kernel and NetD:
  - Kernel: maps via phys_to_virt (higher-half)
  - NetD: maps via sys_shm_grant into its user address space at 0x600000000000

The ring format is length-prefixed Ethernet frames:
  `[2 bytes: frame_len (LE)] [frame_len bytes: raw Ethernet]`

### 3. Control Plane: IPC Wakeup

When the kernel pushes a frame to the RX ring, it sends `sys_ipc_send` to NetD
with command `CMD_RX_NOTIFY (1)`. This wakes NetD from `sys_ipc_recv` blocking.

### 4. TX Path

NetD pushes TX frames into a shared TX ring, then calls `sched_yield` (syscall 119)
which triggers the I/O Pump: drains the TX ring, notifies VirtIO TX queue, polls RX.

### 5. Interrupt Routing

VirtIO ISR remains in Ring 0 (hardware interrupts cannot be delivered to Ring 3
directly). The ISR runs `netcore_poll_all()` → `netd_bridge.push_frame()` →
`sys_ipc_send(NETD_PID, CMD_RX_NOTIFY)` → context switch to NetD.

## Changes Required

| # | File | Change |
|---|------|--------|
| 1 | `kernel/core/main.salt` | Replace kernel-thread spawn with process spawn for NetD |
| 2 | `kernel/net/netcore.salt` | Wire TCP dispatch and RX notification to NetD bridge |
| 3 | `user/netd/main.salt` | Replace salt_yield_check loop with IPC-receive loop |
| 4 | `kernel/net/netd_bridge.salt` | Add sys_shm_grant setup for Ring 3 access |
| 5 | `docs/abi/KEUOS_ABI_STABLE.md` | Document IPC command codes for NetD |

## IPC Command Codes

| Command | Value | Direction | Purpose |
|---------|-------|-----------|---------|
| CMD_RX_NOTIFY | 1 | Kernel → NetD | Frame(s) available in RX ring |
| CMD_TX_NOTIFY | 2 | NetD → Kernel | Frame(s) available in TX ring |
| CMD_BIND | 3 | App → NetD | Bind a port |
| CMD_ACCEPT | 4 | App → NetD | Accept connection |
| CMD_CLOSE | 5 | App → NetD | Close socket |

## Verification

After migration:
1. `tools/runner_qemu.py` boots to kernel prompt
2. NetD appears as a Ring 3 process (state=RUNNING, user_pml4 != 0)
3. UDP echo on port 7 works (packet flows: VirtIO → kernel → SPSC ring → NetD → SPSC ring → kernel → VirtIO)
4. NetD crash does not panic the kernel (isolated by MMU)
