# I/O & Networking

## `std.io.File`

File I/O with read and write operations.

```salt
use std.io.file.File
use std.io.file.{O_RDONLY, O_WRONLY, O_CREAT, O_TRUNC}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `File::open` | `(path: &u8, flags: OpenFlags) -> Result<File>` | Open file at path with flags |
| `read` | `(&self, Ptr<u8>, i64) -> Result<i64>` | Read up to n bytes into buffer |
| `write` | `(&self, &u8, i64) -> Result<i64>` | Write bytes from buffer |
| `write_all` | `(&self, &u8, i64) -> Result<i64>` | Write exactly count bytes, looping until flushed |
| `close` | `(&self) -> i32` | Close the file descriptor |
| `mmap` | `(&self, u64, Prot, MapFlags) -> Result<Ptr<T>>` | Memory-map the file |
| `is_valid` | `(&self) -> bool` | Check if fd >= 0 |
| `fd` | `(&self) -> i32` | Get raw file descriptor |

**OpenFlags constants:** `O_RDONLY`, `O_WRONLY`, `O_RDWR`, `O_CREAT`, `O_TRUNC`
**Prot constants:** `PROT_NONE`, `PROT_READ`, `PROT_WRITE`, `PROT_EXEC`
**MapFlags constants:** `MAP_SHARED`, `MAP_PRIVATE`, `MAP_ANONYMOUS`

**Usage:**
```salt
use std.io.file.{File, O_RDONLY}

let f = File::open("data.txt\0", O_RDONLY)?;
let mut buf: [u8; 1024] = [0; 1024];
let n = f.read(&buf[0] as Ptr<u8>, 1024)?;
f.close();
```

## `std.io.Writer`

Trait for types that accept byte output. Implemented by `File`, `String`, `BufferedWriter`, `Console`.

```salt
use std.io.writer.Writer
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `write_bytes` | `(&mut self, Ptr<u8>, i64) -> ()` | Write bytes |

## `std.io.BufferedWriter`

Buffered output wrapper wrapping a file descriptor. Accumulates writes in an 8KB buffer and flushes in batches.

```salt
use std.io.buffered_writer.BufferedWriter
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(fd: i32) -> BufferedWriter` | Wrap an fd with write buffering |
| `write_bytes` | `(&mut self, &u8, i64) -> ()` | Write bytes (buffered) |
| `flush` | `(&mut self) -> ()` | Force flush buffer to fd |
| `remaining` | `(&self) -> i64` | Remaining buffer capacity |
| `reserve` | `(&mut self, i64) -> &mut u8` | Reserve space in buffer, flushing if needed |
| `advance` | `(&mut self, i64) -> ()` | Advance position after unchecked writes |

**Usage:**
```salt
use std.io.buffered_writer.BufferedWriter

let mut writer = BufferedWriter::new(1);  // stdout
writer.write_bytes("hello\n\0", 6);
writer.flush();
```

## `std.io.BufferedReader`

Buffered input wrapper wrapping a file descriptor. Reads in 8KB chunks and serves from cache.

```salt
use std.io.buffered_reader.BufferedReader
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(fd: i32) -> BufferedReader` | Wrap an fd with read buffering |
| `read` | `(&mut self, Ptr<u8>, i64) -> i64` | Read bytes (buffered), actual count returned |
| `read_byte` | `(&mut self) -> i64` | Read a single byte (-1 on EOF) |
| `read_line` | `(&mut self, Ptr<u8>, i64) -> i64` | Read until newline, writes to output buffer (max_len-1), null-terminates, returns bytes written |
| `has_data` | `(&self) -> bool` | True if data available in buffer |
| `remaining` | `(&self) -> i64` | Bytes left in current buffer |
| `close` | `(&mut self) -> ()` | Close underlying file descriptor |

**Usage:**
```salt
use std.io.file.{File, O_RDONLY}
use std.io.buffered_reader.BufferedReader

let f = File::open("data.txt\0", O_RDONLY).unwrap();
let mut reader = BufferedReader::new(f.fd());
let mut line = malloc(256);
let len = reader.read_line(line as Ptr<u8>, 256);
reader.close();
```

## `std.io` Multipoll Reactors

Platform-specific I/O multiplexing. Multiple reactors available via compile-time dispatch:

- `reactor_kqueue.salt` — macOS / BSD
- `reactor_epoll.salt` — Linux
- `reactor_keuos.salt` — KeuOS native (SPSC-based)

```salt
use std.io.reactor.Poller
```

## `std.net.TcpListener`

TCP server socket. Binds, listens, and accepts connections. Uses SPSC ring IPC to communicate with NetD.

```salt
use std.net.tcp.TcpListener
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `TcpListener::bind` | `(port: i32) -> Result<TcpListener>` | Bind to port |
| `accept` | `(&self) -> Result<TcpStream>` | Accept incoming connection |
| `fd` | `(&self) -> i32` | Get raw file descriptor |
| `close` | `(&self) -> ()` | Close listener |

## `std.net.TcpStream`

TCP server connection. Bidirectional byte stream over SPSC rings (zero-trap data path).

```salt
use std.net.tcp.TcpStream
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `recv` | `(&self, Ptr<u8>, i64) -> i64` | Receive bytes (0 = EOF/EAGAIN) |
| `send` | `(&self, Ptr<u8>, i64) -> i64` | Send bytes, returns count sent |
| `fd` | `(&self) -> i32` | Get raw file descriptor |
| `close` | `(&self) -> ()` | Close connection |

**Usage (echo server):**
```salt
use std.net.tcp.TcpListener

let listener = TcpListener::bind(8080)?;
let stream = listener.accept()?;
let mut buf: [u8; 4096] = [0; 4096];
let n = stream.recv(&buf[0] as Ptr<u8>, 4096);
stream.send(&buf[0] as Ptr<u8>, n);
stream.close();
```

## `std.net.Poller`

Non-blocking I/O readiness notification using SPSC ring polling (zero-trap).

```salt
use std.net.poller.Poller
use std.net.poller.PollFilter
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `() -> Result<Poller>` | Create poller |
| `register` | `(&mut self, fd: i32, filter: PollFilter) -> i32` | Register fd for read/write events |
| `deregister` | `(&mut self, fd: i32) -> i32` | Remove fd from poller |
| `wait` | `(&self, Ptr<i64>, i32, i32) -> i32` | Wait for ready events (event_buf, max_events, timeout_ms) |

## `std.http.Client`

Low-level HTTP client with zero-copy parsing.

```salt
use std.http.client
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `connect` | `(StringView, i32) -> i32` | Connect to host:port, return fd |
| `send` | `(i32, Ptr<u8>, i64) -> i64` | Send request bytes |
| `recv` | `(i32, Ptr<u8>, i64) -> i64` | Receive response bytes |
| `close` | `(i32) -> ()` | Close connection |
| `get_raw` | `(StringView, i32, StringView, Ptr<u8>, i64) -> i64` | High-level GET request |

## `std.http.Parser`

Zero-copy HTTP request/response parser.

```salt
use std.http.parser
```

## `std.fs` — VfsConnection

KeuOS filesystem operations via IPC with StoreD (PID 6). All operations go through a `VfsConnection` handle.

```salt
use std.fs.fs.VfsConnection
use std.fs.fs.FileHandle
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `vfs_connect` | `() -> VfsConnection` | Connect to StoreD, allocate SPSC rings |
| `exists` | `(&mut self, &u8) -> bool` | Check if path exists |
| `create_dir` | `(&mut self, &u8) -> Result<i32>` | Create directory |
| `remove_file` | `(&mut self, &u8) -> Result<i32>` | Delete file |
| `open` | `(&mut self, &u8) -> Result<FileHandle>` | Open file for reading |
| `read` | `(&mut self, &FileHandle, Ptr<u8>, u64) -> Result<u64>` | Read from file handle |
| `write` | `(&mut self, &FileHandle, Ptr<u8>, u64) -> Result<u64>` | Write to file handle |
| `close` | `(&mut self, &FileHandle) -> Result<i32>` | Close file handle |

**Usage:**
```salt
use std.fs.fs.{vfs_connect, VfsConnection}

let mut vfs = vfs_connect();
if vfs.exists("/data/config\0") {
    let handle = vfs.open("/data/config\0")?;
    let mut buf: [u8; 256] = [0; 256];
    let n = vfs.read(&handle, &buf[0] as Ptr<u8>, 256)?;
    vfs.close(&handle)?;
}
```
