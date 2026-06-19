# Lettuce — Verified HTTP Key-Value Server

**Status:** Goal C deliverable
**Safety model:** Z3 compile-time verification of all buffer accesses

## Architecture

```
Client (RESP/HTTP) → TCP → NetD (Ring 3, SPSC rings) → Lettuce server
                                                              │
                                              ┌───────────────┴───────────────┐
                                              │  RESParser (resp.salt)         │
                                              │  ┌─────────────────────────┐   │
                                              │  │ Z3-verified bounds on    │   │
                                              │  │ every buffer access      │   │
                                              │  └─────────────────────────┘   │
                                              │           │                    │
                                              │           ▼                    │
                                              │  Store (store.salt)            │
                                              │  ├─ SwissTable HashMap         │
                                              │  ├─ arena-allocated keys       │
                                              │  └─ Z3-verified Arena safety   │
                                              │           │                    │
                                              │           ▼                    │
                                              │  AOF (aof.salt)                │
                                              │  ├─ append-only file           │
                                              │  └─ requires() on buffer write  │
                                              └───────────────────────────────┘
```

## Safety Guarantees

### 1. RESP Parser — Zero-Copy with Bounds Verification

Every buffer access in `resp.salt` carries a Z3 `requires()` contract:

| Function | Contract | What It Prevents |
|----------|----------|-----------------|
| `find_crlf` | `requires(start >= 0 && start < input.length())` | OOB read on first byte |
| `parse_int_from_view` | `requires(start >= 0 && end <= input.length() && start <= end)` | OOB read in integer parse loop |
| `resp_parse` | `requires(input.length() > 0 || ...)` | Null/empty input parsing |

**Impact:** Malformed RESP input (truncated frames, negative lengths, oversized bulk strings) cannot cause OOB reads. Z3 proves the bounds at compile time — zero runtime overhead when proven.

### 2. Store — Arena-Allocated Memory

The SwissTable `StringMap` uses arena allocation for keys and values. The `ArenaVerifier` performs compile-time escape analysis:
- No reference outlives its arena
- O(1) bulk deallocation via `arena.reset_to(mark)`
- No per-object free, no fragmentation

### 3. AOF Persistence — Verified Buffer Writes

`Aof_append_set` requires `key.length() > 0 && val.length() > 0`, preventing zero-length writes. The write buffer is bounds-checked against the computed `total_size`.

## Running

```bash
# Build the server
salt-front lettuce/server.salt -o lettuce-server

# Run tests
bash lettuce/tests/test_verified_http.sh

# Verify all contracts
salt-front --verify lettuce/resp.salt
salt-front --verify lettuce/aof.salt
salt-front --verify lettuce/store.salt
```

## Regression Tests

`lettuce/tests/test_verified_http.sh` exercises:
1. SET key → +OK
2. GET key → $value (bulk string)
3. GET missing → $-1 (null)
4. DEL key → :1 (integer)
5. DEL missing → :0
6. Pipeline: SET a 1 + SET b 2 + GET a + GET b
7. Overwrite: SET key old + SET key new → GET returns new

## Limitations

- TCP transport requires NetD Ring 3 (booting in QEMU)
- The test script simulates RESP commands against the store directly (no socket dependency)
- Full HTTP frontend is planned for v1.1
