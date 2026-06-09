# LETTUCE 🥬

**An experimental in-memory data store written entirely in Salt, exploring zero-cost abstraction with Arenas and formal verification.**

LETTUCE serves as a proof-of-concept for the Salt language, demonstrating how to build a networked key-value store without `malloc`, garbage collection, or lifetime annotations. 

---

## Why Is It Interesting?

LETTUCE does not use `malloc`, `free`, garbage collection, or reference counting in the hot path. Every architectural choice eliminates a class of overhead that conventional key-value stores pay on every request.

### 1. Zero-Copy RESP Parsing

The RESP parser never allocates. It returns `StringView` (pointer + length) into the recv buffer. A `SET foo bar` command produces zero heap allocations — the key and value are views directly into the kernel's TCP read buffer.

### 2. Arena-Backed SwissTable (`StringMap`)

The data store is a SwissTable hash map with SWAR (SIMD Within A Register) probe matching, backed by a bump-allocator arena. This gives O(1) allocation with zero fragmentation and zero `free` overhead.

### 3. kqueue Event Loop with Single-Syscall Dispatch

The server uses macOS `kqueue` for I/O multiplexing via Salt's `std.net.poller` module. 

### 4. The Compilation Pipeline

Salt compiles to native arm64 through MLIR, inheriting LLVM's full optimization pipeline. The final binary is a statically-linked native executable.

---

## Architecture

```
user/lettuce/
├── server.salt          # Event loop + Reactor
├── store.salt           # Command execution + Database Engine
└── resp.salt            # RESP Parser

Dependencies (Salt stdlib):
├── std.collections.string_map   # 480 lines — SoA SwissTable + Arena
├── std.net.tcp                  # 75 lines  — TcpListener, TcpStream
├── std.net.poller               # 64 lines  — kqueue wrapper
└── std.core.str                 # StringView (zero-copy string slices)
```

**Total application code: ~567 lines of Salt.**  

### Supported Commands

| Command | Description | Response |
|---|---|---|
| `PING` | Health check (inline + RESP) | `+PONG\r\n` |
| `SET key value` | Store a key-value pair | `+OK\r\n` |
| `GET key` | Retrieve value by key | `$N\r\n<data>\r\n` or `$-1\r\n` |
| `DEL key` | Delete a key | `:1\r\n` or `:0\r\n` |

---

## Build & Run

**Prerequisites**: Rust 1.75+, Z3 4.12+ (`brew install z3`), LLVM 21+ (`brew install llvm@21`).

```bash
# 1. Build the Salt compiler (one-time)
cd salt-front && cargo build && cd ..

# 2. Compile LETTUCE → native binary
./scripts/run_test.sh lettuce/src/server.salt --compile-only

# 3. Start the server (Z3 must be on library path)
DYLD_LIBRARY_PATH=/opt/homebrew/lib /tmp/salt_build/server

# 4. Test with redis-cli
redis-cli -p 6379 PING          # → PONG
redis-cli -p 6379 SET foo bar   # → OK
```

---

## License

Part of the KeuOS project. See root `LICENSE` for details.
