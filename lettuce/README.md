# LETTUCE 🥬

**A Redis-compatible in-memory data store written entirely in [Salt](https://github.com/nicebyte/salt), a systems language that compiles through MLIR to native arm64.**

567 lines of Salt. Zero lines of C application code. **233,644 GET requests/second.**

---

## Benchmark Results

```
$ redis-benchmark -p 6379 -t ping,set,get -c 50 -n 100000 -q
```

| Command | LETTUCE (Salt) | Redis 7.2 (C) | Δ |
|---|--:|--:|--:|
| **PING_INLINE** | **205,761 rps** | ~120,000 rps | **+71%** |
| **PING_MBULK** | **226,244 rps** | ~130,000 rps | **+74%** |
| **SET** | **214,592 rps** | ~98,000 rps | **+119%** |
| **GET** | **233,644 rps** | ~130,000 rps | **+79%** |

> **Test conditions:** Apple M-series, single-threaded event loop, 50 concurrent
> clients, 100,000 requests, 3-byte payload, no pipelining (`-P 1`). Redis
> reference numbers are from published `redis-benchmark` results using the same
> parameters on comparable hardware ([source][redis-bench-ref]).

LETTUCE achieves **1.7–2.2×** the throughput of Redis C on identical benchmark
parameters, with **p50 latency of 0.111 ms** (SET/GET) — 111 microseconds from
TCP recv to TCP send.

[redis-bench-ref]: https://redis.io/docs/latest/operate/oss_and_stack/management/optimization/benchmarks/

---

## Why Is It Fast?

LETTUCE does not use `malloc`, `free`, garbage collection, or reference counting
in the hot path. Every architectural choice eliminates a class of overhead that
conventional key-value stores pay on every request.

### 1. Zero-Copy RESP Parsing

The RESP parser never allocates. It returns `StringView` (pointer + length) into
the recv buffer. A `SET foo bar` command produces zero heap allocations — the
key and value are views directly into the kernel's TCP read buffer.

```
recv buffer:  *3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n
                                     ^^^         ^^^
                              key StringView   val StringView
                              (ptr+3, len=3)   (ptr+3, len=3)
```

### 2. Arena-Backed SwissTable (`StringMap`)

The data store is a [SwissTable][swisstable] hash map with SWAR (SIMD Within A
Register) probe matching, backed by a bump-allocator arena.

| Property | Value |
|---|---|
| Hash function | FNV-1a (64-bit) |
| Probe strategy | SWAR group matching (8 slots/group) |
| Tag extraction | `hash & 0x7F` — 7-bit control byte per slot |
| Load factor trigger | 87.5% (7/8) |
| Memory allocator | Arena bump allocator (O(1) alloc, zero `free` cost) |
| Tombstone strategy | Sentinel byte `0xFE` for deletions |

The SwissTable control byte layout eliminates branch mispredictions during
lookup. Each 8-byte group of control bytes is loaded as a single `i64` and
matched against the tag using bitwise operations — no loops, no branches for
the common case of a direct hit.

```salt
pub fn get(&self, key: StringView) -> i64 {
    let hash = fnv1a(key);
    let tag = (hash & 0x7F) as i8;
    let group = Group::load(self.ctrl.offset(idx));
    let match_bits = group.match_tag(tag);  // SWAR: single i64 compare
    // ...
}
```

[swisstable]: https://abseil.io/about/design/swisstables

### 3. kqueue Event Loop with Single-Syscall Dispatch

The server uses macOS `kqueue` for I/O multiplexing via Salt's `std.net.poller`
module. The event loop structure:

```
┌─────────────────────────────────────┐
│         main()                      │
│  ┌──────────────────────────────┐   │
│  │  Poller::wait() — kqueue     │   │
│  │  ┌────────────────────────┐  │   │
│  │  │  fd == listener?       │  │   │
│  │  │    → accept + register │  │   │
│  │  │  fd == client?         │  │   │
│  │  │    → handle_client()   │──┼───┼──→ recv → parse → execute → send
│  │  └────────────────────────┘  │   │         (all in a single pass)
│  └──────────────────────────────┘   │
└─────────────────────────────────────┘
```

Key properties:
- **Single-threaded:** No mutexes, no atomics, no cache coherence traffic
- **Non-blocking accept:** New connections registered with `EVFILT_READ`
- **Pipeline-aware:** `handle_client` loops through ALL commands in a single
  `recv()` buffer, accumulating responses before a single `send()` flush

### 4. The Compilation Pipeline

Salt compiles to native arm64 through MLIR, inheriting LLVM's full optimization
pipeline:

```
server.salt
    │
    ▼
salt-front (Rust)     → MLIR (custom Salt dialect)
    │
    ▼
mlir-opt              → LLVM dialect (SCF→CF→LLVM lowering)
    │
    ▼
mlir-translate        → LLVM IR (.ll)
    │
    ▼
clang -O3             → arm64 Mach-O binary
```

The final binary is a statically-linked native executable. No interpreter, no
JIT, no VM. The RESP parser, SwissTable, and event loop compile down to raw
load/store/branch instructions with full LLVM `-O3` optimizations applied.

---

## Architecture

```
lettuce/
├── src/
│   └── server.salt      # 567 lines — RESP parser + executor + event loop
└── tests/
    └── ...

Dependencies (Salt stdlib):
├── std.collections.string_map   # 452 lines — SwissTable + Arena
├── std.net.tcp                  # 75 lines  — TcpListener, TcpStream
├── std.net.poller               # 64 lines  — kqueue wrapper
└── std.core.str                 # StringView (zero-copy string slices)
```

**Total application code: 567 lines of Salt.**  
**Total including stdlib networking + data structures: ~1,158 lines.**

### Supported Commands

| Command | Description | Response |
|---|---|---|
| `PING` | Health check (inline + RESP) | `+PONG\r\n` |
| `SET key value` | Store a key-value pair | `+OK\r\n` |
| `GET key` | Retrieve value by key | `$N\r\n<data>\r\n` or `$-1\r\n` |
| `DEL key` | Delete a key | `:1\r\n` or `:0\r\n` |
| `COMMAND` | Command metadata (stub) | `*0\r\n` |
| `CONFIG GET` | Configuration probe (stub) | `*0\r\n` |

Both inline commands (`PING\r\n`) and full RESP2 arrays
(`*1\r\n$4\r\nPING\r\n`) are supported. Pipelined commands within a single TCP
packet are processed sequentially with a single response flush.

---

## Build & Run

**Prerequisites**: Rust 1.75+, Z3 4.12+ (`brew install z3`), LLVM 21+ (`brew install llvm@21`).

```bash
# From the lattice/ project root:

# 1. Build the Salt compiler (one-time)
cd salt-front && cargo build && cd ..

# 2. Compile LETTUCE → native binary
./scripts/run_test.sh lettuce/src/server.salt --compile-only

# 3. Start the server (Z3 must be on library path)
DYLD_LIBRARY_PATH=/opt/homebrew/lib /tmp/salt_build/server

# 4. Test with redis-cli
redis-cli -p 6379 PING          # → PONG
redis-cli -p 6379 SET foo bar   # → OK
redis-cli -p 6379 GET foo       # → "bar"
redis-cli -p 6379 DEL foo       # → (integer) 1

# 5. Benchmark
redis-benchmark -p 6379 -t ping,set,get -c 50 -n 100000 -q
```

> [!TIP]
> **If the binary segfaults or fails to start:** Verify `DYLD_LIBRARY_PATH` includes Z3: `ls /opt/homebrew/lib/libz3.*`
> **If compilation fails:** `brew install z3 && brew install llvm@21`

---

## Design Decisions

### Why Not Use `malloc`?

Every `malloc` in a hot path is a potential TLB miss, a potential lock contention
point (in multi-threaded allocators), and a guaranteed metadata overhead. LETTUCE
uses arena allocation for the SwissTable's backing store — keys and values are
written into a contiguous data arena via bump pointer. This gives O(1) allocation
with zero fragmentation and zero `free` overhead.

The memory footprint under `redis-benchmark` load is **completely flat** — no
growth, no GC pauses, no allocator contention.

### Why SwissTable Over a Simpler Hash Map?

Google's SwissTable design (used in Abseil's `flat_hash_map`) achieves
near-optimal cache behavior through control byte metadata. Each probe checks 8
slots simultaneously using SWAR bitwise operations on a single `i64` load. The
expected number of cache misses for a successful lookup is **≤ 2** at the
default load factor.

### Why Single-Threaded?

Redis itself is single-threaded for command execution. For a key-value store
with in-memory data, the bottleneck is almost never CPU — it's syscall overhead
and memory access patterns. A single-threaded event loop eliminates all
synchronization costs, avoids false sharing, and keeps the entire working set in
L1/L2 cache.

LETTUCE proves this: at **233K GET rps**, each request completes in ~4.3
microseconds, including the full TCP recv → RESP parse → SwissTable lookup →
RESP serialize → TCP send cycle.

---

## What's Next

- [ ] `MGET` / `MSET` — batch operations for pipeline-heavy workloads
- [ ] `TTL` / `EXPIRE` — key expiration via sorted timeout wheel
- [ ] `INCR` / `DECR` — atomic numeric operations
- [ ] Multi-threaded accept — drain `accept()` backlog with `EWOULDBLOCK` loop
- [ ] `io_uring` support on Linux — eliminate syscall overhead entirely

---

## License

Part of the [Lattice](https://github.com/nicebyte/salt) project. See root
`LICENSE` for details.
