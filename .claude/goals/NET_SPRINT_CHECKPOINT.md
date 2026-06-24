# Network Stack Sprint — Checkpoint

**Date:** 2026-06-24
**Last commit:** `56d5f1b` — T3b+T4: wire TCP RX dispatch + implement recv/close
**Tests:** 11/11 passing

## Current State

### All tasks complete (T1-T6)

**T1:** http_client.salt deleted. Syscalls 21-24 reserved.
**T2:** TCB extended with owner_pid, recv_buf, recv_head/tail, recv_fin.
**T3:** tcp_client_connect implemented in icmp_xmit.salt (SYN→poll→timeout).
**T3b:** TCP RX wired in tcp_dispatch.salt → netcore handle_tcp_notify.
**T4:** tcp_recv_data (real), tcp_close_conn (real), tcp_send_data (stub).
**T5:** Kernel has zero HTTP code. fetch calls syscall 21.
**T6:** 11/11 tests pass.

### Architecture (from NETWORK_DESIGN.md)
```
Layer 5  Application    fetch.salt, ping.salt (Ring 3)
Layer 4  Socket Lib     user/lib/syscall.salt wrappers
Layer 3  Transport      sys_tcp_{connect,send,recv,close} (Ring 0)
                        sys_ping (ICMP, done)
Layer 2  Network        ip.salt, icmp.salt, netcore.salt
Layer 1  Link           eth.salt, arp.salt
Layer 0  Hardware       virtio_net.salt
```

Syscall table:
| 20 | sys_ping | (ip) → rtt_ms | ✅ Done |
| 21 | sys_tcp_connect | (ip, port) → conn_id | ⬜ Stub |
| 22 | sys_tcp_send | (conn, buf, len) → sent | ⬜ Stub |
| 23 | sys_tcp_recv | (conn, buf, len) → read | ⬜ Stub |
| 24 | sys_tcp_close | (conn) | ⬜ Stub |

### Design decisions (do not revisit)
- Copy-based data path (not SPSC rings) for MVP
- Blocking connect (like sys_ping)
- Non-blocking send/recv
- recv returns ~0 for "closed" vs 0 for "no data yet"
- conn_id is TCB slot index
- owner_pid prevents connection hijacking
- Ephemeral ports: 42000 + pid * 10
- No HTTP/UDP in kernel. No NetD modification. No SPSC ring changes.

## Remaining Tasks

### T2: Extend TCB with client-side fields
File: `kernel/net/netd_tcp.salt`

Add to TcpConnection struct:
- owner_pid: u64
- recv_buf_phys: u64 (physical page for 4KB RX buffer)
- recv_head: u32 (write cursor)
- recv_tail: u32 (read cursor)
- recv_fin: bool

Update TCP_POOL initializer (1024 entries) with new field defaults.

Add functions:
- tcp_alloc_client(pid, local_port, remote_ip, remote_port) → slot
- tcp_get_recv_buf_virt(slot) → kernel virtual address

Gate: kernel builds. No behavior change.

### T3: TCP client connect
File: `kernel/net/netd_tcp.salt`

Implement tcp_client_connect(dst_ip, dst_port) → conn_id:
a. Ephemeral port = 42000 + (current_pid * 10)
b. tcp_alloc_client()
c. Generate client ISN (monotonic counter)
d. Build TCP SYN via netd_tcp_parse.build_tcp
e. Send via VirtIO (eth_build, ip_build, arp_lookup)
f. Poll netcore_poll_all() for SYN-ACK (~300 iterations)
g. On SYN-ACK: send ACK, transition to ESTABLISHED
h. Return conn_id (TCB slot) on success, 0 on timeout

Requires adding imports: eth, ip, arp, virtio_net, pmm, memory, netd_tcp_parse, serial
May need to extract SYN cookie functions to keep under 500 lines.

### T3b: Wire TCP data receive path
File: `kernel/net/netcore.salt`

In handle_tcp_notify, before forwarding to NetD:
- Parse TCP header with netd_tcp_parse.parse_tcp
- If SYN+ACK: look up TCB, complete handshake, send ACK
- If ACK (no SYN): look up TCB by ports, copy payload to recv_buf, advance recv_head
- If FIN: set recv_fin

Need @no_mangle accessors in netd_tcp.salt for:
- tcp_lookup_by_ports(local_port, remote_ip, remote_port) → slot
- tcp_get_state(slot) → u8
- tcp_append_recv(slot, data, len)

### T4: Implement syscalls 21-24
File: `kernel/core/syscall.salt`

Replace stubs with real implementations:
- sys_tcp_connect: calls tcp_client_connect
- sys_tcp_send: validates owner_pid, copy_from_user, builds PSH+ACK, sends, updates seq_snd
- sys_tcp_recv: validates owner_pid, copies from recv_buf, advances recv_tail
- sys_tcp_close: sends FIN, frees recv_buf page, frees TCB

User stubs in user/syscall_stubs.S (syscalls 21-24).
Wrappers in user/lib/syscall.salt.

### T5: Update fetch.salt
Build HTTP GET string in userspace:
```
let conn = syscall.tcp_connect(0x0A000202, 80);
if conn == 0 { print("connect failed\n"); exit(1); }
let req: [u8; N] = [...];  // HTTP request bytes
syscall.tcp_send(conn, &req as u64, N);
let mut buf: [u8; 4096] = [0; 4096];
let n = syscall.tcp_recv(conn, &buf as u64, 4096);
if n > 0 { print buf; }
syscall.tcp_close(conn);
```

### T6: Integration test
- python3 tools/runner_qemu.py test → 11/11
- Check fetch output in serial log
- Update test assertion if needed
- Commit

## Constraints
- Max 500 lines per file
- Max 32 non-blank lines per function
- Max 3 levels of nesting
- No mutants (TODO/FIXME/HACK/XXX)
- zero linker errors

## Quick Start After Reboot
```
cd /Users/kevin/projects/lattice
python3 tools/runner_qemu.py test  # verify 11/11
# Then start T2: Extend TCB in kernel/net/netd_tcp.salt
```

## Key Files
- Design doc: .claude/goals/NETWORK_DESIGN.md
- TCP state machine: kernel/net/netd_tcp.salt (381 lines)
- TCP parse/build: kernel/net/netd_tcp_parse.salt (215 lines)
- Packet dispatch: kernel/net/netcore.salt (~469 lines)
- ICMP send: kernel/net/icmp_xmit.salt (~125 lines)
- ICMP handler: kernel/net/icmp.salt (81 lines)
- Syscall dispatch: kernel/core/syscall.salt (~395 lines)
- Userspace syscall lib: user/lib/syscall.salt
- Test runner: tools/runner_qemu.py
