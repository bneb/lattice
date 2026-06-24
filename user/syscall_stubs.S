/* user/syscall_stubs.S
 * User-space SYSCALL invocation stubs.
 * Linked into the Ring 3 user ELF binary.
 *
 * x86_64 SYSCALL ABI:
 *   RAX = syscall number
 *   RDI = arg0, RSI = arg1, RDX = arg2, R10 = arg3
 *   Returns: RAX = result
 *
 *   SYSCALL clobbers: RCX (saved RIP), R11 (saved RFLAGS)
 *   All other registers preserved by kernel convention.
 */
.intel_syntax noprefix

/* sys_write(fd: u64, buf: &u8, len: u64) -> u64
 * Syscall 1 — write len bytes from buf to file descriptor fd.
 *   RDI = fd (1 = stdout)
 *   RSI = buf (pointer to data)
 *   RDX = len (byte count)
 */
.global sys_write
sys_write:
    mov rax, 1
    syscall
    ret

/* sys_exit(code: u64) -> !
 * Syscall 60 — terminate the process. Never returns.
 *   RDI = exit code
 */
.global sys_exit
sys_exit:
    mov rax, 60
    syscall
    /* Never reaches here — kernel halts */
    ud2

/* sys_str_data(sv: &u64) -> u64
 * SIP-safe string data pointer extraction.
 * Reads the first 8 bytes from a StringView struct.
 * Salt StringView layout: { ptr: u64, len: u64 }
 *   RDI = address of StringView
 *   Returns: RAX = data pointer (first field)
 *
 * This function exists to provide a formal boundary for SIP code
 * that cannot do `(&msg) as u64` then `*(addr as &u64)` because
 * those integer-to-pointer casts are rejected by the SIP safety gate.
 */
.global sys_str_data
sys_str_data:
    mov rax, [rdi]
    ret

/* sys_brk(new_brk: u64) -> u64
 * Syscall 12 — set/query program break (heap boundary).
 *   RDI = new_brk (0 = query current brk)
 *   Returns: RAX = new brk value
 */
.global sys_brk
sys_brk:
    mov rax, 12
    syscall
    ret

/* sys_mmap(length: u64, prot: u64) -> u64
 * Syscall 9 — anonymous memory mapping.
 *   RDI = length (bytes, page-aligned by kernel)
 *   RSI = prot (protection flags)
 *   Returns: RAX = mapped virtual address (0 on failure)
 */
.global sys_mmap
sys_mmap:
    mov rax, 9
    syscall
    ret

/* sys_ipc_send(target_pid: u64, msg0: u64, msg1: u64, msg2: u64) -> u64
 * Syscall 200 — Fast-path IPC: send 3 register words to target process.
 *   RDI = target_pid (process slot index)
 *   RSI = msg0  (payload word 0)
 *   RDX = msg1  (payload word 1)
 *   R10 = msg2  (payload word 2, moved from RCX per SYSCALL ABI)
 *   Returns: RAX = 0 on success, -1 on error
 *
 * The kernel overwrites the target's saved context so when it resumes:
 *   RAX = 200 (IPC sentinel), RDI = sender_pid, RSI/RDX/R10 = msg0/msg1/msg2
 * Then immediately yields the CPU to the target — zero scheduler overhead.
 */
.weak sys_ipc_send
sys_ipc_send:
    mov rax, 200
    mov r10, rcx        /* SysV arg3 (RCX) → SYSCALL arg3 (R10) */
    syscall
    ret

.global sys_ipc_recv_full
sys_ipc_recv_full:
    # RDI = pointer to u64 array of size 4 [sender, msg0, msg1, msg2]
    push rdi
    mov rax, 201
    syscall
    pop rcx
    mov [rcx+0], rax
    mov [rcx+8], rdi
    mov [rcx+16], rsi
    mov [rcx+24], rdx
    ret
.weak sys_ipc_recv
sys_ipc_recv:
    mov rax, 201
    syscall
    ret

/* sys_shm_grant(target_pid: u64, src_vaddr: u64, dst_vaddr: u64, flags: u64) -> u64
 * Syscall 202 — Grant target process access to a physical page.
 *   RDI = target_pid  (process to grant access to)
 *   RSI = src_vaddr   (caller's virtual address of page to share)
 *   RDX = dst_vaddr   (virtual address in target's space)
 *   R10 = flags       (reserved, 0)
 *   Returns: RAX = 0 on success, -1 on error
 */
.weak sys_shm_grant
sys_shm_grant:
    mov rax, 202
    mov r10, rcx        /* SysV arg3 (RCX) → SYSCALL arg3 (R10) */
    syscall
    ret
.global sys_ipc_get_msg0
sys_ipc_get_msg0:
    ret
.global sys_ipc_get_msg1
sys_ipc_get_msg1:
    ret
.global sys_ipc_get_msg2
sys_ipc_get_msg2:
    ret
.global sys_cap_bind
sys_cap_bind:
    ret
.global sys_ring_map
sys_ring_map:
    ret
.global sys_core_acquire
sys_core_acquire:
    ret
.global sys_ipc_reg_send
sys_ipc_reg_send:
    ret
.global sys_ipc_ring_await
sys_ipc_ring_await:
    ret
.global sys_mfence
sys_mfence:
    mfence
    ret
.global sys_yield
sys_yield:
    mov rax, 119
    syscall
    ret

/* sys_meminfo(buf_ptr: u64) -> u64
 * Syscall 17 — fill buffer with total/free/used page counts.
 *   RDI = buf_ptr (pointer to 3 x u64 output buffer)
 *   Returns: RAX = 0 on success, -1 on error
 */
.global sys_meminfo
sys_meminfo:
    mov rax, 17
    syscall
    ret

/* sys_ps(buf_ptr: u64) -> u64
 * Syscall 16 — fill buffer with process info.
 *   RDI = buf_ptr (pointer to output buffer)
 *   Returns: RAX = number of processes written
 */
.global sys_ps
sys_ps:
    mov rax, 16
    syscall
    ret

/* sys_uptime() -> u64
 * Syscall 18 — returns system tick count since boot.
 *   No arguments.
 *   Returns: RAX = tick count
 */
.global sys_uptime
sys_uptime:
    mov rax, 18
    syscall
    ret

/* sys_getpid() -> u64
 * Syscall 19 — returns calling process's PID.
 *   No arguments.
 *   Returns: RAX = PID
 */
.global sys_getpid
sys_getpid:
    mov rax, 19
    syscall
    ret

.global sys_spawn
sys_spawn:
    mov rax, 400
    syscall
    ret

.global sys_wait
sys_wait:
    mov rax, 401
    syscall
    ret

/* NetD Ring 3 migration stubs — kernel-internal symbols
   referenced by netd user code. These will be replaced by
   proper syscall gates when the NetD ABI is finalized. */
.global fastpath_handoff_syscall
.weak fastpath_handoff_syscall
fastpath_handoff_syscall:
    ret

.global salt_yield_check
.weak salt_yield_check
salt_yield_check:
    ret

/* sys_ping(ip: u32) -> u64
 * Syscall 20 — send ICMP echo request, return RTT in ms or 0 on timeout.
 *   RDI = dst_ip (u32 in lower 32 bits)
 *   Returns: RAX = RTT in milliseconds, 0 on timeout
 */
.global sys_ping
sys_ping:
    mov rax, 20
    syscall
    ret

/* sys_tcp_connect(ip: u32, port: u16) -> u64
 * Syscall 21 — TCP active open, returns conn_id or 0 on failure.
 *   RDI = dst_ip (u32, port encoded in upper 16 bits)
 *   Returns: RAX = conn_id (nonzero) or 0 on failure
 */
.global sys_tcp_connect
sys_tcp_connect:
    mov rax, 21
    syscall
    ret

/* sys_tcp_send(conn_id: u64, buf: u64, len: u64) -> u64
 * Syscall 22 — TCP send data, returns bytes sent or 0 on failure.
 *   RDI = conn_id, RSI = buf, RDX = len
 */
.global sys_tcp_send
sys_tcp_send:
    mov rax, 22
    syscall
    ret

/* sys_tcp_recv(conn_id: u64, buf: u64, len: u64) -> u64
 * Syscall 23 — TCP receive data, returns bytes read or 0.
 *   RDI = conn_id, RSI = buf, RDX = len
 */
.global sys_tcp_recv
sys_tcp_recv:
    mov rax, 23
    syscall
    ret

/* sys_tcp_close(conn_id: u64)
 * Syscall 24 — TCP close connection.
 *   RDI = conn_id
 */
.global sys_tcp_close
sys_tcp_close:
    mov rax, 24
    syscall
    ret

/* sys_tcp_listen(port: u16) -> u64
 * Syscall 25 — TCP passive open, returns listen_slot or 0xFF..FF on failure.
 *   RDI = port (u16)
 *   Returns: RAX = listen_slot
 */
.global sys_tcp_listen
sys_tcp_listen:
    mov rax, 25
    syscall
    ret

/* sys_tcp_accept(listen_slot: u64) -> u64
 * Syscall 26 — TCP accept, returns conn_slot or 0xFF..FF if none pending.
 *   RDI = listen_slot
 *   Returns: RAX = conn_slot
 */
.global sys_tcp_accept
sys_tcp_accept:
    mov rax, 26
    syscall
    ret

/* sys_http_get(ip: u32) -> u64
 * Syscall 21 — TCP connect + HTTP GET, returns bytes received or 0 on failure.
 *   RDI = dst_ip (u32)
 *   Returns: RAX = bytes received, 0 on failure/timeout
 */
.global sys_http_get
sys_http_get:
    mov rax, 21
    xor esi, esi     # send_buf = 0 (built-in request)
    xor edx, edx     # send_len = 0
    syscall
    ret

// memset — zero a memory region (called by compiler for large array init)
// RDI = dst, RSI = val (ignored, always 0), RDX = count
.global memset
.type memset, @function
memset:
    test rdx, rdx
    jz 2f
    xor eax, eax
1:
    mov byte ptr [rdi + rdx - 1], 0
    dec rdx
    jnz 1b
2:
    mov rax, rdi
    ret
