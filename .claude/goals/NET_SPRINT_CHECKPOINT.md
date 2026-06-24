# Network Stack Sprint — Checkpoint

**Date:** 2026-06-24
**Last commit:** `0ea2634` — complete TCP stack, end-to-end HTTP fetch verified
**Tests:** 12/12 passing

## Current State

### TCP Client Stack — Complete ✅

| Syscall | Num | Status | Implementation |
|---|---|---|---|
| sys_ping | 20 | ✅ Done | ICMP echo request/reply, ARP resolution |
| sys_tcp_connect | 21 | ✅ Done | SYN→SYN-ACK→ACK, slot=0 fix, ISN tracking |
| sys_tcp_send | 22 | ✅ Done | Full segment: eth+IP+TCP+payload, copy_from_user, checksum |
| sys_tcp_recv | 23 | ✅ Done | copy_to_user, KPTI-safe user buffer access |
| sys_tcp_close | 24 | ✅ Done | Buffer cleanup, TCB free |

### End-to-End Verified
```
fetch.salt → TCP connect(10.0.2.2:8080) → HTTP GET → Python server → HTTP 200 OK (1602 bytes)
```

### Key Bug Fixes
- **Final ACK**: tcp_client_connect sends ACK to complete 3-way handshake (SLIRP gate)
- **seq_snd tracking**: ISN+1 stored after SYN (SYN consumes one sequence number)
- **SMAP safety**: copy_from_user for TX, copy_to_user for RX
- **Return sentinel**: 0xFF..FF for errors (slot 0 is a valid TCB)
- **Module imports**: tcp_send_data lives in icmp_xmit.salt (tcp_dispatch can't import eth/ip/virtio)

### Architecture
```
Layer 5  Application    fetch.salt, ping.salt (Ring 3)
Layer 4  Socket Lib     user/lib/syscall.salt wrappers
Layer 3  Transport      sys_tcp_{connect,send,recv,close} (Ring 0)
                        sys_ping (ICMP, done)
Layer 2  Network        ip.salt, icmp.salt, netcore.salt
Layer 1  Link           eth.salt, arp.salt
Layer 0  Hardware       virtio_net.salt
```

### Known Limitations
- **No server-side TCP**: TcpListener/bind/accept not implemented
- **No TCP retransmission**: Lost segments not recovered
- **Single connection**: fetch opens one connection; no concurrent TCP
- **Hardcoded port 8080**: sys_tcp_connect ignores user-specified port
- **tcp_dispatch.salt import issue**: Adding eth/ip/virtio imports crashes linker
- **Incremental build fragility**: must `rm -rf qemu_build` for reliable kernel rebuilds

### Next Steps
1. **Server-side TCP**: listen/accept for Lettuce and other servers
2. **Extract ip_transmit()**: shared packet construction (used by ICMP, TCP SYN, TCP data)
3. **Port parameter**: pass dst_port through syscall from userspace
4. **Fix incremental builds**: qemu_build cache produces broken kernels
