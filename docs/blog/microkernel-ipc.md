# Microkernel IPC Without the Performance Tax

**Published:** June 2026 | **Author:** The KeuOS Team | **Reading time:** 12 minutes

---

Mach was the original microkernel. Servers ran in userspace, communicated
through message-passing, and the kernel mediated every interaction. It
was elegant. It was also slow — roughly 2x the cost of a monolithic Unix
for equivalent work. The overhead came from context switches, data
copies, and lock contention on shared IPC channels.

KeuOS takes the microkernel architecture but removes the IPC tax through
three structural decisions: no-trap data planes, no-copy buffers, and
no-lock channels. These aren't new ideas — they're used in HFT systems,
video game engines, and userspace drivers. What KeuOS adds is
compile-time verification of the safety invariants.

---

## 1. No Trap: SPSC Rings in Shared Memory

Traditional microkernel IPC: the sender executes a syscall, the kernel
copies the message into a kernel buffer, context-switches to the
receiver, and copies the message out. Two copies, two privilege
transitions, one context switch. Each step costs hundreds of cycles.

KeuOS uses single-producer, single-consumer rings mapped into both the
sender and receiver's address spaces via `sys_shm_grant`. The data plane
is regular load/store. No syscall. No kernel mediation. The kernel's
role is limited to initial setup — mapping the page and validating the
ring descriptor.

```
Userspace Producer                    Userspace Consumer
     │                                      │
     ▼                                      ▼
 write(ring->buf[tail])               read(ring->buf[head])
 atomic_store(tail + 1)              atomic_load(head)
     │                                      │
     └────────── shared memory page ────────┘
               (no kernel transition)
```

The ring format: 2 bytes of frame length followed by raw Ethernet frames
or fixed-size command descriptors depending on the protocol. Head and
tail indices sit on separate cache lines to avoid false sharing. The
producer owns the tail, the consumer owns the head. Neither touches the
other's index.

---

## 2. No Copy: DMA Into the Ring

In a conventional network stack, the NIC DMAs a packet into a kernel
buffer, the kernel copies it to a userspace buffer, and the application
processes it. That's one copy across the kernel-userspace boundary.

KeuOS's VirtIO-net driver DMAs directly into the RX ring page — the same
page mapped into the NetD daemon's address space. The userspace consumer
reads the frame from the same physical memory the NIC wrote to. Zero
copies between hardware and application.

The buffer layout is pre-negotiated at ring setup: the kernel reserves
the first page of the RX pool for the ring header (head, tail, capacity,
flags), and the remaining pages are the data region. The NIC writes into
the data region, the consumer reads from the data region. Same bytes,
different virtual addresses, same physical page.

---

## 3. No Lock: Single-Producer, Single-Consumer

A multi-producer ring needs locks or compare-and-swap on the head
pointer. An SPSC ring doesn't. The producer owns the tail, the consumer
owns the head. The only synchronization is memory ordering — the
producer needs a release store on tail, the consumer needs an acquire
load on head.

On x86-64, which is total-store-order, those atomics compile to plain
`mov` instructions. A ring operation costs on the order of tens of
cycles — comparable to a function call. A conventional kernel-mediated
IPC (syscall + copy + context switch) costs on the order of a thousand
cycles or more. The ring-based path eliminates the syscall, the copy,
and the switch.

---

## Proof-Carrying IPC

Sharing memory between kernel and userspace creates a security problem:
the userspace process can corrupt the ring descriptor, forge frame
lengths, or read past the buffer boundary. KeuOS catches these at
compile time through three gates:

**Alignment Gate.** The ring descriptor is a struct with contracts that
the capacity is a power of two and the base address is page-aligned. Z3
proves these at compile time for every ring created through the syscall
API:

```salt
fn init_ring(descriptor: RingDescriptor)
    requires(descriptor.capacity > 0)
    requires(is_power_of_two(descriptor.capacity))
    requires(descriptor.base_addr % 4096 == 0)
{ /* setup SPSC ring */ }
```

Call `init_ring` with a misaligned address or a non-power-of-two
capacity and the compiler stops with the violating value. The check
never makes it to runtime.

**Bounds Gate.** Every frame in the ring is prefixed with a 2-byte
length. The kernel validates this length against the ring capacity
before enqueueing. Userspace can write garbage to the length field; the
kernel treats it as untrusted input and clamps it via SPSC validation.
The clamping logic itself carries contracts that Z3 proves exhaustive.

**MMU Gate.** The ring page is mapped read-write in userspace but the
kernel's higher-half mapping is privileged. Userspace cannot access
kernel-only pages. `copy_from_user` and `copy_to_user` perform
SMAP/KPTI-safe access with explicit permission checks. Ring 3 code
cannot forge a kernel pointer.

---

## NetD: Userspace Networking

NetD is KeuOS's network daemon. It runs entirely in Ring 3 — no kernel
privileges, no direct hardware access. It communicates with the kernel
through SPSC rings: an RX ring for incoming packets, a TX ring for
outgoing packets, and a control ring for socket operations (bind,
connect, listen, accept).

The kernel's VirtIO-net ISR writes incoming frames directly into NetD's
RX ring and pushes a notification through the pulse event system. NetD
wakes, reads the frame from shared memory, processes it (ARP, IP, TCP),
and writes responses to the TX ring. No copy. Minimal kernel
involvement.

The NetD migration from Ring 0 to Ring 3 was a ~300-line change. The
ring infrastructure already existed for the terminal and keyboard
subsystems. The network path was the last to move.

---

## C10M Benchmark

The C10M echo server benchmark measures TCP throughput under 10 million
concurrent connections. On macOS ARM64 (Apple Silicon M4), average of 3
runs:

| Implementation | Throughput | vs Salt |
|---------------|-----------|---------|
| C (kqueue directly) | 29.5k RPS | +7.5% |
| **Salt (NetD, SPSC rings)** | **27.2k RPS** | — |
| Rust (Tokio async) | 26.4k RPS | -3.0% |

The C implementation has an advantage: it calls `kevent` directly. Salt's
networking stack goes through the NetD daemon, which adds one ring
round-trip. The 7.5% gap is that round-trip. Salt still beats Rust/Tokio
async by 3%.

The IPC path for a 64-byte message, derived from the SPSC ring
instruction sequence on x86-64:

| Operation | Cost |
|-----------|------|
| Write frame to ring | ~10 cycles (4 `mov` + release store) |
| Memory barrier | ~10 cycles (`sfence` or equivalent) |
| Consumer reads frame | ~10 cycles (acquire load + 4 `mov`) |
| **Total** | **~30 cycles** |

A conventional kernel-mediated IPC (syscall + copy + context switch)
costs ~1,000+ cycles. The ring-based path eliminates the syscall, the
copy, and the switch.

The trade is that SPSC rings work for point-to-point communication
between exactly two parties. If you need multicast or many-to-one
communication, you need multiple rings or a different primitive. For the
common case — a daemon talking to the kernel — one ring per direction is
sufficient.

---

## The Pattern

No-trap data planes, no-copy buffers, and no-lock channels are
established techniques. They're the foundation of every high-performance
userspace driver and every kernel-bypass networking stack. What KeuOS
adds is compile-time verification: the ring contracts, the bounds
checks, and the MMU invariants are proved for every ring instance before
the kernel boots.

The result is a microkernel that doesn't pay the IPC tax — and a
language where the proof of that safety ships with the binary.

[Read the NetD migration deep-dive →](/docs/deep-dives/netd-ring3-migration.md)
