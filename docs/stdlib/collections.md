# Collections

## `std.collections.Vec<T, A>`

Dynamic array parameterized by element type `T` and arena allocator `A`. Grows by doubling when full.

```salt
use std.collections.Vec
use std.vec.vec
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `() -> Vec<T>` | Create empty vector |
| `with_capacity` | `(i64) -> Vec<T>` | Pre-allocate capacity |
| `push` | `(&mut self, T) -> ()` | Append element (may grow) |
| `pop` | `(&mut self) -> Option<T>` | Remove and return last element |
| `get` | `(&self, i64) -> T` | Access by index |
| `set` | `(&mut self, i64, T) -> ()` | Write by index |
| `len` | `(&self) -> i64` | Number of elements |
| `cap` | `(&self) -> i64` | Allocated capacity |
| `is_empty` | `(&self) -> bool` | True if len == 0 |
| `clear` | `(&mut self) -> ()` | Remove all elements |

**Usage:**
```salt
let mut v = Vec::new::<i64>();
v.push(10);
v.push(20);
v.push(30);
let second = v.get(1);  // 20
let len = v.len();      // 3
```

## `std.collections.HashMap<K, V>` (Swiss-Table)

Open-addressing hash map using the Swiss-table algorithm. 8-wide SIMD probes for cache-friendly lookup.

```salt
use std.collections.HashMap
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `() -> HashMap<K, V>` | Create empty map |
| `insert` | `(&mut self, K, V) -> ()` | Insert key-value pair |
| `get` | `(&self, K) -> Option<V>` | Look up by key |
| `remove` | `(&mut self, K) -> Option<V>` | Remove key |
| `contains` | `(&self, K) -> bool` | True if key exists |
| `len` | `(&self) -> i64` | Number of entries |
| `iter` | `(&self) -> Iterator<Entry<K,V>>` | Iterate over entries |

**Usage:**
```salt
use std.collections.HashMap

let mut map = HashMap::new::<StringView, i64>();
map.insert("hello", 1);
map.insert("world", 2);

let val = map.get("hello");  // Option::Some(1)

for entry in map.iter() {
    println(f"{entry.key}: {entry.value}");
}
```

**Entry iterator:**
| Field | Type | Description |
|-------|------|-------------|
| `key` | `K` | The key |
| `value` | `V` | The value |

## `std.collections.Slab<T>`

Pre-allocated object pool with stable indices. Key-based access instead of pointer-based.

```salt
use std.collections.Slab
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(i64) -> Slab<T>` | Create with capacity |
| `insert` | `(&mut self, T) -> i64` | Insert and return index |
| `get` | `(&self, i64) -> &T` | Access by index |
| `remove` | `(&mut self, i64) -> Option<T>` | Remove by index |

## `std.collections.StringMap`

Swiss-table specialized for `StringView` keys. Optimized hash function for string data.

```salt
use std.collections.StringMap
```

Same interface as `HashMap<StringView, V>` with a string-optimized hash function.

**Usage:**
```salt
use std.collections.StringMap

let mut sm = StringMap::new::<i64>();
sm.insert("temperature", 72);
sm.insert("pressure", 1013);
```
