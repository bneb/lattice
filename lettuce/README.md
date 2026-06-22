# LETTUCE

A Redis-compatible server written in [Salt](https://github.com/bneb/lattice), a systems language with compile-time Z3 verification. Supports PING, SET, GET, DEL over the Redis protocol (RESP) on port 6379.

```
$ make lettuce
$ redis-cli -p 6379 PING → PONG
$ redis-cli -p 6379 SET k v → OK
```

---

## Verification

Salt supports `requires` and `ensures` clauses on functions. The compiler translates these to Z3 formulas and checks them at compile time. If Z3 can prove the condition always holds, the runtime check is elided. If Z3 finds a counterexample, it's reported as a warning. If Z3 cannot decide within the timeout, a runtime assertion is emitted as a safe fallback.

LETTUCE uses this on its persistence layer. From `aof.salt`:

```salt
fn Aof_append_set(ctx: Ptr<AofContext>, key: StringView, val: StringView) {
    requires(!ctx.is_null())
    requires(key.length() > 0 && key.length() <= 4000)
    requires(val.length() > 0 && val.length() <= 4000)
    // ...
}
```

The RESP parser also carries bounds annotations that Z3 can statically prove, eliminating bounds checks in the generated code.

Run the contract suite:

```bash
$ make lettuce-verify
  resp_contracts: PASS (Z3 contracts verified)
  aof_contracts:  PASS (Z3 contracts verified)
  store_module:   PASS
```

---

## Commands

| Command | Response |
|---------|----------|
| `PING` | `+PONG` |
| `SET key value` | `+OK` |
| `GET key` | bulk string or `$-1` (null) |
| `DEL key` | `:1` or `:0` |

Works with `redis-cli` and `redis-benchmark`.

---

## Build

Prerequisites: Rust 1.75+, Z3 4.12+, LLVM 21+.

```bash
git clone https://github.com/bneb/lattice.git
cd lattice
make lettuce          # compiler + contract verification + MLIR output
```

MLIR is written to `/tmp/lettuce_server.mlir`.

---

## How it works

`server.salt` (314 lines) runs an event loop over `kqueue`/`epoll`. Each connection gets a 16KB sliding window buffer allocated from a `Slab<ClientSession>`. The RESP parser (`resp.salt`) returns `StringView` pointers into the buffer — zero-copy, zero-alloc on the read path. The key-value store (`store.salt`) uses an arena-backed hash map with SWAR probing. Persistence (`aof.salt`) writes commands to an append-only file with Z3-verified buffer bounds.

---

## Further reading

- [Salt tutorial](https://github.com/bneb/lattice/tree/main/docs/tutorial) — build your first verified Salt program
- [Salt language spec](https://github.com/bneb/lattice/blob/main/docs/SPEC.md)
- [KeuOS kernel](https://github.com/bneb/lattice/tree/main/kernel) — the OS Salt was built for

---

## License

Part of the KeuOS project. See repository root `LICENSE`.
