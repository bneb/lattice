# ECS-Native Sprint Plan — 24 Weeks

## Architecture Principle

KeuOS uses the **DBOS (Database Operating System)** pattern. Everything is an
Entity with Components stored in flat SparseSet arrays. There is no
traditional VFS — no file paths, no inode trees, no directory hierarchies.

- **World**: The in-memory columnar database of all OS state
- **Entity**: 32-bit ID + 32-bit generation counter (generational index)
- **Component**: POD struct stored in a SparseSet column (ThreadState,
  MemoryMap, IpcCapability, OpenSocket, SchedulingPriority, etc.)
- **ECS Scheduler**: Priority-based entity query over dense arrays
- **ECS IPC**: Direct entity-to-entity message passing

The legacy process table (PROC_TABLE), legacy scheduler, and VFS are
transitional infrastructure. Each tier below replaces one legacy subsystem
with its ECS equivalent.

## Current State: What Exists

### ECS (target architecture — partially wired)
| Module | Status |
|--------|--------|
| `world.salt` | World struct + ecs_world_init — working, called during boot |
| `entity.salt` | Generational EntityAllocator, 64K entities, O(1) alloc/free/is-alive |
| `sparse_set.salt` | 8 component sets with delegation to scheduling_sets.salt |
| `components.salt` | ThreadState, SchedulingPriority, MemoryMap, IpcCapability, OpenSocket, CpuAffinity, EpochInfo, PerfCounters |
| `ecs_scheduler.salt` | scheduler_system_tick, yield, block, wakeup — implemented |
| `ecs_bridge.salt` | Spawn thread via ECS, set status, destroy — used by boot_helpers for Terminal/TX_RING |
| `ecs_ipc.salt` | ECS IPC path — defined |
| `scheduling_sets.salt` | SparseSet impl for thread/priority/affinity/epoch/perf sets |

### Legacy (still primary code paths)
| Module | Status |
|--------|--------|
| `process.salt` | PROC_TABLE — primary process registry (16 slots, 0xD8 bytes each) |
| `scheduler.salt` | Legacy scheduler — what actually dispatches processes |
| `syscall.salt` | sys_spawn(path), sys_wait(pid), sys_exit — VFS/proc_table based |
| `syscall_io.salt` | sys_open/sys_read/sys_write — VFS-backed (with serial special-case for fd 0/1/2) |
| `vfs.salt` / `ext2.salt` / `ramfs.salt` | Traditional filesystem — to be replaced by entity components |

### User-facing (works today)
| Capability | Mechanism |
|-----------|-----------|
| Program execution | Embedded ELF → spawn_process → Ring 3 |
| Serial output | syscall.write(fd=1) — serial special-case |
| Serial input | syscall.read(fd=0) — serial RX ring |
| IPC | syscall.ipc_send/recv — register-level fast path |
| Process exit | syscall.exit(code) → process_exit → PROC_ZOMBIE |
| 6-test harness | `python3 tools/runner_qemu.py test` — all passing |

## Tier 0: Foundation (Week 1-2)

**Goal:** Everything we build is verifiable. No untested code paths.

### 0.1 Test harness — ✅ DONE
- 6 tests passing, `make test-userspace` target
- Framework for adding tests in 3 lines of Python

### 0.2 ECS World diagnostics
- Add `ecs_world_dump()` — print entity count, component counts to serial
- Call during boot after ecs_world_init
- Add test assertion for World initialization
- **Deliverable:** Serial log shows "ECS World: N entities, M threads, P memory maps"

### 0.3 ECS spawn test (kernel-internal)
- Allocate entity via ecs_bridge_spawn_thread with a test step function
- Verify thread_set_insert, priority_set_insert succeed
- Verify scheduler_system_tick finds the entity
- **Deliverable:** Kernel-level test that spawns entity, scheduler picks it up

## Tier 1: ECS Process Model (Week 3-6)

**Goal:** Processes are entities. The legacy PROC_TABLE becomes a compat layer
on top of ECS, not the source of truth.

### 1.1 Entity-based process creation
- `ecs_spawn_process(elf_addr, kernel_pml4) → entity_id`
  - Allocates entity via world_spawn
  - Creates user PML4, loads ELF, sets up kernel stack (reuses existing spawn code)
  - Inserts ThreadState (status=RUNNABLE), MemoryMap (pml4_phys), SchedulingPriority
  - Registers entity in legacy PROC_TABLE for backward compat
- **Deliverable:** Boot-spawned processes exist as both entities and PROC_TABLE entries

### 1.2 Transition sys_spawn to ECS
- Modify sys_spawn to:
  1. Accept entity capability instead of path (or keep path as compat)
  2. Create child entity via world_spawn
  3. Insert ThreadState + MemoryMap + SchedulingPriority
  4. Grant stdin/stdout/stderr IpcCapability to child
  5. Return entity_id as the "pid"
- **Deliverable:** `syscall.spawn("/hello")` works through ECS entity creation

### 1.3 Entity lifecycle syscalls
- `sys_entity_wait(entity_id)` — query ThreadState, yield until ZOMBIE
- `sys_entity_exit(code)` — set ThreadState → ZOMBIE, store exit code in entity data
- Reaper: on scheduler tick, free ZOMBIE entities via world_destroy
- **Deliverable:** Parent spawns child entity, waits, receives exit code

### 1.4 Capability passing at spawn
- On entity spawn, parent grants capabilities to child:
  - Console IpcCapability (stdin/stdout/stderr)
  - MemoryMap capability (child's own memory)
- Capabilities stored in IpcCapSparseSet, indexed by entity_id
- **Deliverable:** Child entity inherits console capability, prints to serial

## Tier 2: ECS I/O Model (Week 7-10)

**Goal:** I/O goes through entity capabilities, not file descriptors.
The fd table becomes a compat shim. VFS becomes unused.

### 2.1 Console entity
- Create a singleton "console" entity at boot with IpcCapability
- sys_write and sys_read route through console entity:
  - sys_write: send message to console entity → serial output
  - sys_read: receive message from console entity → serial RX ring
- Remove fd=0/1/2 special-cases in syscall_io.salt
- **Deliverable:** stdio.print works via IPC to console entity

### 2.2 Data entities (replaces files)
- `sys_entity_create_data(size)` → new entity with DataBlob component
- `sys_entity_read(entity_id, offset, buf, len)` → read from data entity
- `sys_entity_write(entity_id, offset, buf, len)` → write to data entity
- DataBlob component: size + list of physical pages
- **Deliverable:** Program creates data entity, writes to it, reads it back

### 2.3 Entity directory (replaces directory tree)
- `sys_entity_register(name, entity_id)` — register entity by name
- `sys_entity_lookup(name)` — find entity by name
- Names stored in a name→entity sparse set or a simple registry
- **Deliverable:** Program registers entity as "hello", another looks it up

### 2.4 Legacy VFS freeze
- Stop adding features to vfs.salt, ext2.salt, ramfs.salt
- Existing VFS code compiles but no new callers
- sys_open/sys_read/sys_write VFS path preserved for EXT2 disk access only
- **Deliverable:** No new imports of kernel.sys.vfs outside of syscall_io.salt

## Tier 3: ECS Scheduler (Week 11-14)

**Goal:** The ECS scheduler dispatches all processes. The legacy scheduler
becomes dead code.

### 3.1 Wire ECS scheduler to timer interrupt
- PIT timer ISR calls scheduler_system_tick(world, core_id)
- Selected entity's ThreadState.ctx_ptr has the saved kernel RSP
- proc_context_switch to the selected entity
- **Deliverable:** Timer interrupt dispatches through ECS, not legacy scheduler

### 3.2 ECS-native context switch
- Entity's ThreadState.stack_ptr stores the saved kernel RSP
- On switch-out: save RSP → entity's ThreadState.stack_ptr
- On switch-in: load RSP from entity's ThreadState.stack_ptr
- Remove dependency on PROC_TABLE for context switching
- **Deliverable:** Context switch reads/writes entity ThreadState, not PROC_TABLE

### 3.3 ECS idle loop
- When no RUNNABLE entities, scheduler_system_tick returns ENTITY_INVALID
- Idle: hlt until next interrupt
- **Deliverable:** Kernel idles cleanly when no work to do

### 3.4 Legacy scheduler removal
- scheduler.salt: mark all functions as deprecated
- do_dispatch() becomes a wrapper around scheduler_system_tick
- PROC_TABLE state field mirrors ThreadState.status (compat only)
- **Deliverable:** Legacy scheduler zero callers from timer ISR

## Tier 4: The Shell (Week 15-18)

**Goal:** A proper shell built on ECS primitives. Entity-based, not path-based.

### 4.1 Grit command parser
- Tokenize input, identify commands vs. arguments
- Built-in dispatch: cd, ps, free, spawn, help
- **Deliverable:** `ps` prints entity list from World

### 4.2 Entity-based program lookup
- sys_entity_lookup(name) finds executable entities
- spawn entity by creating child with ThreadState + MemoryMap
- Wait for child via sys_entity_wait
- **Deliverable:** `spawn hello` runs the hello entity

### 4.3 I/O redirection via capability rerouting
- `>` redirect: replace child's stdout IpcCapability with data entity
- `<` redirect: replace child's stdin IpcCapability with data entity
- `|` pipe: create intermediate entity with dual IpcCapability
- **Deliverable:** `spawn echo hello > greeting` writes to data entity

### 4.4 Job control
- Background entities: spawn with THREAD_RUNNABLE, don't wait
- `jobs` builtin: query entities with ThreadState where parent = shell
- Signal delivery via IPC to entity
- **Deliverable:** `spawn sleep 5 &` returns immediately, `jobs` shows it

## Tier 5: ECS Networking (Week 19-22)

**Goal:** Networking through entity components. NetD manages socket entities.

### 5.1 Socket entities
- sys_socket_create() → entity with OpenSocket component
- OpenSocket: protocol, local_port, remote_ip, remote_port, state, rx_ring, tx_ring
- sys_socket_connect(entity_id, ip, port) → sets OpenSocket fields, initiates TCP
- **Deliverable:** User program creates socket entity, connects via ECS

### 5.2 NetD as socket entity manager
- NetD owns the OpenSocket sparse set
- User programs request socket entities from NetD via IPC
- NetD grants IpcCapability for the SPSC rings
- **Deliverable:** fetch requests socket entity from NetD, NetD creates it

### 5.3 TCP state machine as component
- TCP connection state (CLOSED, SYN_SENT, ESTABLISHED, etc.) in OpenSocket
- TCP retransmission via entity timers (EpochInfo component)
- **Deliverable:** TCP connection establishes via entity state transitions

### 5.4 ping via ICMP entity
- ICMP echo entity: short-lived entity for ping request/reply
- sys_ping(ip) → creates ICMP entity, waits for reply, returns RTT
- **Deliverable:** ping.salt shows actual RTT, not stub message

## Tier 6: Hardening & Persistence (Week 23-24)

### 6.1 World snapshot
- Serialize World state (entities + components) to NVMe on shutdown
- Restore on boot — entities survive reboot
- **Deliverable:** Registered entities persist across power cycles

### 6.2 Capability security
- Z3 proof hints on all IpcCapability transfers
- Entity isolation: entity A cannot access entity B's MemoryMap without capability
- Validate generation on every entity reference
- **Deliverable:** Security audit document

### 6.3 Performance baseline
- Entity spawn → RUNNABLE latency
- ECS scheduler tick latency (SELECT query time)
- IPC round-trip via entity capability
- **Deliverable:** Performance dashboard in CI

## Deliberately NOT Building

- VFS hierarchy, inodes, directory trees — entities replace them
- POSIX compatibility — ECS-native API is the target
- File descriptors beyond compat shim — IpcCapability entities replace them
- EXT2 integration beyond boot disk — World snapshot is the storage model
- RamFS — data entities with DataBlob components are the "filesystem"

## Dependency Graph

```
Tier 0 (test harness, World diagnostics, ECS spawn test)
  └─→ Tier 1 (ECS process model: entity spawn, lifecycle, capabilities)
        └─→ Tier 2 (ECS I/O: console entity, data entities, entity directory)
              └─→ Tier 3 (ECS scheduler: timer ISR, context switch, idle)
                    └─→ Tier 4 (ECS shell: entity lookup, capability routing)
                          └─→ Tier 5 (ECS networking: socket entities, NetD)
                                └─→ Tier 6 (hardening: persistence, security, perf)
```

## Measurement Gates

After each tier:
- `make test-userspace` — all previously passing tests still pass
- New ECS tests added to test harness
- Entities created during boot: count (increases each tier)
- Legacy code paths touched: count (decreases each tier)
- No new imports of kernel.sys.vfs, kernel.sys.ext2, kernel.sys.ramfs
