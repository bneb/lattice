# Salt Standard Library — API Reference

The Salt standard library provides 70+ modules organized into packages. All modules are written in Salt and compiled with the same Z3 verification as user code.

## Package Index

| Package | Modules | Description |
|---------|---------|-------------|
| `std.core` | 15 | Fundamental types: `Option`, `Result`, `Ptr`, `StringView`, `Clone`, `Eq`, `Arena` |
| `std.collections` | 4 | `Vec<T,A>`, `HashMap<K,V>` (Swiss-table), `Slab<T>`, `StringMap` |
| `std.string` | 1 | `String` — owning string with f-string support |
| `std.io` | 6 | `File`, `BufferedWriter`, `BufferedReader`, `Writer`, multipoll reactors |
| `std.net` | 1 | `TcpListener`, `TcpStream` |
| `std.http` | 4 | HTTP client & server, zero-copy parsing |
| `std.sync` | 3 | `Mutex`, `AtomicI64`, `RCU` |
| `std.thread` | 1 | `Thread::spawn`, `Thread::join` |
| `std.channel` | 2 | Bounded/unbounded channels, typed IPC channels |
| `std.json` | 1 | JSON parser & writer |
| `std.math` | 1 | Vectorized transcendentals, NEON SIMD |
| `std.nn` | 1 | `relu`, `sigmoid`, `softmax`, `cross_entropy` |
| `std.linalg` | 1 | `Tensor`, `matmul`, `transpose` (AMX on Apple Silicon) |
| `std.autograd` | 1 | Automatic differentiation |
| `std.fmt` | 2 | `Debug`, `Display` formatting traits |
| `std.hash` | 1 | WyHash-based hashing |
| `std.crypto` | 1 | TLS bridge (BearSSL FFI) |
| `std.regex` | 1 | Regex engine (QuickJS FFI) |
| `std.encoding` | 1 | Base64, hex encoding |
| `std.fs` | 1 | File system operations |
| `std.path` | 1 | Path manipulation |
| `std.process` | 1 | `Command` — subprocess execution |
| `std.random` | 1 | Random number generation |
| `std.time` | 1 | Clock and timing |
| `std.env` | 1 | Environment variables |
| `std.args` | 1 | Command-line argument parsing |

## Detailed References

- [Core Types & Primitives](core.md) — `Option`, `Result`, `Ptr`, `StringView`, `Arena`, `Clone`, `Eq`
- [Collections](collections.md) — `Vec`, `HashMap`, `Slab`, `StringMap`
- [I/O & Networking](io-net.md) — `File`, `Writer`, `BufferedWriter`, `TcpListener`, `TcpStream`
- [Concurrency](concurrency.md) — `Thread`, `Mutex`, `AtomicI64`, `Channel`
- [Math, ML & Specialized](math-ml.md) — SIMD, `Tensor`, NN ops, autograd, crypto, JSON
