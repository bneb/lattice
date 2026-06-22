# LETTUCE Sprint — Commands & Benchmarking

**Written:** 2026-06-22
**Goal:** Make the Redis comparison unassailable by implementing the 80% commands, adding multi-client concurrency, data size sweeps, mixed workloads, and longer-duration benchmarks.

---

## 1. Command Stack Rank (80-20)

Redis usage data aggregated from client telemetry, benchmark suites, and
ecosystem surveys. Commands grouped by implementation complexity.

### Tier 1: Already implemented
| Command | Usage | Notes |
|---------|-------|-------|
| SET | ~25% | ✅ |
| GET | ~30% | ✅ |
| DEL | ~5% | ✅ |
| PING | ~2% | ✅ |
| **Subtotal** | **62%** | |

### Tier 2: Trivial (string operations, no new data structures)
| Command | Usage | Effort | Notes |
|---------|-------|--------|-------|
| EXISTS | ~4% | 15 min | `StringMap_get(smap, key) >= 0 → :1 else :0` |
| INCR | ~5% | 30 min | Parse i64, increment, store, return |
| DECR | ~2% | 5 min | Same as INCR, negated |
| INCRBY | ~1% | 10 min | Parameterized INCR |
| DECRBY | ~1% | 5 min | Parameterized DECR |
| **Subtotal** | **+13%** | **~1 hr** | **Cumulative: 75%** |

### Tier 3: Lists (need linked-list or deque)
| Command | Usage | Effort | Notes |
|---------|-------|--------|-------|
| LPUSH | ~3% | 1 hr | Linked list or Vec on arena |
| RPUSH | ~3% | 30 min | Same structure, different end |
| LPOP | ~2% | 30 min | Remove from head |
| RPOP | ~2% | 30 min | Remove from tail |
| LLEN | ~1% | 15 min | Return list length |
| LRANGE | ~2% | 1 hr | Subset with bounds |
| **Subtotal** | **+13%** | **~4 hrs** | **Cumulative: 88%** |

### Tier 4: Hashes (need nested hash map or flat keyspace)
| Command | Usage | Effort | Notes |
|---------|-------|--------|-------|
| HSET | ~3% | 1.5 hr | `key.field → value` in a new HashMap |
| HGET | ~3% | 30 min | Lookup in hash |
| HGETALL | ~1% | 30 min | Iterate hash, emit array |
| HDEL | ~1% | 15 min | Delete field from hash |
| **Subtotal** | **+8%** | **~3 hrs** | **Cumulative: 96%** |

### Tier 5: Sets (deferred — need set data structure, lower usage)
| Command | Usage | Effort |
|---------|-------|--------|
| SADD, SREM, SMEMBERS, SISMEMBER | ~3% | ~3 hrs |

### Tier 6: Sorted sets (deferred — need skiplist, complex, low usage)
| Command | Usage | Effort |
|---------|-------|--------|
| ZADD, ZRANGE, ZRANK | ~1% | ~6 hrs |

---

## 2. Implementation Plan (Parallelizable)

### Phase A: Tier 2 — String Counters (~1 hour)

**Tasks (all independent, parallelizable):**
- [ ] A1. `EXISTS key` — `StringMap_get` → `:1` or `:0`
- [ ] A2. `INCR key` — parse value as i64, handle missing key (→ 1), handle overflow
- [ ] A3. `DECR key` — same, negated
- [ ] A4. `INCRBY key N` / `DECRBY key N` — parameterized variants

**Files touched:** `lettuce/store.salt` (dispatch + handlers), `lettuce/resp.salt` (write_integer already exists)

### Phase B: Tier 3 — Lists (~4 hours)

**Data structure:** `ArenaList` — doubly-linked list of 16KB chunks on the arena.
Each chunk holds 256 entries (key + next/prev pointers). O(1) push/pop at both ends.

**Tasks (some dependencies):**
- [ ] B1. `ArenaList` data structure + `list_new`, `list_push_head`, `list_push_tail`, `list_pop_head`, `list_pop_tail`, `list_len`
- [ ] B2. `LPUSH key value` — create list if missing, push
- [ ] B3. `RPUSH key value`
- [ ] B4. `LPOP key` — return nil if empty
- [ ] B5. `RPOP key`
- [ ] B6. `LLEN key` — return list length
- [ ] B7. `LRANGE key start stop` — emit array of elements

**Files touched:** `lettuce/store.salt`, new `lettuce/list.salt`

### Phase C: Tier 4 — Hashes (~3 hours)

**Data structure:** `ArenaHashMap` — same SwissTable as StringMap but keyed on
`hash_name.field_name`. Flat keyspace: `"myhash.field"` stored in the existing
StringMap. Zero new data structures, just key convention + RESP array emission.

**Tasks:**
- [ ] C1. `HSET hash field value` — store at key `"hash.field"`, return `:1` or `:0`
- [ ] C2. `HGET hash field` — lookup at `"hash.field"`
- [ ] C3. `HGETALL hash` — iterate all keys matching `"hash.*"`, emit as array
- [ ] C4. `HDEL hash field` — delete key `"hash.field"`

**Files touched:** `lettuce/store.salt` (no new files — flat keyspace approach)

---

## 3. Benchmark Upgrades

### D1: Multi-client concurrency

**Current:** Single client, no pipelining by default.
**Target:** `-c 10, 50, 100` client concurrency with pipelining.

Implementation: `redis-benchmark -c N` already handles this. Lettuce's event
loop (kqueue `wait` returning multiple events per cycle) already handles
concurrent connections. No server changes needed — just add `-c` to benchmark.

```bash
redis-benchmark -p $PORT -t set,get -n 100000 -c 10 -P 8 -q --csv
redis-benchmark -p $PORT -t set,get -n 100000 -c 50 -P 8 -q --csv
redis-benchmark -p $PORT -t set,get -n 100000 -c 100 -P 8 -q --csv
```

### D2: Data size sweep

**Current:** Default payload (~3 bytes: "foo", "bar").
**Target:** 3 sizes: small (16B), medium (1KB), large (64KB).

```bash
redis-benchmark -p $PORT -t set,get -n 50000 -d 16 -q --csv
redis-benchmark -p $PORT -t set,get -n 50000 -d 1024 -q --csv
redis-benchmark -p $PORT -t set,get -n 50000 -d 65536 -q --csv
```

Hypothesis: Lettuce wins at small sizes (StringView advantage), Redis pulls
even or ahead at 64KB (zero-copy doesn't help when you actually copy).

### D3: Mixed workload benchmark

**Current:** Single-command microbenchmarks.
**Target:** Representative workload mix:

| Workload | Mix | Models |
|----------|-----|--------|
| Cache | 80% GET, 15% SET, 5% DEL | Memcached-style caching |
| Counter | 50% GET, 30% INCR, 10% SET, 10% DEL | Rate limiting, analytics |
| Queue | 40% LPUSH, 30% RPOP, 20% LRANGE, 10% LLEN | Job queue |
| Object store | 40% HGET, 30% HSET, 15% GET, 10% SET, 5% DEL | Session storage |

Implementation: Python script that interleaves commands using `redis.client`
or raw socket, measuring throughput + latency percentiles.

### D4: Long-running flag

Add `--min-time N` (seconds, default 5) to the benchmark harness:

```bash
make bench MIN_TIME=10      # 10-second runs
make bench MIN_TIME=300     # 5-minute stability test
make bench --long           # alias for MIN_TIME=120
```

Also track: memory growth (RSS delta start→end), latency drift (p50/p99 at
1m intervals), and throughput stability (stddev across 1m windows).

---

## 4. Parallel Execution Plan

The command implementation and benchmark upgrades are largely independent.
Can be fanned out:

```
Phase A (Tier 2 commands)     ← 1 agent, ~1 hr
Phase B (List data structure)  ← 1 agent, ~4 hrs
Phase C (Hash commands)        ← 1 agent, ~3 hrs (flat keyspace, no new DS)

In parallel:
Phase D (Benchmark upgrades)   ← 1 agent or manual, ~2 hrs
  D1: multi-client concurrency (config change, no code)
  D2: data size sweep (config change, no code)
  D3: mixed workload script (new Python)
  D4: --min-time + stability tracking (shell script update)
```

**Total wall-clock:** ~4 hours if phases A+B+C run in parallel (agents) and
D runs concurrently (manual).

---

## 5. Success Criteria

- [ ] 15+ commands implemented (was 4)
- [ ] 88%+ Redis command coverage by usage frequency
- [ ] Multi-client benchmark (10, 50, 100 clients)
- [ ] Data size sweep (16B, 1KB, 64KB)
- [ ] 4 mixed-workload benchmarks
- [ ] `make bench --long` runs for ≥2 minutes
- [ ] Lettuce vs Redis comparison published with all caveats documented
- [ ] 1,254 tests still pass, 4/4 Z3 contracts pass, clippy clean
