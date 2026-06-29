# Chapter 8: Async, Yield, and State Machines

## The Problem

In KeuOS a single core runs hundreds of fibers. When one fiber issues a network read and the socket buffer is empty, it must not stall the entire core. It should **suspend itself**, let the scheduler run another fiber, and resume later when data arrives. Salt solves this with stackless async functions.

## The `@yielding` Attribute

Mark a function `@yielding` to opt into cooperative scheduling. The compiler injects yield checks at loop back-edges — every iteration checks a deadline counter and yields to the executor if the budget is exhausted:

```salt
package main

import std.core.keuos.context.Context

@yielding
fn process_batch(ctx: &Context, items: &[i64]) -> i64 {
    let mut sum: i64 = 0;
    let mut i: i64 = 0;
    while i < items.len() as i64 {
        sum = sum + items[i as i64];
        i = i + 1;
        // Compiler inserts a yield check here.
        // If the time budget is used up, the function suspends.
    }
    return sum;
}

// Synchronous — no Context, guaranteed non-blocking.
// Z3 rejects any call to a @yielding function from here.
fn compute_sum(items: &[i64]) -> i64 {
    let mut sum: i64 = 0;
    let mut i: i64 = 0;
    while i < items.len() as i64 {
        sum = sum + items[i as i64];
        i = i + 1;
    }
    return sum;
}
```

The `Context` parameter is a **capability token** — it proves this function runs under the executor. Functions without `Context` are proven synchronous at compile time by Z3. Calling a `@yielding` function from a synchronous context is a compile-time error.

The optional argument tunes the heartbeat: `@yielding(4096)` inserts a yield check every 4096 iterations instead of the default 1024.

## The `yield` Keyword

Inside a `@yielding` function, `yield;` suspends execution immediately and returns control to the scheduler. The fiber resumes from the same point later:

```salt
import std.io.keuos_poller.{KeuOSPoller, PollFilter}

@yielding
fn read_packet(ctx: &Context, poller: &KeuOSPoller, fd: i32) -> Packet {
    let buf = Buffer::new(4096);

    loop {
        let n = try_read(fd, &buf);
        if n > 0 {
            break;
        }
        // Register interest and suspend — poller will resume
        // this fiber when fd becomes readable.
        poller.register(fd, PollFilter::Read);
        yield;
    }

    return Packet::from(&buf);
}
```

The compiler transforms this function into a state machine. Each `yield` point becomes a numbered state. Variables that live across a yield boundary (`buf` in the example) are "lifted" into a struct field so their value survives suspension.

## How Lowering Works

Given a function with one yield point:

```salt
@yielding
fn example(ctx: &Context, start: i64) -> i64 {
    let tmp = start + 1;
    yield;
    return tmp * 2;
}
```

The compiler emits something equivalent to:

```salt
// Generated struct — one field per yield-crossing variable
struct __AsyncState_example {
    __state: i64,   // discriminator: 0 = enter, 1 = resume
    start: i64,     // parameter (lives across yield)
    tmp: i64,       // lifted local
}

// Generated step function — called by the scheduler on each resume
fn __step_example(state: &mut __AsyncState_example) -> i64 {
    match state.__state {
        0 => {
            state.tmp = state.start + 1;
            state.__state = 1;
            return 0;  // POLL_PENDING
        }
        1 => {
            state.__state = -1;  // completed
            return 1;  // POLL_READY
        }
        _ => unreachable(),
    }
}
```

The scheduler calls `__step_example` repeatedly. The return value uses the **Poll ABI**:

| Constant | Value | Meaning |
|----------|-------|---------|
| `POLL_PENDING` | `0` | Fiber suspended — reschedule later |
| `POLL_READY` | `1` | Fiber finished — deallocate frame |

## Non-Blocking I/O in KeuOS

KeuOS ties async yielding to I/O through `Context`. A socket read with Context calls into the kernel which either returns data immediately or registers the fiber for wakeup. The userspace wrapper looks like this:

```salt
@yielding
fn socket_read(ctx: &Context, sock: &mut TcpSocket,
               poller: &KeuOSPoller, buf: &mut [u8]) -> i64 {
    loop {
        let result = sock.try_recv(ctx, buf);
        if result >= 0 {
            return result;
        }
        poller.register(sock.fd(), PollFilter::Read);
        yield;
    }
}
```

The kernel path:

```
Userspace fiber calls socket.read(ctx, &mut buf)
    │
    ▼
Kernel checks: data in socket buffer?
    │
    ├── Yes → copy data into buf, return Ok(len)
    │
    └── No  → register fiber in socket's wake list
              return Err(WouldBlock)
                  │
                  ▼
Userspace yields via poller.register() + yield
                  │
                  ▼
Poller wakes fiber when socket has data
                  │
                  ▼
Fiber retries the read — data is now available
```

The `Context` token is the lynchpin: it proves the fiber is under the scheduler, so yielding is safe. The Z3 verifier rejects any I/O path that lacks a `Context`, preventing accidental blocking in synchronous code.

## Comparison

| Feature | Salt | Rust | Go |
|---------|------|------|----|
| Async model | Stackless state machine | Stackless state machine (async/await) | Stackful goroutine |
| Yield injection | Compiler-injected at loop edges via `@yielding` | Manual `.await` points | Runtime-managed preemption |
| Suspension target | `yield;` keyword | `.await` expression | `runtime.Gosched()` |
| Sync/async boundary | Z3-verified via `Context` token | `Send`/`Sync` traits | Implicit (goroutines can block) |
| State representation | `__AsyncState_{name}` struct | Generated `Future` enum | Goroutine stack (growable) |
| Scheduler | Per-core bitmap, O(1) dispatch | Per-thread work-stealing (tokio/async-std) | M:N scheduler with work-stealing |

Salt's approach is closest to Rust's: both lower async functions to stackless state machines. The key difference is that Salt uses compiler-injected yield checks at loop edges (via `@yielding`) so long-running compute loops automatically cooperate without manual `.await` insertions. Go's goroutines are stackful and preempted by the runtime, which is more flexible but uses more memory per task (kilobytes vs. bytes).

With 256 fibers per core at ~64 bytes of state each, the entire async dispatch fits in a single cache line.

## Summary

| Feature | Syntax / Mechanism | Purpose |
|---------|-------------------|---------|
| Cooperative scheduling | `@yielding fn f(ctx: &Context) { ... }` | Opt into auto yield-check injection |
| Explicit suspension | `yield;` inside `@yielding` fn | Suspend until poller wakes the fiber |
| Capability token | `Context` parameter | Z3-verified proof of executor context |
| Poll Pending | Return `0` from step function | Signal "not done, reschedule me" |
| Poll Ready | Return `1` from step function | Signal "done, deallocate frame" |
| State lowering | `async fn` → `__AsyncState_{name}` struct + step dispatch | Stackless state machine transform |

Next: [Chapter 9: Z3 Contracts](09-contracts.md)
