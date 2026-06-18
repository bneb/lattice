# I/O & Networking

## `std.io.File`

File I/O with read, write, and seek operations.

```salt
use std.io.file.File
```

| Function/Method | Signature | Description |
|----------------|-----------|-------------|
| `File::open` | `(StringView) -> Result<File>` | Open existing file for reading |
| `File::create` | `(StringView) -> Result<File>` | Create or truncate for writing |
| `read` | `(&mut self, Ptr<u8>, i64) -> Result<i64>` | Read up to n bytes into buffer |
| `write` | `(&mut self, Ptr<u8>, i64) -> Result<i64>` | Write bytes from buffer |
| `close` | `(&mut self) -> ()` | Close the file descriptor |
| `seek` | `(&mut self, i64) -> ()` | Seek to position |

**Usage:**
```salt
use std.io.file.File

let mut f = File::open("data.txt")?;
let mut buf: [u8; 1024] = [0; 1024];
let n = f.read(&buf as Ptr<u8>, 1024)?;
f.close();
```

## `std.io.Writer`

Trait for types that accept byte output. Implemented by `File`, `String`, `BufferedWriter`.

```salt
use std.io.writer.Writer
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `write` | `(&mut self, Ptr<u8>, i64) -> Result<i64>` | Write bytes |

## `std.io.BufferedWriter`

Buffered output wrapper. Accumulates writes in an internal buffer and flushes in batches.

```salt
use std.io.buffered_writer.BufferedWriter
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(Writer) -> BufferedWriter` | Wrap a writer with buffering |
| `write` | `(&mut self, Ptr<u8>, i64) -> Result<i64>` | Write bytes (buffered) |
| `flush` | `(&mut self) -> Result<()>` | Force flush buffer to inner writer |

## `std.io.BufferedReader`

Buffered input wrapper. Reads in chunks and serves from cache.

```salt
use std.io.buffered_reader.BufferedReader
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(File) -> BufferedReader` | Wrap a file with read buffering |
| `read` | `(&mut self, Ptr<u8>, i64) -> Result<i64>` | Read bytes (buffered) |
| `read_line` | `(&mut self) -> Result<String>` | Read until newline |

## `std.io Multipoll Reactors`

Platform-specific I/O multiplexing. Multiple reactors available via compile-time dispatch:

- `reactor_kqueue.salt` — macOS / BSD
- `reactor_epoll.salt` — Linux
- `reactor_keuos.salt` — KeuOS native (SPSC-based)

```salt
use std.io.reactor.Poller
```

## `std.net.TcpListener`

TCP server socket. Binds, listens, and accepts connections.

```salt
use std.net.tcp.TcpListener
```

| Function/Method | Signature | Description |
|----------------|-----------|-------------|
| `TcpListener::bind` | `(StringView, i32) -> Result<TcpListener>` | Bind to address:port |
| `accept` | `(&self) -> Result<TcpStream>` | Accept incoming connection |
| `close` | `(&self) -> ()` | Close listener |

## `std.net.TcpStream`

TCP client/server connection. Bidirectional byte stream.

```salt
use std.net.tcp.TcpStream
```

| Function/Method | Signature | Description |
|----------------|-----------|-------------|
| `TcpStream::connect` | `(StringView, i32) -> Result<TcpStream>` | Connect to remote address:port |
| `read` | `(&mut self, Ptr<u8>, i64) -> Result<i64>` | Read bytes |
| `write` | `(&mut self, Ptr<u8>, i64) -> Result<i64>` | Write bytes |
| `close` | `(&mut self) -> ()` | Close connection |

**Usage (echo server):**
```salt
use std.net.tcp.TcpListener

let listener = TcpListener::bind("0.0.0.0", 8080)?;
let stream = listener.accept()?;
let mut buf: [u8; 4096] = [0; 4096];
let n = stream.read(&buf as Ptr<u8>, 4096)?;
stream.write(&buf as Ptr<u8>, n)?;
stream.close();
```

## `std.net.Poller`

Non-blocking I/O readiness notification. Wraps `kqueue` (macOS) or `epoll` (Linux).

```salt
use std.net.poller.Poller
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `() -> Poller` | Create poller |
| `add` | `(&mut self, fd: i32, events: i32) -> ()` | Register fd for events |
| `poll` | `(&mut self, timeout_ms: i32) -> i32` | Wait for ready events |

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

## `std.http.Server`

HTTP server with request parsing and response building.

```salt
use std.http.server
```

## `std.http.Parser`

Zero-copy HTTP request/response parser.

```salt
use std.http.parser
```
