# Concurrency

## `std.thread.Thread`

OS-level threads with pthread backing. Each thread receives its own stack and kernel scheduling.

```salt
use std.thread.Thread
```

| Function/Method | Signature | Description |
|----------------|-----------|-------------|
| `Thread::spawn` | `(fn() -> i64) -> Thread` | Spawn a new thread |
| `join` | `(&self) -> i64` | Wait for thread completion, get return value |

**Usage:**
```salt
use std.thread.Thread

fn worker() -> i64 {
    println("running on worker thread");
    return 42;
}

fn main() -> i32 {
    let handle = Thread::spawn(worker);
    let result = handle.join();  // 42
    println(f"worker returned {result}");
    return 0;
}
```

## `std.sync.Mutex`

Mutual exclusion lock. Protects shared data from concurrent access.

```salt
use std.sync.Mutex
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `() -> Mutex` | Create unlocked mutex |
| `lock` | `(&mut self) -> ()` | Acquire lock (blocks if held) |
| `unlock` | `(&mut self) -> ()` | Release lock |
| `destroy` | `(&mut self) -> ()` | Destroy mutex |

**Usage:**
```salt
use std.sync.Mutex

let m = Mutex::new();

// Thread 1
m.lock();
// ... critical section ...
m.unlock();

// Thread 2
m.lock();   // blocks until Thread 1 unlocks
// ... critical section ...
m.unlock();

m.destroy();
```

## `std.sync.AtomicI64`

C11-compatible 64-bit atomic integer. Lock-free operations with sequentially-consistent ordering.

```salt
use std.sync.AtomicI64
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(i64) -> AtomicI64` | Create with initial value |
| `load` | `(&self) -> i64` | Atomic load (acquire semantics) |
| `store` | `(&mut self, i64) -> ()` | Atomic store (release semantics) |
| `fetch_add` | `(&mut self, i64) -> i64` | Atomic add, return previous value |
| `compare_exchange` | `(&mut self, i64, i64) -> i64` | CAS: returns value at location (equals expected if swap succeeded) |

**Usage:**
```salt
use std.sync.AtomicI64

let mut counter = AtomicI64::new(0);
counter.fetch_add(1);      // counter = 1
let val = counter.load();  // val = 1
```

## `std.sync.RCU`

Read-Copy-Update synchronization. Readers never block; writers create new versions.

```salt
use std.sync.rcu
```

## `std.sync.RingBuffer`

Lock-free SPSC ring buffer for single-producer, single-consumer scenarios.

```salt
use std.sync.ring_buffer
```

## `std.channel.Channel`

Bounded channel with fixed-capacity ring buffer. Stores `i64` values. Sends fail when full.

```salt
use std.channel.channel.Channel
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `bounded` | `(i32) -> Channel` | Create with fixed capacity (max 1024) |
| `capacity` | `(&self) -> i32` | Bounded capacity |
| `len` | `(&self) -> i32` | Current number of elements |
| `is_empty` | `(&self) -> bool` | True if empty |
| `is_full` | `(&self) -> bool` | True if full |
| `send` | `(&mut self, i64) -> Result<i32>` | Send value (fails if full) |
| `try_recv` | `(&mut self) -> Option<i64>` | Non-blocking receive |

**Usage:**
```salt
use std.channel.channel.Channel

let mut ch = Channel::bounded(4);  // capacity 4
ch.send(42);
let val = ch.try_recv();  // Option::Some(42)
```

## `std.channel.UnboundedChannel`

Unbounded channel with heap-backed doubling ring buffer. Stores `i64` values. Sends never fail.

```salt
use std.channel.channel.UnboundedChannel
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `() -> UnboundedChannel` | Create unbounded channel |
| `len` | `(&self) -> i64` | Current number of elements |
| `is_empty` | `(&self) -> bool` | True if empty |
| `send` | `(&mut self, i64) -> ()` | Send value (never blocks, grows as needed) |
| `try_recv` | `(&mut self) -> Option<i64>` | Non-blocking receive (FIFO) |

**Usage:**
```salt
use std.channel.channel.UnboundedChannel

let mut uch = UnboundedChannel::new();
uch.send(1);
uch.send(2);
uch.send(3);
let v = uch.try_recv();  // Option::Some(1) — FIFO
```

## `std.ipc.TypedChannel`

KeuOS-specific typed IPC channel for inter-process communication.

```salt
use std.ipc.typed_channel
```

## Cooperative Concurrency

Salt supports cooperative multitasking via `@yielding` and `@pulse` attributes. The compiler injects yield checks at loop back-edges.

```salt
@yielding
fn worker() {
    let mut i = 0;
    while i < 1000000 {
        i = i + 1;
        // Compiler inserts yield checks automatically
    }
}

@pulse(1000)  // 1kHz tick rate
fn high_frequency_task() {
    // Compiler verifies: every path completes within 1ms
}
```

## KeuOS Executor (`std.core.keuos`)

KeuOS-specific async executor with fiber-based task scheduling:

```salt
use std.core.keuos.executor
```

| Component | Description |
|-----------|-------------|
| `executor` | Fiber-based task executor with work-stealing |
| `mailbox` | Async message passing between fibers |
| `context` | Fiber context save/restore (GPR + FXSAVE) |
| `arena` | KeuOS-aware arena with epoch tracking |
