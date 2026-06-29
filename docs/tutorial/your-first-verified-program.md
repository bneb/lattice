# Your First Verified Salt Program

**Goal:** Write a verified key-value store in Salt in under 15 minutes.

This tutorial assumes you've [installed Salt](/docs/tutorial/README.md#prerequisites). Every code sample is copy-pasteable.

---

## Step 1: Hello, Verified World (2 min)

Create a file called `kv.salt`:

```salt
package main

import std.core.result.Result

fn main() -> i32 {
    let store = new_store(16);
    put(&store, "hello", 42);
    let val = get(&store, "hello");
    return 0;
}
```

Compile and run:

```bash
salt-front kv.salt --lib --disable-alias-scopes -o /tmp/kv
```

It fails. We haven't defined `new_store`, `put`, or `get` yet. Let's add them.

---

## Step 2: The Data Structure (3 min)

We'll use a fixed-size array of key-value pairs with linear search. Add above `main`:

```salt
struct KeyValue {
    key: StringView,
    value: i32,
    occupied: bool,
}

struct Store {
    entries: [KeyValue; 16],
}
```

Now `new_store` — a constructor that zero-initializes the array:

```salt
fn new_store(capacity: i32) -> Store
    requires(capacity > 0 && capacity <= 16)
{
    return Store {
        entries: [
            KeyValue { key: "", value: 0, occupied: false }; 16
        ],
    };
}
```

The `requires` clause tells Z3: "prove that every call site passes a valid capacity." We'll see this in action soon.

---

## Step 3: Insert (4 min)

`put` finds the first empty slot and writes the key-value pair:

```salt
fn put(store: &Store, key: StringView, value: i32)
    requires(key.length() > 0)
{
    let mut i: i64 = 0;
    while i < 16 {
        if !store.entries[i].occupied {
            store.entries[i].key = key;
            store.entries[i].value = value;
            store.entries[i].occupied = true;
            return;
        }
        i = i + 1;
    }
}
```

The `requires(key.length() > 0)` contract means Z3 will reject any call site where it can prove the key might be empty.

---

## Step 4: Lookup (3 min)

`get` searches for a matching key and returns a `Result`:

```salt
fn get(store: &Store, key: StringView) -> Result<i32>
    requires(key.length() > 0)
{
    let mut i: i64 = 0;
    while i < 16 {
        if store.entries[i].occupied {
            if store.entries[i].key == key {
                return Result::Ok(store.entries[i].value);
            }
        }
        i = i + 1;
    }
    return Result::Err(Status::from_code(-1));
}
```

---

## Step 5: Verify It Works (2 min)

Update `main` to exercise the store and print results:

```salt
fn main() -> i32 {
    let store = new_store(16);

    put(&store, "hello", 42);
    put(&store, "world", 99);

    let v1 = get(&store, "hello");
    let v2 = get(&store, "world");
    let v3 = get(&store, "missing");

    match v1 {
        Result::Ok(val) => println(f"hello = {val}"),
        Result::Err(_) => println("hello: not found"),
    }
    match v2 {
        Result::Ok(val) => println(f"world = {val}"),
        Result::Err(_) => println("world: not found"),
    }
    match v3 {
        Result::Ok(val) => println(f"missing = {val}"),
        Result::Err(_) => println("missing: not found (expected)"),
    }

    return 0;
}
```

Compile and run:

```bash
salt-front kv.salt --lib --disable-alias-scopes -o /tmp/kv && /tmp/kv
```

Output:
```
hello = 42
world = 99
missing: not found (expected)
```

---

## Step 6: See Z3 in Action (3 min)

Now let's see Z3 reject a contract violation. Change the `new_store` call in `main`:

```salt
let store = new_store(0);  // violates requires(capacity > 0)
```

Compile:

```bash
salt-front kv.salt --lib --disable-alias-scopes -o /tmp/kv
```

The compiler reports:

```
VERIFICATION ERROR: could not prove '(capacity > 0 && capacity <= 16)'
  context: precondition check at call site
  counterexample:
    capacity = 0
  hint: the argument 'capacity' must be positive and <= 16
```

Z3 found a counterexample (`capacity = 0`) that violates the contract. This is a **compile error** — the binary is never produced. No runtime crash, no UB.

Change it back to `new_store(16)` before continuing.

---

## Step 7: Add Postconditions (2 min)

`requires` proves inputs. `ensures` proves outputs. Let's add an `ensures` to `get`:

```salt
fn get(store: &Store, key: StringView) -> Result<i32>
    requires(key.length() > 0)
    ensures(match_result_is_valid(result, key, store))
```

The `ensures` clause says: "prove that whatever this function returns, it satisfies `match_result_is_valid`."

Define the predicate:

```salt
fn match_result_is_valid(result: Result<i32>, key: StringView, store: &Store) -> bool {
    match result {
        Result::Ok(val) => key_exists_in_store(key, val, store),
        Result::Err(_) => true,  // not found is always valid
    }
}

fn key_exists_in_store(key: StringView, val: i32, store: &Store) -> bool {
    let mut i: i64 = 0;
    while i < 16 {
        if store.entries[i].occupied {
            if store.entries[i].key == key {
                return store.entries[i].value == val;
            }
        }
        i = i + 1;
    }
    return false;  // key not found — should have returned Err
}
```

---

## Step 8: The Full Picture

Here's the complete verified key-value store:

```salt
package main

import std.core.result.Result
import std.status.Status

struct KeyValue {
    key: StringView,
    value: i32,
    occupied: bool,
}

struct Store {
    entries: [KeyValue; 16],
}

fn new_store(capacity: i32) -> Store
    requires(capacity > 0 && capacity <= 16)
{
    return Store {
        entries: [
            KeyValue { key: "", value: 0, occupied: false }; 16
        ],
    };
}

fn put(store: &Store, key: StringView, value: i32)
    requires(key.length() > 0)
{
    let mut i: i64 = 0;
    while i < 16 {
        if !store.entries[i].occupied {
            store.entries[i].key = key;
            store.entries[i].value = value;
            store.entries[i].occupied = true;
            return;
        }
        i = i + 1;
    }
}

fn get(store: &Store, key: StringView) -> Result<i32>
    requires(key.length() > 0)
{
    let mut i: i64 = 0;
    while i < 16 {
        if store.entries[i].occupied {
            if store.entries[i].key == key {
                return Result::Ok(store.entries[i].value);
            }
        }
        i = i + 1;
    }
    return Result::Err(Status::from_code(-1));
}

fn main() -> i32 {
    // Z3 proves: 16 > 0 && 16 <= 16 ✓
    let store = new_store(16);

    // Z3 proves: "hello".length() > 0
    put(&store, "hello", 42);
    put(&store, "world", 99);

    let v1 = get(&store, "hello");
    let v2 = get(&store, "world");
    let v3 = get(&store, "missing");

    match v1 {
        Result::Ok(val) => println(f"hello = {val}"),
        Result::Err(_) => println("hello: not found"),
    }
    match v2 {
        Result::Ok(val) => println(f"world = {val}"),
        Result::Err(_) => println("world: not found"),
    }
    match v3 {
        Result::Ok(val) => println(f"missing = {val}"),
        Result::Err(_) => println("missing: not found (expected)"),
    }

    return 0;
}
```

---

## What You Just Built

| Concept | How It's Used |
|---------|---------------|
| **`requires`** | `new_store` proves capacity bounds; `put`/`get` prove non-empty keys |
| **`ensures`** | `get` proves the returned value matches the stored value (if found) |
| **Z3 proof** | Every compile-time-known argument (like `16`, `"hello"`) has its contract proved and elided — zero runtime cost |
| **Compile error** | Violating a contract (like `new_store(0)`) is a compile error with a counterexample — you can't ship a provably-broken contract |

---

## Next Steps

- Read [Chapter 9: Z3 Contracts](09-contracts.md) for the full verification model
- Try adding `ensures` clauses to `put` — what postcondition makes sense?
- Extend the store to support update (overwrite an existing key) — what contracts does that require?
- Check out the [Lettuce key-value server](../../lettuce/) — a production KV store with Z3-verified operations
