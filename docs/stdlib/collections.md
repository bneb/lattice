# Collections

## `std.collections.Vec<T, A>`

Dynamic array parameterized by element type `T` and allocator `A`. Grows by doubling when full.

```salt
use std.collections.vec.Vec
use std.mem.allocator.HeapAllocator
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(allocator: A, cap_hint: i64) -> Vec<T, A>` | Create vector with allocator and capacity hint |
| `push` | `(&mut self, T) -> ()` | Append element (may grow) |
| `pop` | `(&mut self) -> T` | Remove and return last element |
| `get` | `(&self, i64) -> T` | Access by index |
| `set` | `(&mut self, i64, T) -> ()` | Write by index |
| `len` | `(&self) -> i64` | Number of elements |
| `capacity` | `(&self) -> i64` | Allocated capacity |
| `is_empty` | `(&self) -> bool` | True if len == 0 |
| `clear` | `(&mut self) -> ()` | Remove all elements (retains capacity) |
| `as_ptr` | `(&self) -> Ptr<T>` | Raw pointer to data |
| `free` | `(self) -> ()` | Deallocate backing memory |
| `iter` | `(&self) -> VecIter<T>` | Create element iterator |

**Usage:**
```salt
use std.collections.vec.Vec
use std.mem.allocator.HeapAllocator

let alloc = HeapAllocator {};
let mut v = Vec::<i64, HeapAllocator>::new(alloc, 8);
v.push(10);
v.push(20);
v.push(30);
let second = v.get(1);  // 20
let last = v.pop();     // 30
```

## `std.collections.HashMap<K, V>` (Swiss-Table)

Open-addressing hash map using the Swiss-table algorithm. 8-wide SIMD probes for cache-friendly lookup.

```salt
use std.collections.hash_map.HashMap
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `() -> HashMap<K, V>` | Create empty map (zero capacity) |
| `with_capacity` | `(i64) -> HashMap<K, V>` | Pre-allocate with capacity |
| `insert` | `(&mut self, K, V) -> ()` | Insert key-value pair |
| `get` | `(&self, &K) -> i64` | Look up by key, returns value (0 if missing) |
| `remove` | `(&mut self, &K) -> bool` | Remove key, returns true if found |
| `len` | `(&self) -> i64` | Number of entries |
| `is_empty` | `(&self) -> bool` | True if len == 0 |
| `iter` | `(&self) -> HashMapIter<K, V>` | Iterate over entries |

**Usage:**
```salt
use std.collections.hash_map.HashMap

let mut map = HashMap::new::<i64, i64>();
map.insert(10, 100);
map.insert(20, 200);

let val = map.get(&10);  // 100 (0 if missing)

for entry in map.iter() {
    // entry.key, entry.val
}
```

**Entry iterator:**
| Field | Type | Description |
|-------|------|-------------|
| `key` | `K` | The key |
| `val` | `V` | The value |

## `std.collections.Slab<T>`

Pre-allocated object pool with stable indices. O(1) slot access with Z3-verified bounds.

```salt
use std.collections.slab.Slab
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(i32) -> Slab<T>` | Allocate slab with capacity |
| `get` | `(&self, i32) -> &mut T` | O(1) mutable slot access (Z3: index < capacity) |
| `get_ref` | `(&self, i32) -> &T` | O(1) immutable slot access |
| `reset` | `(&self, i32) -> ()` | Zero-fill a slot |
| `cap` | `(&self) -> i32` | Pre-allocated capacity |
| `drop` | `(&mut self) -> ()` | Free underlying allocation |

**Usage:**
```salt
use std.collections.slab.Slab

let slab = Slab::<i64>::new(1000);
slab.reset(42);
let val = slab.get(42);
*val = 7;
```

## `std.collections.StringMap`

Swiss-table specialized for `StringView` keys and values. SoA (Structure of Arrays) layout with inline arena storage. Uses static-function pattern (not method calls).

```salt
use std.collections.string_map
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `StringMap_new` | `() -> Ptr<StringMap>` | Create empty map |
| `StringMap_with_capacity` | `(i64) -> Ptr<StringMap>` | Create with pre-allocated capacity |
| `StringMap_get` | `(Ptr<StringMap>, StringView) -> i64` | Look up key, returns slot index or -1 |
| `StringMap_value_at` | `(Ptr<StringMap>, i64) -> StringView` | Get value StringView at slot |
| `StringMap_set` | `(Ptr<StringMap>, StringView, StringView) -> ()` | Insert or overwrite key-value pair |
| `StringMap_del` | `(Ptr<StringMap>, StringView) -> bool` | Delete key, returns true if found |
| `StringMap_length` | `(Ptr<StringMap>) -> i64` | Number of entries |
| `StringMap_is_empty` | `(Ptr<StringMap>) -> bool` | True if empty |
| `StringMap_drop` | `(Ptr<StringMap>) -> ()` | Free all allocations |

**Usage:**
```salt
use std.collections.string_map.{StringMap_new, StringMap_set, StringMap_get, StringMap_value_at}

let sm = StringMap_with_capacity(16);
StringMap_set(sm, "temperature", "72");
let slot = StringMap_get(sm, "temperature");
if slot >= 0 {
    let val = StringMap_value_at(sm, slot);
}
```
