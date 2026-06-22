# Salt Standard Library API Reference

The Salt standard library lives in `salt-front/std/`. Modules are organized under the `std` package namespace. Below is every public module, its purpose, and its public API surface.

---

## std.core

Core language primitives -- pointers, memory, ownership, and fundamental types.

### std.core.ptr (`std/core/ptr.salt`)

Generic pointer type backed by a raw `i64` address (lowered to `!llvm.ptr`).

- `Ptr<T>` -- zero-cost pointer to `T`
  - `new() -> Ptr<T>` -- non-null sentinel (address 0x1)
  - `empty() -> Ptr<T>` -- null sentinel (address 0x0)
  - `addr(self) -> u64` -- raw address
  - `from_addr(addr: u64) -> Ptr<T>` -- construct from raw address
  - `offset(self, n: i64) -> Ptr<T>` -- pointer arithmetic (GEP)
  - `index(self, i: i64) -> T` -- index read (GEP + load)
  - `read(self) -> T` -- load value
  - `write(self, val: T)` -- store value
  - `is_null(self) -> bool` -- null check

```salt
let p = Ptr::<i64>::new();
p.write(42);
let v = p.read();
```

### std.core.mem (`std/core/mem.salt`)

Compile-time memory layout primitives.

- `Layout` -- `{ size, align }`
  - `new<T>() -> Layout`
  - `array<T>(n: i64) -> Layout`
  - `pad_to_align(&self) -> Layout`
- `size_of<T>() -> i64` -- compile-time size
- `align_of<T>() -> i64` -- compile-time alignment
- `zeroed<T>() -> T`
- `swap<T>(a: &mut T, b: &mut T)`

### std.core.alloc (`std/core/alloc.salt`)

Allocator trait with Z3-verified contracts.

- **Trait `Allocator`** -- `alloc`, `dealloc`, `realloc`

### std.core.arena (`std/core/arena.salt`)

Global arena allocator with mark/reset pattern (O(1) reclamation).

- `Arena`
  - `new(capacity: i64) -> Arena`
  - `alloc<T>(self, val: T) -> Ptr<T>`
  - `alloc_bytes(self, size: i64) -> Ptr<u8>`
  - `alloc_array<T>(self, count: i64) -> Ptr<T>`
  - `mark(self) -> i64`
  - `reset_to(self, m: i64)`
- `alloc(size: i64) -> i64` -- raw arena allocation
- `mark() -> i64`
- `reset_to(m: i64)`

### std.core.option (`std/core/option.salt`)

Optional value type.

- `Option<T>` -- enum `Some(T)` / `None`
  - `is_some(self) -> bool`
  - `is_none(self) -> bool`
  - `unwrap(self) -> T` -- panics on None
  - `unwrap_or(self, default: T) -> T`
  - `ok_or(self, status: Status) -> Result<T>`

### std.core.result (`std/core/result.salt`)

Fallible operation result paired with Status error codes.

- `Result<T>` -- enum `Ok(T)` / `Err(Status)`
  - `is_ok(self) -> bool`
  - `is_err(self) -> bool`
  - `unwrap(self) -> T`
  - `status(self) -> Status`
  - `unwrap_or(self, default: T) -> T`
  - `drop(self)`

### std.core.str (`std/core/str.salt`)

Zero-copy string view (like C++ `std::string_view` or Rust `&str`).

- `StringView` -- `{ ptr: Ptr<u8>, len: i64 }`
  - `from_raw(ptr: Ptr<u8>, len: i64) -> StringView`
  - `empty() -> StringView`
  - `length(&self) -> i64`
  - `is_empty(&self) -> bool`
  - `byte_at(&self, idx: i64) -> u8`
  - `slice(&self, start: i64, end: i64) -> StringView`
  - `find_byte(&self, needle: u8) -> i64`
  - `split_at_byte(&self, delim: u8, before: &mut StringView) -> StringView`
  - `trim(&self) -> StringView`
  - `starts_with_byte(&self, b: u8) -> bool`
  - `eq_bytes(&self, other: &StringView) -> bool`
  - `eq3(&self, a: u8, b: u8, c: u8) -> bool`
  - `eq4(&self, a: u8, b: u8, c: u8, d: u8) -> bool`

```salt
let sv = StringView::from_raw("hello", 5);
let len = sv.length();
let sub = sv.slice(0, 2);
```

### std.core.slice (`std/core/slice.salt`)

Generic fat pointer: non-owning view over contiguous memory.

- `Slice<T>` -- `{ data: Ptr<T>, len: i64 }`
  - `new(ptr: Ptr<T>, len: i64) -> Slice<T>`
  - `at(self, index: i64) -> T` -- verified read (Z3)
  - `set(self, index: i64, val: T)` -- verified write (Z3)
  - `sub(self, start: i64, end: i64) -> Slice<T>`
  - `len(self) -> i64`
  - `is_empty(self) -> bool`
  - `as_ptr(self) -> Ptr<T>`
  - `offset(self, n: i64) -> Slice<T>`

### std.core.boxed (`std/core/boxed.salt`)

Heap-allocated box wrapper (replaces raw `malloc` + cast + write).

- `Box<T>` -- owned heap allocation
  - `new(val: T) -> Box<T>`
  - `as_ptr(self) -> Ptr<T>`
  - `read(self) -> T`
  - `write(self, val: T)`
  - `drop(self)`

### std.core.buffer (`std/core/buffer.salt`)

Zero-cost buffer view with direct GEP lowering for contiguous memory.

- `Buffer<T>` -- `{ data: u64, len: i64 }`
  - `from_raw(ptr: u64, len: i64) -> Buffer<T>`
  - `len(&self) -> i64`
  - `as_ptr(&self) -> u64`
  - `get(&self, index: i64) -> T` -- verified read (Z3)
  - `set(&mut self, index: i64, value: T)` -- verified write (Z3)

### std.core.clone (`std/core/clone.salt`)

Clone trait for producing copies.

- **Trait `Clone`** -- `fn clone(&self) -> Self`
- Implementations for all primitives: `i8`-`i64`, `u8`-`u64`, `usize`, `f32`, `f64`, `bool`

### std.core.conv (`std/core/conv.salt`)

Integer to decimal ASCII conversion.

- `ITOA_MIN_BUF_SIZE: i64` = 24
- `itoa(n: i64, buf: Ptr<u8>, buf_len: i64) -> i64`

### std.core.drop (`std/core/drop.salt`)

RAII cleanup trait.

- **Trait `Drop`** -- `fn drop(&mut self)`

### std.core.fmt (`std/core/fmt.salt`)

Structural formatting infrastructure -- Formatter passed to user `fmt()` implementations.

- `Formatter` -- `{ data: Ptr<u8>, len: i64, cap: i64 }`
  - `new() -> Formatter`
  - `write(&mut self, s: &u8, len: i64)`
  - `write_i64(&mut self, val: i64)`
  - `write_i32(&mut self, val: i32)`
  - `write_bool(&mut self, val: bool)`
  - `len(&self) -> i64`
  - `as_ptr(&self) -> Ptr<u8>`

### std.core.iter (`std/core/iter.salt`)

Iterator combinators: Range, Filter, Map.

- `Range` -- `{ current: i64, end: i64 }`
  - `new(start: i64, end: i64) -> Range`
  - `next(&mut self) -> Option<i64>`
  - `filter<F>(self, predicate: F) -> Filter<Range, F, i64>`
  - `map<F, U>(self, func: F) -> Map<Range, F, U>`
  - `fold<A, F>(&mut self, init: A, f: F) -> A`
  - `sum(&mut self) -> i64`
  - `count(&mut self) -> i64`
  - `any<F>(&mut self, pred: F) -> bool`
  - `all<F>(&mut self, pred: F) -> bool`
- `Filter<I, F, T>` -- `next`, `filter`, `map`, `fold`, `sum`, `count`, `any`, `all`
- `Map<I, F, U>` -- `next`, `filter`, `map`, `fold`, `sum`, `count`, `any`, `all`

```salt
let sum = Range::new(0, 10).filter(fn (x) { x % 2 == 0 }).sum();
```

### std.core.node_ptr (`std/core/node_ptr.salt`)

Safe pointer wrapper for node-like allocations (data structures).

- `NodePtr<T>`
  - `null() -> NodePtr<T>`
  - `is_null(&self) -> bool`
  - `is_valid(&self) -> bool`
  - `as_ref(&self) -> &T`
  - `as_mut(&mut self) -> &mut T`
  - `from_ref(r: &T) -> NodePtr<T>`
  - `from_mut(r: &mut T) -> NodePtr<T>`
  - `from_raw(addr: i64) -> NodePtr<T>`
  - `as_raw(&self) -> i64`
  - `as_llvm_ptr(&self) -> !llvm.ptr`
  - `from_llvm_ptr(ptr: !llvm.ptr) -> NodePtr<T>`

### std.core.slab_alloc (`std/core/slab_alloc.salt`)

Pre-allocated slab allocator with size classes (64/512/4096 bytes) and a BumpAlloc fallback. Global `GLOBAL_ALLOC` instance.

- `BumpAlloc` -- 128 MB bump allocator
- `GlobalSlabAlloc`
- `alloc(layout: Layout) -> Result<Ptr<u8>>`
- `dealloc(ptr: Ptr<u8>, layout: Layout)`

---

## std.collections

Heap-allocated and arena-backed data structures.

### std.collections.vec (`std/collections/vec.salt`)

Generic dynamic array with pluggable allocator (HeapAllocator or ArenaAllocator).

- `Vec<T, A>` -- `{ data: Ptr<T>, len: i64, cap: i64, allocator: A }`
  - `new(allocator: A, cap_hint: i64) -> Vec<T, A>`
  - `len(&self) -> i64`
  - `is_empty(&self) -> bool`
  - `capacity(&self) -> i64`
  - `push(&mut self, val: T)`
  - `pop(&mut self) -> T`
  - `get(&self, idx: i64) -> T`
  - `set(&mut self, idx: i64, val: T)`
  - `get_unchecked(&self, idx: i64) -> T`
  - `set_unchecked(&mut self, idx: i64, val: T)`
  - `as_ptr(&self) -> Ptr<T>`
  - `as_raw_ptr(&self) -> i64`
  - `clear(&mut self)`
  - `free(self)`
  - `index(&self, idx: i64) -> T`
  - `index_mut(&mut self, idx: i64, val: T)`
  - `push_byte(&mut self, b: u8)`
  - `iter(&self) -> VecIter<T>`
- `VecIter<T>` -- `next(&mut self) -> Option<T>`

### std.collections.slab (`std/collections/slab.salt`)

Fixed-capacity, pre-allocated index pool with Z3-verified bounds. Ideal for network connection state, ECS, timer wheels.

- `Slab<T>` -- `{ data: Ptr<u8>, capacity: i32, elem_size: i64 }`
  - `new(capacity: i32) -> Slab<T>`
  - `get(&self, index: i32) -> &mut T` -- Z3: index < capacity
  - `get_ref(&self, index: i32) -> &T`
  - `reset(&self, index: i32)` -- zero-fill slot
  - `cap(&self) -> i32`
  - `drop(&mut self)`

```salt
let mut slab = Slab::<Session>::new(10000);
slab.reset(fd);
let session = slab.get(fd);
```

### std.collections.string_map (`std/collections/string_map.salt`)

SwissTable `StringView -> StringView` map with SoA layout and arena storage. Uses FNV-1a hashing.

- `StringMap` -- opaque struct
- `StringMap_new() -> Ptr<StringMap>`
- `StringMap_with_capacity(min_cap: i64) -> Ptr<StringMap>`
- `StringMap_get(smap: Ptr<StringMap>, key: StringView) -> i64` -- slot index or -1
- `StringMap_value_at(smap: Ptr<StringMap>, slot: i64) -> StringView`
- `StringMap_set(smap: Ptr<StringMap>, key: StringView, val: StringView)`
- `StringMap_del(smap: Ptr<StringMap>, key: StringView) -> bool`
- `StringMap_length(smap: Ptr<StringMap>) -> i64`
- `StringMap_is_empty(smap: Ptr<StringMap>) -> bool`
- `StringMap_drop(smap: Ptr<StringMap>)`
- `fnv1a(sv: StringView) -> u64`

### std.collections.hash_map (`std/collections/hash_map.salt`)

Generic SwissTable HashMap with Hash + Eq traits.

- `HashMap<K, V>` -- generic hash map
  - `new() -> HashMap<K, V>`
  - `with_capacity(min_cap: i64) -> HashMap<K, V>`
  - `get(&self, key: &K) -> i64`
  - `insert(&mut self, key: K, val: V)`
  - `remove(&mut self, key: &K) -> bool`
  - `len(&self) -> i64`
  - `is_empty(&self) -> bool`
  - `drop(&mut self)`
  - `iter(&self) -> HashMapIter<K, V>`
- `Entry<K, V>` -- `{ key: K, val: V }`
- `HashMapIter<K, V>` -- `next(&mut self) -> Option<Entry<K, V>>`

---

## std.string (`std/string.salt`)

Owning heap-allocated UTF-8 string with 23-byte inline small-string optimization and f-string support.

- `String` -- `{ data: Ptr<u8>, len: i64, cap: i64, is_inline: bool, inline_buf: [u8; 23] }`
  - `new() -> String`
  - `with_capacity(cap: i64) -> String`
  - `len(&self) -> i64`
  - `is_empty(&self) -> bool`
  - `capacity(&self) -> i64`
  - `push_byte(&mut self, b: u8)`
  - `push_ascii(&mut self, ch: u8)` -- Z3: ch < 128
  - `push(&mut self, ch: i32)` -- Unicode codepoint
  - `byte_at(&self, idx: i64) -> u8`
  - `clear(&mut self)`
  - `push_cstr(&mut self, s: &u8)`
  - `mut_ptr(&mut self) -> Ptr<u8>`
  - `as_ptr(&self) -> Ptr<u8>`
  - `as_view(&self) -> StringView`
  - `from_view(sv: &StringView) -> String`
  - `free(self)`
  - `reserve(&mut self, additional: i64)`
  - `set_len(&mut self, len: i64)`
  - `write_str_unchecked(&mut self, src: &u8, len: i64)`
  - `write_i32_unchecked(&mut self, val: i32)`
  - `write_i64_unchecked(&mut self, val: i64)`
- `InterpolatedStringHandler` -- f-string builder
- `FormattedF64` -- float with precision spec
- `from_literal(s: &u8, len: i64) -> String`
- `fmt_f64(val: f64, precision: i32) -> FormattedF64`

```salt
let mut s = String::new();
s.push(48);        // '0'
s.push_byte(65);   // 'A'
s.push_ascii(88);  // 'X'
let view = s.as_view();
```

---

## std.status (`std/status.salt`)

Canonical gRPC-compatible error codes packed in 8 bytes `{ code: i32, detail: i32 }`.

- `Status` -- `{ code: i32, detail: i32 }`
  - `ok() -> Status`
  - `with_detail(code: i32, detail: i32) -> Status`
  - `from_code(code: i32) -> Status`
  - `cancelled(msg: StringView) -> Status` (one per error code)
  - `is_ok(self) -> bool`
  - `is_err(self) -> bool`
  - `is_not_found(self) -> bool` (one per error code)
- **Constants:** `OK` (0), `CANCELLED` (1), `UNKNOWN` (2), `INVALID_ARGUMENT` (3), `DEADLINE_EXCEEDED` (4), `NOT_FOUND` (5), `ALREADY_EXISTS` (6), `PERMISSION_DENIED` (7), `RESOURCE_EXHAUSTED` (8), `FAILED_PRECONDITION` (9), `ABORTED` (10), `OUT_OF_RANGE` (11), `UNIMPLEMENTED` (12), `INTERNAL` (13), `UNAVAILABLE` (14), `DATA_LOSS` (15), `UNAUTHENTICATED` (16)

---

## std.io

Input/output -- file operations, buffered readers/writers, console, and event reactors.

### std.io (`std/io/mod.salt`)

Console I/O singleton.

- `Console`
  - `print_str(self, s: str) -> i64`
  - `println(self, s: str) -> i64`
  - `print_int(self, n: i64) -> i64`
- `io: Console` -- global singleton

### std.io.print (`std/io/print.salt`)

Type-safe printing via the Display trait.

- `print<T: Display>(value: &T)`
- `println<T: Display>(value: &T)`
- `print_str(s: &u8)`
- `println_str(s: &u8)`
- `print_int(n: i64)`
- `println_int(n: i64)`
- `print_usize(n: usize)`
- `println_usize(n: usize)`

### std.io.file (`std/io/file.salt`)

File I/O with Result-based API, including memory mapping.

- `File` -- `{ fd: i32 }`
  - `open(path: &u8, flags: OpenFlags) -> Result<File>`
  - `close(&self) -> i32`
  - `mmap<T>(&self, len: u64, prot: Prot, flags: MapFlags) -> Result<Ptr<T>>`
  - `read(&self, buf: Ptr<u8>, count: i64) -> Result<i64>`
  - `write(&self, buf: &u8, count: i64) -> Result<i64>`
  - `write_all(&self, buf: &u8, count: i64) -> Result<i64>`
  - `is_valid(&self) -> bool`
  - `fd(&self) -> i32`
- `Prot` -- `PROT_NONE`, `PROT_READ`, `PROT_WRITE`, `PROT_EXEC`
- `MapFlags` -- `MAP_SHARED`, `MAP_PRIVATE`, `MAP_ANONYMOUS`
- `OpenFlags` -- `O_RDONLY`, `O_WRONLY`, `O_RDWR`, `O_CREAT`, `O_TRUNC`
- `mmap_raw(fd: i32, len: u64, prot: i32, flags: i32) -> u64`

### std.io.writer (`std/io/writer.salt`)

Writer protocol trait for zero-allocation f-string I/O.

- **Concept `Writer`**
  - `write_bytes(&mut self, data: Ptr<u8>, len: usize)`
  - `flush(&mut self)`
  - `write_i32(&mut self, val: i32)` -- default impl
  - `write_i64(&mut self, val: i64)` -- default impl
  - `write_f64(&mut self, val: f64)` -- default impl
  - `write_f64_prec(&mut self, val: f64, precision: i32)` -- default impl
  - `write_str(&mut self, s: &u8, len: i64)`
  - `write_bool(&mut self, val: bool)` -- default impl

### std.io.buffered_reader (`std/io/buffered_reader.salt`)

8 KB buffered input reader for efficient file reading.

- `BufferedReader`
  - `new(fd: i32) -> BufferedReader`
  - `read_byte(&mut self) -> i64`
  - `read(&mut self, out: Ptr<u8>, count: i64) -> i64`
  - `read_line(&mut self, out: Ptr<u8>, max_len: i64) -> i64`
  - `has_data(&self) -> bool`
  - `remaining(&self) -> i64`
  - `close(&mut self)`

### std.io.buffered_writer (`std/io/buffered_writer.salt`)

8 KB buffered output writer implementing the Writer protocol.

- `BufferedWriter`
  - `new(fd: i32) -> BufferedWriter`
  - `write_bytes(&mut self, data: &u8, len: i64)`
  - `flush(&mut self)`
  - `remaining(&self) -> i64`
  - `reserve(&mut self, bytes: i64) -> &mut u8`
  - `advance(&mut self, bytes: i64)`
  - `write_str_unchecked(&mut self, src: &u8, len: i64)`
  - `write_i32(&mut self, val: i32)`
  - `write_i64(&mut self, val: i64)`
  - `write_str(&mut self, s: &u8, len: i64)`

### std.io.reactor (`std/io/reactor.salt`)

Backend-agnostic event loop trait (KqueueReactor, EpollReactor, KeuOSReactor).

---

## std.net

Networking -- TCP listener/stream and event polling via zero-trap SPSC ring IPC to NetD.

### std.net.tcp (`std/net/tcp.salt`)

TCP socket operations communicating with the Network Daemon (PID 5) through SPSC rings.

- `TcpListener` -- `{ fd: i32 }`
  - `bind(port: i32) -> Result<TcpListener>`
  - `accept(&self) -> Result<TcpStream>`
  - `fd(&self) -> i32`
  - `close(&self)`
- `TcpStream` -- `{ fd: i32 }`
  - `recv(&self, buf: Ptr<u8>, len: i64) -> i64`
  - `send(&self, buf: Ptr<u8>, len: i64) -> i64`
  - `fd(&self) -> i32`
  - `close(&self)`
- `rx_vaddr(fd: i32) -> u64` -- receive ring address
- `tx_vaddr(fd: i32) -> u64` -- transmit ring address

```salt
let listener = TcpListener::bind(8080).unwrap();
let stream = listener.accept().unwrap();
let bytes = stream.recv(buf, 1024);
```

### std.net.poller (`std/net/poller.salt`)

Userspace event poller (replaces kqueue for KeuOS). Polls SPSC rings for available data.

- `Poller` -- `{ fds: [i32; 64], filters: [i32; 64] }`
  - `new() -> Result<Poller>`
  - `register(&mut self, fd: i32, filter: PollFilter) -> i32`
  - `deregister(&mut self, fd: i32) -> i32`
  - `wait(&self, event_buf: Ptr<i64>, max_events: i32, timeout_ms: i32) -> i32`
- `PollFilter` -- `Read`, `Write`

---

## std.http

HTTP/1.1 zero-copy parsing, response construction, and client operations.

### std.http.parser (`std/http/parser.salt`)

Zero-copy HTTP/1.1 request line parser.

- `parse_request_line(buf: Ptr<u8>, len: i64, method_out: &mut StringView, uri_out: &mut StringView, version_out: &mut StringView) -> i64`

### std.http.response (`std/http/response.salt`)

Zero-copy HTTP response writer (constructs responses directly into a send buffer).

- `write_response(buf: Ptr<u8>, status: i64, content_type: StringView, body: StringView) -> i64`

### std.http.client (`std/http/client.salt`)

HTTP client for GET requests and raw socket operations.

- `Response` -- `{ status_code, body_ptr, body_len, raw_ptr, raw_len }`
  - `status(&self) -> i32`
- `get_raw(host: StringView, port: i32, path: StringView, out_buf: Ptr<u8>, buf_size: i64) -> i64`
- `connect(host: StringView, port: i32) -> i32`
- `send(fd: i32, data: Ptr<u8>, len: i64) -> i64`
- `recv(fd: i32, buf: Ptr<u8>, len: i64) -> i64`
- `close(fd: i32)`

---

## std.fs (`std/fs/fs.salt`)

Filesystem operations via SPSC ring IPC to StoreD (the Storage Daemon, PID 6).

- `VfsConnection` -- `{ cmd_ring, comp_ring, seq }`
  - `exists(&mut self, path: &u8) -> bool`
  - `create_dir(&mut self, path: &u8) -> Result<i32>`
  - `remove_file(&mut self, path: &u8) -> Result<i32>`
  - `open(&mut self, path: &u8) -> Result<FileHandle>`
  - `read(&mut self, handle: &FileHandle, buffer: Ptr<u8>, size: u64) -> Result<u64>`
  - `write(&mut self, handle: &FileHandle, buffer: Ptr<u8>, size: u64) -> Result<u64>`
  - `close(&mut self, handle: &FileHandle) -> Result<i32>`
- `FileHandle` -- `{ fd: u64 }`
- `vfs_connect() -> VfsConnection`

```salt
let mut vfs = vfs_connect();
let handle = vfs.open("/data/config").unwrap();
let bytes = vfs.read(&handle, buf, 4096).unwrap();
vfs.close(&handle);
```

---

## std.process

Subprocess execution and kernel-level process creation.

### std.process (`std/process/process.salt`)

Subprocess execution via posix_spawn.

- `Command` -- builder pattern
  - `new(program: Ptr<u8>) -> Command`
  - `arg1(self, a: Ptr<u8>) -> Command`
  - `arg2(self, a: Ptr<u8>) -> Command`
  - `arg3(self, a: Ptr<u8>) -> Command`
  - `execute(self) -> i32`
  - `capture(self, out_buf: Ptr<u8>, buf_size: i64) -> i64`

### std.process.spawn (`std/process/spawn.salt`)

Kernel-level declarative process creation manifest (KeuOS).

- `SpawnManifest` -- `{ elf_vaddr: u64, namespace_flags: u64 }`
  - `new(elf_vaddr: u64) -> SpawnManifest`
- **Constants:** `NS_ISOLATE_FS`, `NS_ISOLATE_NET`, `NS_ISOLATE_IPC`

---

## std.sync

Thread synchronization primitives.

### std.sync.sync (`std/sync/sync.salt`)

Mutex and atomic operations backed by pthreads and C11 atomics.

- `Mutex` -- `{ handle: i64 }`
  - `new() -> Mutex`
  - `lock(&mut self)`
  - `unlock(&mut self)`
  - `destroy(&mut self)`
- `AtomicI64` -- lock-free atomic 64-bit integer (sequentially-consistent)
  - `new(val: i64) -> AtomicI64`
  - `load(&self) -> i64`
  - `store(&mut self, val: i64)`
  - `fetch_add(&mut self, val: i64) -> i64`
  - `compare_exchange(&mut self, expected: i64, desired: i64) -> i64`

### std.sync.ring_buffer (`std/sync/ring_buffer.salt`)

SPSC lock-free ring buffer.

- `RingBuffer` -- `{ buffer, capacity, mask, head, tail }`
  - `new(buffer: Ptr<Ptr<u8>>, capacity: i64) -> RingBuffer`
  - `push(&mut self, item: Ptr<u8>) -> Result<i32>`
  - `pop(&mut self) -> Result<Ptr<u8>>`

### std.sync.rcu (`std/sync/rcu.salt`)

Read-Copy-Update pointer with atomic load/store.

- `RcuPointer` -- `{ ptr: Ptr<u8> }`
  - `new(initial: Ptr<u8>) -> RcuPointer`
  - `read(&self) -> Ptr<u8>`
  - `update(&mut self, new_ptr: Ptr<u8>)`

---

## std.thread (`std/thread/thread.salt`)

1:1 OS threading via pthreads.

- `Thread` -- `{ handle: i64 }`
  - `spawn(f: i64) -> Thread`
  - `join(self) -> i32`

---

## std.time (`std/time.salt`)

Monotonic clock and duration type.

- `Duration` -- `{ nanos: i64 }`
  - `from_secs(secs: i64) -> Duration`
  - `from_millis(ms: i64) -> Duration`
  - `from_micros(us: i64) -> Duration`
  - `from_nanos(ns: i64) -> Duration`
  - `as_secs(&self) -> i64`
  - `as_millis(&self) -> i64`
  - `as_micros(&self) -> i64`
  - `as_nanos(&self) -> i64`
  - `add(&self, other: &Duration) -> Duration`
  - `sub(&self, other: &Duration) -> Duration`
  - `is_zero(&self) -> bool`
- `Instant` -- `{ nanos: i64 }`
  - `now() -> Instant`
  - `elapsed(&self) -> Duration`
  - `duration_since(&self, earlier: &Instant) -> Duration`
  - `as_nanos(&self) -> i64`
- `now() -> Instant`
- `now_nanos() -> i64`
- `sleep_nanos(target_ns: i64)`
- `sleep_ms(ms: i64)`
- `sleep(d: &Duration)`

```salt
let start = Instant::now();
// ... work ...
let elapsed = start.elapsed();
println("took {elapsed.as_millis()} ms");
```

---

## std.path (`std/path/path.salt`)

Zero-copy path manipulation (parent, filename, extension, join).

- `Path` -- `{ data: Ptr<u8>, len: i64 }`
  - `from_raw(data: Ptr<u8>, len: i64) -> Path`
  - `to_view(&self) -> StringView`
  - `separator() -> u8` -- returns 47 ('/')
  - `is_absolute(&self) -> bool`
  - `parent(&self) -> Path`
  - `filename(&self) -> StringView`
  - `extension(&self) -> StringView`
  - `stem(&self) -> StringView`
  - `join(&self, other: Ptr<u8>, other_len: i64, buf: Ptr<u8>, buf_cap: i64) -> Path`

---

## std.env (`std/env/env.salt`)

Command-line arguments and environment variables via C FFI.

- `args_count() -> i32`
- `arg(idx: i32) -> StringView`
- `get_env(name: &u8) -> Option<StringView>`
- `quit(code: i32)`

---

## std.args (`std/args/args.salt`)

Zero-allocation CLI argument parser.

- `ArgParser` -- `{ argc: i32 }`
  - `new() -> ArgParser`
  - `count(&self) -> i32`
  - `program_name(&self) -> StringView`
  - `has_flag(&self, name: StringView) -> bool`
  - `get_option(&self, name: StringView) -> Option<StringView>`
  - `positional(&self, idx: i32) -> Option<StringView>`

```salt
let args = ArgParser::new();
if args.has_flag("--verbose") {
    log_info("verbose mode");
}
```

---

## std.hash (`std/hash/mod.salt`)

Hash trait and FNV-1a / WyHash-style mixing functions with optional SIMD acceleration.

- **Trait `Hash`** -- `fn hash(&self) -> u64`
- Implementations: `i64`, `u64`, `i32`, `u32`, `i8`, `u8`, `usize`, `bool`, `f64`, `f32`, `String`
- `hash_i64(val: i64) -> u64`
- `hash_u64(val: u64) -> u64`
- `hash_i32(val: i32) -> u64`
- `hash_bytes(ptr: Ptr<u8>, len: i64) -> u64`
- `hash_bytes_fast(ptr: Ptr<u8>, len: usize) -> u64`

---

## std.eq (`std/eq/mod.salt`)

Equality trait for HashMap key comparison.

- **Trait `Eq`** -- `fn eq(&self, other: &Self) -> bool`
- Implementations: all primitives (`i8`-`i64`, `u8`-`u64`, `usize`, `bool`, `f32`, `f64`, `String`)

---

## std.ord (`std/ord/mod.salt`)

Total ordering trait.

- **Trait `Ord`** -- `fn cmp(&self, other: &Self) -> i32`
- Implementations: all primitives (`i8`-`i64`, `u8`-`u64`, `usize`, `f32`, `f64`, `bool`)
- **Constants:** `LESS` (-1), `EQUAL` (0), `GREATER` (1)

---

## std.math (`std/math/mod.salt`)

Math functions (compiler intrinsics lowered to LLVM opcodes).

**f32:** `expf`, `logf`, `powf`, `sqrtf`, `sinf`, `cosf`, `fabsf`, `floorf`, `ceilf`

**f64:** `exp`, `log`, `pow`, `sqrt`, `sin`, `cos`, `fabs`, `floor`, `ceil`

**Bit manipulation:** `ctz_u64`, `clz_u64`, `popcount_u64`

---

## std.simd

SIMD-Within-A-Register operations for SwissTable, prefetch hints, and branch prediction.

### std.simd (`std/simd/mod.salt`)

- `Group` -- SwissTable 8-byte group (u64)
  - `load(ptr: Ptr<i8>) -> Group`
  - `match_tag(self, tag: i8) -> u64`
  - `first_empty(self) -> i64`
  - `has_empty(self) -> bool`
  - `first_match(self, tag: i8) -> i64`
  - `width() -> i64` -- returns 8
- `u64x2` -- 2-lane 64-bit vector for parallel FNV
  - `splat(val: u64) -> u64x2`
  - `new(lo: u64, hi: u64) -> u64x2`
  - `xor(&self, other: u64x2) -> u64x2`
  - `mul(&self, other: u64x2) -> u64x2`
  - `extract_lo(&self) -> u64`
  - `extract_hi(&self) -> u64`
  - `reduce_xor(&self) -> u64`
- `load_u64x2(ptr: Ptr<u8>) -> u64x2`
- `prefetch_read(ptr: Ptr<i8>)`
- `prefetch_read_once(ptr: Ptr<i8>)`
- `prefetch_write(ptr: Ptr<i8>)`
- `unlikely(cond: bool) -> bool`
- `likely(cond: bool) -> bool`

### std.simd.text (`std/simd/text.salt`)

SWAR text scanning (8 bytes per iteration).

- `find_byte(haystack: Ptr<u8>, len: i64, needle: u8) -> i64`
- `find_byte2(haystack: Ptr<u8>, len: i64, a: u8, b: u8) -> i64`

---

## std.encoding (`std/encoding/encoding.salt`)

Encoding utilities -- Base64 and hexadecimal (RFC 4648).

- `base64_encoded_len(input_len: i64) -> i64`
- `base64_encode(data: Ptr<u8>, len: i64, out_buf: Ptr<u8>) -> i64`
- `hex_encoded_len(input_len: i64) -> i64`
- `hex_encode(data: Ptr<u8>, len: i64, out_buf: Ptr<u8>) -> i64`

---

## std.json (`std/json/json.salt`)

Cursor-based recursive-descent JSON parser and buffer-based writer. Zero-copy string references.

- `JsonValue` -- `{ type_tag, num_val, bool_val, str_ptr, str_len }`
  - `number(val: f64) -> JsonValue`
  - `boolean(val: bool) -> JsonValue`
  - `null() -> JsonValue`
  - `string(ptr: Ptr<u8>, len: i64) -> JsonValue`
- `JsonArray` -- fixed-capacity (64 elements)
- `JsonEntry` -- `{ key_ptr, key_len, value }`
- `JsonObject` -- fixed-capacity (32 entries)
- `JsonParser` -- `{ data, len, pos }`
  - `new(data: Ptr<u8>, len: i64) -> JsonParser`
  - `parse_value(&mut self) -> Result<JsonValue>`
  - `parse_array(&mut self, out: &mut JsonArray) -> Result<i32>`
  - `parse_object(&mut self, out: &mut JsonObject) -> Result<i32>`
- `JsonWriter` -- `{ buf, cap, pos }`
  - `new(buf: Ptr<u8>, cap: i64) -> JsonWriter`
  - `write_null(&mut self) -> i64`
  - `write_bool(&mut self, val: bool) -> i64`
  - `write_string(&mut self, src: Ptr<u8>, len: i64) -> i64`
  - `write_i64(&mut self, val: i64) -> i64`
  - `write_sv(&mut self, ptr: Ptr<u8>, len: i64) -> i64`
  - `bytes_written(&self) -> i64`
  - `write_array_start(&mut self)`, `write_array_end(&mut self)`
  - `write_object_start(&mut self)`, `write_object_end(&mut self)`
  - `write_comma(&mut self)`, `write_key(&mut self, key: Ptr<u8>, key_len: i64)`
- **Trait `ToJson`** -- `fn to_json(&self, w: &mut JsonWriter)`
- **Constants:** `JSON_STRING` (0), `JSON_NUMBER` (1), `JSON_BOOL` (2), `JSON_NULL` (3), `JSON_ARRAY` (4), `JSON_OBJECT` (5)

```salt
let mut parser = JsonParser::new(data, len);
let val = parser.parse_value().unwrap();
```

---

## std.log (`std/log.salt`)

Structured logging with level prefixes via `puts`.

- `log_debug(msg: &u8)`
- `log_info(msg: &u8)`
- `log_warn(msg: &u8)`
- `log_error(msg: &u8)`

---

## std.regex (`std/regex/regex.salt`)

Regular expression matching via POSIX ERE (C bridge).

- `Match` -- `{ start: i64, end: i64 }`
- `salt_regex_compile(pattern: &u8) -> i64`
- `salt_regex_match(handle: i64, text: &u8) -> i32`
- `salt_regex_find(handle: i64, text: &u8, start_out: Ptr<i64>, end_out: Ptr<i64>) -> i32`
- `salt_regex_find_groups(handle: i64, text: &u8, groups_out: Ptr<i64>, max_groups: i32) -> i32`
- `salt_regex_free(handle: i64)`

---

## std.test (`std/test.salt`)

Minimal test assertion library.

- `assert(condition: bool)`
- `assert_eq(a: u64, b: u64)`

---

## std.fmt

Formatting traits for type-safe string output.

### std.fmt.display (`std/fmt/display.salt`)

- **Trait `Display`** -- `fn fmt(&self, buf: &mut String)`
- Implementations: `i32`, `i64`, `bool`, `&u8`, `f64`, `f32`
- `format<T: Display>(value: &T) -> String`

### std.fmt.debug (`std/fmt/debug.salt`)

- **Trait `Debug`** -- `fn fmt_debug(&self)`

---

## std.os.syscall (`std/os/syscall.salt`)

Raw FFI declarations for OS networking primitives (Layer 1 of the 3-layer network architecture).

- `http_tcp_listen(port: i32) -> i32`
- `http_accept(listen_fd: i32) -> i32`
- `http_recv(fd: i32, buf: Ptr<u8>, len: i64) -> i64`
- `http_send(fd: i32, buf: Ptr<u8>, len: i64) -> i64`
- `http_close(fd: i32) -> i32`
- `http_kq_create() -> i32`
- `http_kq_register(kq: i32, fd: i32, filter: i32) -> i32`
- `http_kq_deregister(kq: i32, fd: i32) -> i32`
- `http_kq_wait(kq: i32, events_out: Ptr<i64>, max_events: i32, timeout_ms: i32) -> i32`
- `rdtsc() -> i64`

---

## std.mem

### std.mem (`std/mem/mod.salt`)

Memory view and mmap abstraction.

- `SafeView<T>` -- verified safe view

### std.mem.allocator (`std/mem/allocator.salt`)

Pluggable allocators for Vec.

- `HeapAllocator` -- `malloc`/`realloc`/`free`
- `ArenaAllocator` -- arena bump allocation (free is no-op)

---

## std.random (`std/random/mod.salt`)

Simple xorshift-based RNG.

- `Rng` -- `{ state0: u64, state1: u64 }`
  - `new(seed: u64) -> Rng`

---

## std.comptime (`std/comptime.salt`)

Compile-time tokenizer for prefixed string literals (f-strings, hex strings).

- `Span`, `TokenKind`, `Token`, `TokenStream`, `FormatSpec`
- `tokenize(prefix: String, content: String) -> TokenStream`

---

## std.ipc (`std/ipc/typed_channel.salt`)

Typed IPC messages over ring buffers.

---

## Module Index

| Package | File |
|---------|------|
| `std.core.ptr` | `std/core/ptr.salt` |
| `std.core.mem` | `std/core/mem.salt` |
| `std.core.alloc` | `std/core/alloc.salt` |
| `std.core.arena` | `std/core/arena.salt` |
| `std.core.option` | `std/core/option.salt` |
| `std.core.result` | `std/core/result.salt` |
| `std.core.str` | `std/core/str.salt` |
| `std.core.slice` | `std/core/slice.salt` |
| `std.core.boxed` | `std/core/boxed.salt` |
| `std.core.buffer` | `std/core/buffer.salt` |
| `std.core.clone` | `std/core/clone.salt` |
| `std.core.conv` | `std/core/conv.salt` |
| `std.core.drop` | `std/core/drop.salt` |
| `std.core.fmt` | `std/core/fmt.salt` |
| `std.core.iter` | `std/core/iter.salt` |
| `std.core.node_ptr` | `std/core/node_ptr.salt` |
| `std.core.slab_alloc` | `std/core/slab_alloc.salt` |
| `std.collections.vec` | `std/collections/vec.salt` |
| `std.collections.slab` | `std/collections/slab.salt` |
| `std.collections.string_map` | `std/collections/string_map.salt` |
| `std.collections.hash_map` | `std/collections/hash_map.salt` |
| `std.string` | `std/string.salt` |
| `std.status` | `std/status.salt` |
| `std.io` | `std/io/mod.salt` |
| `std.io.print` | `std/io/print.salt` |
| `std.io.file` | `std/io/file.salt` |
| `std.io.writer` | `std/io/writer.salt` |
| `std.io.buffered_reader` | `std/io/buffered_reader.salt` |
| `std.io.buffered_writer` | `std/io/buffered_writer.salt` |
| `std.io.reactor` | `std/io/reactor.salt` |
| `std.net.tcp` | `std/net/tcp.salt` |
| `std.net.poller` | `std/net/poller.salt` |
| `std.http.parser` | `std/http/parser.salt` |
| `std.http.response` | `std/http/response.salt` |
| `std.http.client` | `std/http/client.salt` |
| `std.fs.fs` | `std/fs/fs.salt` |
| `std.process` | `std/process/process.salt` |
| `std.process.spawn` | `std/process/spawn.salt` |
| `std.sync.sync` | `std/sync/sync.salt` |
| `std.sync.ring_buffer` | `std/sync/ring_buffer.salt` |
| `std.sync.rcu` | `std/sync/rcu.salt` |
| `std.thread` | `std/thread/thread.salt` |
| `std.time` | `std/time.salt` |
| `std.path.path` | `std/path/path.salt` |
| `std.env.env` | `std/env/env.salt` |
| `std.args.args` | `std/args/args.salt` |
| `std.hash` | `std/hash/mod.salt` |
| `std.eq` | `std/eq/mod.salt` |
| `std.ord` | `std/ord/mod.salt` |
| `std.math` | `std/math/mod.salt` |
| `std.simd` | `std/simd/mod.salt` |
| `std.simd.text` | `std/simd/text.salt` |
| `std.encoding` | `std/encoding/encoding.salt` |
| `std.json.json` | `std/json/json.salt` |
| `std.log` | `std/log.salt` |
| `std.regex` | `std/regex/regex.salt` |
| `std.test` | `std/test.salt` |
| `std.fmt` | `std/fmt/display.salt` |
| `std.fmt.debug` | `std/fmt/debug.salt` |
| `std.os.syscall` | `std/os/syscall.salt` |
| `std.mem` | `std/mem/mod.salt` |
| `std.mem.allocator` | `std/mem/allocator.salt` |
| `std.random` | `std/random/mod.salt` |
| `std.comptime` | `std/comptime.salt` |
| `std.ipc.typed_channel` | `std/ipc/typed_channel.salt` |
