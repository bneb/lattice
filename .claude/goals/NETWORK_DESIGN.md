# Network Stack Design — KeuOS
# (Design Pass 2 — post red-team)

## Layering

```
Layer 5  Application    fetch.salt, ping.salt (Ring 3)
                        Protocol logic belongs here. Period.
───────────────────────────────────────────────────────────
Layer 4  Socket Lib     user/lib/syscall.salt wrappers (Ring 3)
                        tcp_connect / tcp_send / tcp_recv / tcp_close.
                        No IPC to NetD for outbound client connections.
───────────────────────────────────────────────────────────
Layer 3  Transport      kernel/net/tcp.salt (new, merged)
(Ring 0)                sys_tcp_{connect,send,recv,close}
                        sys_ping (ICMP, stateless)
                        TCB pool, sequence tracking, receive buffers,
                        client handshake + server SYN cookies.
───────────────────────────────────────────────────────────
Layer 2  Network        ip.salt, icmp.salt, netcore.salt
(Ring 0)                IP dispatch: protocol→handler routing.
───────────────────────────────────────────────────────────
Layer 1  Link           eth.salt, arp.salt
(Ring 0)                Frame build/parse. MAC resolution.
───────────────────────────────────────────────────────────
Layer 0  Hardware       virtio_net.salt
(Ring 0)                Virtqueue TX/RX, descriptor management.
═══════════════════════════════════════════════════════════
```

## Syscall Surface

| #  | Name | Signature | Semantics |
|----|------|-----------|-----------|
| 20 | `sys_ping` | `(dst_ip: u32) → rtt_ms: u64` | Blocking, 3s timeout, 0 = failure |
| 21 | `sys_tcp_connect` | `(dst_ip: u32, dst_port: u16) → conn_id: u64` | Blocking, 5s timeout, 0 = failure |
| 22 | `sys_tcp_send` | `(conn_id: u64, buf: u64, len: u64) → sent: u64` | Non-blocking, 0 = failure |
| 23 | `sys_tcp_recv` | `(conn_id: u64, buf: u64, len: u64) → read: u64` | Non-blocking, 0 = no data, ~0 = closed |
| 24 | `sys_tcp_close` | `(conn_id: u64)` | Sends FIN, frees TCB |

## TCB (Transmission Control Block) — extended

```
struct TcpConnection {
    state: u8,           // CLOSED, SYN_SENT, ESTABLISHED, etc.
    local_port: u16,
    remote_port: u16,
    remote_ip: u32,
    seq_snd: u32,        // next sequence number to send
    seq_rcv: u32,        // next sequence number expected
    owner_pid: u64,      // process that owns this connection
    recv_buf: u64,       // kernel virtual address of 4KB RX buffer
    recv_head: u32,      // write cursor into recv_buf
    recv_tail: u32,      // read cursor (advanced by sys_tcp_recv)
    recv_fin: bool,      // true if FIN received from remote
    last_activity: u64,  // monotonic timestamp
}
```

## Return value semantics (from red team)

- `sys_tcp_connect`: returns conn_id (1..1024). Returns 0 on timeout or pool exhaustion.
- `sys_tcp_send`: returns number of bytes sent. Returns 0 if conn invalid or not ESTABLISHED.
- `sys_tcp_recv`: returns number of bytes read into user buffer. Returns 0 if no data available (idle). Returns `0xFFFFFFFFFFFFFFFF` if connection is closed (FIN received + buffer drained). Caller distinguishes idle vs. closed by checking the return value.
- `sys_tcp_close`: sends FIN, transitions to FIN_WAIT_1, eventually frees TCB.

## Connection ownership (from red team)

Each TCB has an `owner_pid` field. `sys_tcp_connect` sets it to the calling process. `sys_tcp_send`, `sys_tcp_recv`, and `sys_tcp_close` verify that `TCB[conn_id].owner_pid == current_pid` before operating. This prevents one process from hijacking another's connection.

## Data path

**TX (send):**
1. `fetch.salt` builds HTTP GET string in userspace
2. Calls `sys_tcp_send(conn_id, buf, len)`
3. Kernel copies user data → kernel buffer
4. Builds TCP segment (PSH+ACK) with correct seq numbers
5. Sends via VirtIO
6. Updates TCB.seq_snd

**RX (receive):**
1. Packet arrives via VirtIO RX interrupt → `process_one()`
2. IPv4 dispatch: protocol 6 (TCP) → handle TCP data
3. Kernel parses TCP header, looks up TCB by (dst_port, src_ip, src_port)
4. If found and ESTABLISHED: copies payload to TCB.recv_buf, updates TCB.seq_rcv
5. Later: `sys_tcp_recv(conn_id, buf, len)` copies from TCB.recv_buf → user buffer

## What gets deleted

- `http_client.salt` — HTTP byte arrays in kernel. Wrong layer.
- `tcp_client.salt` — was a draft, superseded by merged tcp.salt.

## What gets created

- `kernel/net/tcp.salt` — merged from netd_tcp.salt (TCB pool, SYN cookies, server handshake) + new client-side connect/send/recv. Under 500 lines by extracting pure functions to netd_tcp_parse.salt (already done).

## What gets updated

- `kernel/net/netcore.salt` — TCP dispatch: data packets → buffer in TCB (not just forward to NetD)
- `kernel/core/syscall.salt` — add syscalls 21-24, remove http_get delegation
- `user/fetch.salt` — build HTTP request string, call sys_tcp_connect/send/recv/close
- `user/lib/syscall.salt` — add tcp_connect/send/recv/close wrappers
- `user/syscall_stubs.S` — add assembly stubs for syscalls 21-24

## What stays unchanged

- `virtio_net.salt`, `eth.salt`, `arp.salt`, `ip.salt`, `icmp.salt`, `icmp_xmit.salt`
- `netd_tcp_parse.salt` — pure TCP header functions, correctly separated
- `tcp_syn_cookie.salt` — can merge into tcp.salt later, not blocking
- `user/lib/socket.salt` — existing code for NetD-mediated connections; client programs can use simpler syscall wrappers instead
- NetD itself — still runs, still needed for server-side operations
- `sys_ping` (syscall 20) — done, correct, unchanged

## Non-goals (deferred)

- SPSC zero-copy rings in data path — optimization, not architecture
- Non-blocking connect — MVP is synchronous
- UDP syscalls — no userspace UDP consumer yet
- Socket fd integration — separate namespace for now
- IPv6, jumbo frames, TCP window scaling, congestion control
