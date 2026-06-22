# LETTUCE Tutorial — Build a Verified Redis Server in Salt

This tutorial walks through building and verifying LETTUCE, a Redis-compatible
server written in [Salt](https://github.com/bneb/lattice). By the end, you will
understand how Salt's Z3 contracts prove properties of your code at compile time,
and how to use them in your own programs.

**Time:** ~45 minutes
**Prerequisites:** Rust 1.75+, Z3 4.12+ (`brew install z3`), LLVM 21+

---

## 1. Setup (5 min)

```bash
git clone https://github.com/bneb/lattice.git
cd lattice
make build          # Build the Salt compiler (first time: ~2 min)
make lettuce        # Verify contracts + compile LETTUCE
```

If `make lettuce` succeeds, you will see:

```
  resp_contracts: PASS (Z3 contracts verified)
  aof_contracts:  PASS (Z3 contracts verified)
  store_module:   PASS
✅ MLIR compiled successfully.
```

---

## 2. Run the Server (3 min)

```bash
make lettuce-verify    # Run verification tests
```

LETTUCE listens on port 6379. Open a second terminal and test:

```bash
redis-cli -p 6379 PING          # → PONG
redis-cli -p 6379 SET foo bar   # → OK
redis-cli -p 6379 GET foo       # → bar
redis-cli -p 6379 INCR counter  # → 1
redis-cli -p 6379 INCR counter  # → 2
redis-cli -p 6379 DEL foo       # → 1
```

The server implements 9 Redis commands: PING, SET, GET, DEL, EXISTS, INCR, DECR,
INCRBY, DECRBY. All without `malloc` on the hot path.

---

## 3. Read a Contract (5 min)

Open `lettuce/aof.salt`. This is the append-only file persistence layer. Look
at the `Aof_append_set` function:

```salt
pub fn Aof_append_set(ctx: Ptr<AofContext>, key: StringView, val: StringView)
    requires(!ctx.is_null())
    requires(key.length() > 0 && key.length() <= 4000)
    requires(val.length() > 0 && val.length() <= 4000)
{
    // ... append SET command to AOF buffer ...
}
```

The `requires` clauses are Z3 contracts. Before the function body executes, the
compiler translates each `requires` into a Z3 formula and checks whether it can
be violated.

If Z3 says **UNSAT** (the negation is impossible): the condition always holds.
The runtime check is elided — zero instructions emitted.

If Z3 says **SAT** (there exists a counterexample): the compiler emits a warning
with the specific values that would violate the contract.

If Z3 **times out** (100ms default): the compiler inserts a runtime assertion as
a safe fallback. Your program still compiles, still runs, still has defined
behavior.

---

## 4. Write Your Own Contract (10 min)

Open `lettuce/store.salt`. Find the `handle_set` function. It stores a
key-value pair in the hash map. The function takes the input buffer
(`elems: StringView`) and the parsed first argument (`arg0: RespValue`).

Let's add a contract: **the input buffer must contain at least enough data
for a valid RESP command.**

Add this line after the opening brace of `handle_set`:

```salt
    requires(elems.length() > 0)
```

The function should now look like:

```salt
fn handle_set(smap: Ptr<StringMap>, elems: StringView, arg0: RespValue,
              consumed: i64, out_buf: Ptr<u8>) -> ExecResult {
    requires(elems.length() > 0)
    let rest1 = elems.slice(arg0.bytes_consumed, elems.length());
    // ... rest of function ...
}
```

Recompile with verification:

```bash
salt-front/target/release/salt-front lettuce/store.salt --lib --verify \
  -o /tmp/store_verified.mlir
```

Output:
```
✅ MLIR compiled successfully.
```

The contract passes. Z3 proves that `handle_set` is never called with an empty
buffer — the command dispatcher in `execute()` validates input before calling
any handler. Since the proof succeeds, the runtime check is elided: zero
instructions emitted.

---

## 5. Break a Contract (10 min)

Now let's see what happens when Z3 finds a violation. Change the contract to
something that cannot be proven from the call site:

```salt
    requires(elems.length() > 1000000)   // buffer unlikely to be this large
```

Recompile:

```bash
salt-front/target/release/salt-front lettuce/store.salt --lib --verify \
  -o /tmp/store_verified.mlir 2>&1 | head -20
```

You should see output indicating the contract could not be proven. Z3 may
report a counterexample or timeout. The compiler will emit a runtime assertion
as a safe fallback — the program still compiles and runs correctly.

Change the contract back:

```salt
    requires(elems.length() > 0)
```

---

## 6. Understand the RESP Parser Contracts (10 min)

Open `lettuce/resp.salt`. The RESP parser uses `StringView::byte_at()`, which
bounds-checks every access. Look at the `find_crlf` function:

```salt
fn find_crlf(&input: &StringView, start: i64) -> i64 {
    let len = input.length();
    if start < 0 || start >= len {
        return -1;
    }
    let mut i = start;
    while i < len - 1 {
        if input.byte_at(i) == 13 && input.byte_at(i + 1) == 10 {
            return i;
        }
        i = i + 1;
    }
    return -1;
}
```

The comment above the call site explains the proof strategy:

```
// Guard: find_crlf requires start < len. With start=1, we need len > 1.
// This makes the bounds check statically provable by Z3.
```

Because the caller passes `start=1` and the input length is known to be >1 at
the call site (it was already parsed as a valid RESP array), Z3 can prove that
`i < len - 1` holds throughout the loop — all `byte_at` accesses are in bounds.
When Z3 proves a bounds check is redundant, the compiler elides it.

This is the core idea behind Salt's verification: write the check once (the
caller validates `len > 1`), let Z3 propagate the proof through the function
(the loop condition implies all indices are safe), and eliminate the runtime
checks that the proof covers.

---

## 7. What Was Verified

At this point, you have seen:

| What | Where | How |
|------|-------|-----|
| Non-null context | `aof.salt:64` | `requires(!ctx.is_null())` |
| Buffer bounds | `aof.salt:137-138` | `requires(key.length() <= 4000)` |
| Parser bounds | `resp.salt:225-226` | Comment-annotated, Z3-provable |
| Argument count | `store.salt` dispatch | `cmd.array_count < N` guards |

All contracts pass `make lettuce-verify`. The verification feedback loop is
sub-second per module.

---

## Next Steps

- **Read the Salt tutorial:** `docs/tutorial/` — 8 chapters covering types,
  functions, structs, generics, arenas, and contracts
- **Read the benchmark results:** `benchmarks/LETTUCE_BENCH.md`
- **Read the architecture doc:** `docs/ARCH.md`
- **Contribute:** `CONTRIBUTING.md`
