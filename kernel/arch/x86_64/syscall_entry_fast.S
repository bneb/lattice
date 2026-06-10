/* kernel/arch/x86_64/syscall_entry_fast.S
 * SYSCALL/SYSRET fast path handler with KPTI scratch stack pivot.
 *
 * On SYSCALL entry (hardware-defined):
 *   RCX = user RIP (return address)
 *   R11 = user RFLAGS
 *   RAX = syscall number
 *   RDI, RSI, RDX, R10 = syscall arguments
 *   IF is cleared by FMASK
 *   CR3 is still the User PML4 (KPTI)
 *
 * KPTI Scratch Stack Pivot:
 *   After SYSCALL, CR3 still points to the User PML4. The real kernel
 *   stack (allocated by PMM/slab) is NOT mapped in the User PML4.
 *   Any stack access (#PF → #DF → Triple Fault) or NMI would be fatal.
 *
 *   Solution: Pivot RSP to a static scratch stack in .kpti_trampoline_data
 *   (mapped in BOTH the User PML4 and the Kernel PML4), swap CR3 to the
 *   Kernel PML4, THEN switch to the real kernel stack.
 *
 * SWAPGS pairing:
 *   Entry: swapgs (User GS → Kernel GS / PerCpuData)
 *   Exit:  swapgs (Kernel GS → User GS) before SYSRET
 */
.intel_syntax noprefix

.section .kpti_trampoline_text, "ax"
.global syscall_entry_fast
.extern handle_syscall
.extern process_exit

syscall_entry_fast:
    /* ═══ PHASE 0: SWAPGS (must be first instruction) ═══ */
    swapgs




    /* ═══ PHASE 1: Save user RSP (GS-relative, no stack needed) ═══ */
    mov gs:[104], rsp                 /* user_rsp_scratch */

    /* ═══ PHASE 2: KPTI Scratch Stack Pivot ═══
     * Pivot to the static scratch stack in .kpti_trampoline_data.
     * This is SAFE because .kpti_trampoline_data is mapped in the
     * User PML4 as supervisor-only, present, writable.
     * The scratch stack is 4096 bytes — enough for the CR3 swap. */
    lea rsp, [rip + kpti_scratch_stack_top]

    /* ═══ PHASE 3: CR3 Swap (now safe to push on scratch stack) ═══ */
    push rdx                          /* Save RDX (clobbered by CR3 ops) */

    mov rdx, cr3
    mov gs:[128], rdx                 /* Save User CR3 to user_cr3_scratch */

    mov rdx, gs:[64]                  /* Load kernel_cr3 from PerCpuData */
    /* bts rdx, 63 removed for QEMU TCG compatibility */
    mov cr3, rdx                      /* Switch to Kernel PML4 */

    pop rdx                           /* Restore RDX from scratch stack */

    /* ═══ PHASE 4: Switch to real kernel stack ═══
     * NOW we are under the Kernel PML4. The dynamically-allocated
     * kernel stack at gs:[96] IS mapped. Safe to use it. */
    mov rsp, gs:[96]                  /* Load kernel RSP from kernel_rsp0_scratch */

    /* ═══ PHASE 5: Save user context on kernel stack ═══ */
    push qword ptr gs:[104]           /* Saved user RSP */
    push rcx                          /* User RIP (return address) */
    push r11                          /* User RFLAGS */

    /* ═══ PHASE 6: Sentinel dispatch ═══ */

#ifndef KERNEL_PRODUCTION
    /* Benchmark escape hatch (RAX=0xBEEF) */
    cmp eax, 0xBEEF
    je .Lbench_escape

    /* Test exit sentinel (RAX=0xDEAD) — store exit code, return to Ring 0 */
    cmp eax, 0xDEAD
    je .Ltest_exit

    /* Fast path — null syscall (RAX=0) */
    test rax, rax
    jz .Lfast_null_ret
#endif

    /* sys_exit (RAX=60) — process lifecycle, never returns */
    cmp rax, 60
    je .Lsys_exit

    /* ═══ Fast-path IPC dispatch (RAX=200, 201) ═══
     * These bypass the slow-path register save to minimize latency.
     * sys_ipc_send needs 4 args: RDI=target, RSI=msg0, RDX=msg1, R10=msg2.
     * We pass R10 as the 4th SysV argument (RCX). */
    cmp rax, 200
    je .Lsys_ipc_send
    cmp rax, 201
    je .Lsys_ipc_recv

    /* ═══ PHASE 7: Slow path — save regs, dispatch to Salt handler ═══ */
    push rdi
    push rsi
    push rdx
    push r8
    push r9
    push r10

    /* Set up arguments for handle_syscall(num, arg0, arg1, arg2) */
    mov r8, rdx                 /* Save arg2 (rdx) before clobbering */
    mov rdx, rsi                /* arg1 -> rdx (3rd param) */
    mov rsi, rdi                /* arg0 -> rsi (2nd param) */
    mov rdi, rax                /* num  -> rdi (1st param) */
    mov rcx, r8                 /* arg2 -> rcx (4th param) */

    /* Align stack to 16 bytes for System V ABI */
    sub rsp, 8

    call handle_syscall

    /* Restore stack alignment */
    add rsp, 8

    /* RAX now holds the syscall return value (SysV ABI).
     * Save it before restoring caller-saved registers. */
    mov gs:[112], rax

    /* Restore caller-saved registers */
    pop r10
    pop r9
    pop r8
    pop rdx
    pop rsi
    pop rdi

    /* Restore syscall return value into RAX for Ring 3 */
    mov rax, gs:[112]

.Lfast_null_ret:
    /* ═══ PHASE 8: Return to Ring 3 via SYSRET ═══ */
    pop r11                     /* Restore RFLAGS */
    pop rcx                     /* Restore return RIP */
    pop rsp                     /* Restore user RSP */

    /* ═══ KPTI: Reverse CR3 Swap ═══
     * RSP is now the user RSP. We cannot use push/pop because the kernel
     * stack is gone. Use GS-segment scratch to save RDX. */
    mov gs:[112], rdx           /* Save RDX to syscall_ret_scratch */
    mov rdx, gs:[128]           /* Load user_cr3_scratch */
    /* bts rdx, 63 removed for QEMU TCG compatibility */
    mov cr3, rdx                /* Switch to User PML4 */
    mov rdx, gs:[112]           /* Restore RDX from scratch */

    /* SWAPGS: Kernel GS → User GS (before returning to Ring 3) */
    swapgs
.global syscall_sysretq_point
syscall_sysretq_point:
    sysretq

.Lsys_exit:
    /* sys_exit: RDI = exit code (already in RDI from SYSCALL ABI).
     * process_exit() NEVER RETURNS. */
    call process_exit
    /* Fallthrough to halt if process_exit somehow returns */
.Lhalt_loop:
    cli
    hlt
    jmp .Lhalt_loop

.Ltest_exit:
    /* Test exit sentinel detected (RAX=0xDEAD).
     * Store RDI (exit code) in ring3_test_exit_code scratch variable,
     * then escape to Ring 0 via bench_ring0_restore.
     * This tests the full SYSCALL argument-passing ABI without
     * invoking process_exit's full lifecycle. */
    mov gs:[120], rdi

    /* We do NOT restore User PML4 here because bench_ring0_restore
     * returns directly to Ring 0 (the test runner).
     *
     * We also do NOT swapgs here! The test runner is in Ring 0 and MUST
     * continue using the Kernel GS base (PerCpuData).
     */
    .extern bench_ring0_restore
    jmp bench_ring0_restore

.Lbench_escape:
    /* Benchmark sentinel detected (RAX=0xBEEF).
     * bench_ring0_restore loads RSP from bench_ring0_rsp_save,
     * discarding all SYSCALL handler state, and returns to the
     * Ring 0 call site of drop_to_ring3_and_bench.
     *
     * Do NOT swapgs! We are returning to Ring 0.
     */
    .extern bench_ring0_restore
    jmp bench_ring0_restore


/* ═══════════════════════════════════════════════════════════════════════
 * Fast-Path IPC Handlers — Surgical Register Orchestration
 * ═══════════════════════════════════════════════════════════════════════
 *
 * INVARIANT: At this point, PHASE 5 has already saved on the kernel stack:
 *     [user RSP]  [user RIP (RCX)]  [user RFLAGS (R11)]
 * So RCX and R11 are expendable — they can be clobbered freely.
 *
 * On the SEND path:
 *   - The Salt handler calls switch_to_process() on success, which
 *     performs proc_context_switch and NEVER returns here.
 *   - We only resume at this instruction pointer when someone schedules
 *     us back. At that point the proc_context_switch restore gives us
 *     a clean stack frame and we fall through to SYSRET.
 *   - If the Salt handler FAILS (invalid target, receiver not blocked),
 *     it returns an error code in RAX and we SYSRET immediately.
 *
 * On the RECV path:
 *   - The Salt handler marks us IPC_BLOCKED and yields. We are parked.
 *   - A future sys_ipc_send unblocks us and switch_to_process resumes
 *     our context. The Salt handler returns sender_pid in RAX.
 * ═══════════════════════════════════════════════════════════════════════ */

.Lsys_ipc_send:
    /* SYS_IPC_SEND (RAX=200)
     *
     * Register state entering this block:
     *   RDI = target_pid       (SYSCALL arg0, maps to SysV arg0)
     *   RSI = msg0             (SYSCALL arg1, maps to SysV arg1)
     *   RDX = msg1             (SYSCALL arg2, maps to SysV arg2)
     *   R10 = msg2             (SYSCALL arg3, must → RCX for SysV arg3)
     *   RCX = user RIP         (saved on stack by PHASE 5, expendable)
     *   R11 = user RFLAGS      (saved on stack by PHASE 5, expendable)
     *
     * Minimal state save: RCX is the only register we clobber that
     * isn't already saved by PHASE 5 or preserved by SysV callee-save.
     * RDI/RSI/RDX are caller-saved per SysV — the Salt handler is
     * free to clobber them. We don't need their original values on
     * return because the user only cares about RAX (return value). */
    .extern sys_ipc_send

    mov rcx, r10            /* R10 (SYSCALL arg3) → RCX (SysV arg3) */

    /* sys_ipc_send(target_pid=RDI, msg0=RSI, msg1=RDX, msg2=RCX)
     *
     * If target is IPC_BLOCKED:
     *   → stores payload in target PCB
     *   → calls switch_to_process(sender, target)
     *   → proc_context_switch saves our callee-saved regs
     *   → CPU jumps to receiver
     *   → WE DO NOT RETURN until someone switches back to us
     *   → When we resume, proc_context_switch restores regs, returns
     *   → RAX = 0 (success)
     *
     * If target is invalid or not blocked:
     *   → returns error code in RAX immediately */
    call sys_ipc_send

    /* We reach here in two cases:
     *  1. Error: sys_ipc_send returned immediately (target invalid)
     *  2. Resume: We were context-switched away, now switched back
     * In both cases, RAX holds the return value for Ring 3. */
    jmp .Lfast_null_ret     /* SYSRET: pops RFLAGS/RIP/RSP from PHASE 5 */

.Lsys_ipc_recv:
    /* SYS_IPC_RECV (RAX=201)
     *
     * Before returning, sys_ipc_recv calls ipc_stage_payload() which
     * writes msg0/1/2 to PerCpuData via gs: segment. We then load those
     * into RDI/RSI/RDX so the user-space receiver sees the full payload
     * in CPU registers after SYSRET.
     *
     * SMP SAFETY: Each core reads from its OWN PerCpuData. No cross-core
     * aliasing. This replaces the global .data variables that were a
     * fatal race condition on multi-core. */
    .extern sys_ipc_recv

    call sys_ipc_recv

    /* RAX = sender_pid.
     * Now inject the payload from THIS core's PerCpuData. */
    mov rdi, gs:[136]       /* ipc_msg0_scratch */
    mov rsi, gs:[144]       /* ipc_msg1_scratch */
    mov rdx, gs:[152]       /* ipc_msg2_scratch */

    jmp .Lfast_null_ret     /* SYSRET: pops RFLAGS/RIP/RSP from PHASE 5 */

/* ═══════════════════════════════════════════════════════════════════════
 * ipc_stage_payload — Salt→Assembly handoff for IPC payload delivery
 * ═══════════════════════════════════════════════════════════════════════
 * Called from sys_ipc_recv in Salt:
 *   ipc_stage_payload(msg0: u64, msg1: u64, msg2: u64)
 * SysV ABI: RDI=msg0, RSI=msg1, RDX=msg2
 *
 * SMP SAFETY: Stores to per-CPU gs: segment — no cross-core aliasing.
 * Each core writes to its own PerCpuData IPC scratch slots.
 * ═══════════════════════════════════════════════════════════════════════ */
.global ipc_stage_payload
ipc_stage_payload:
    mov gs:[136], rdi       /* ipc_msg0_scratch @ PerCpuData+136 */
    mov gs:[144], rsi       /* ipc_msg1_scratch @ PerCpuData+144 */
    mov gs:[152], rdx       /* ipc_msg2_scratch @ PerCpuData+152 */
    ret

/* ═══════════════════════════════════════════════════════════════════════
 * KPTI Scratch Stack — 4096 bytes in .kpti_trampoline_data
 * ═══════════════════════════════════════════════════════════════════════
 * This stack is mapped in BOTH the User PML4 and the Kernel PML4.
 * It provides a safe pivot point for the CR3 swap window between
 * swapgs and mov cr3 in the SYSCALL entry path.
 *
 * Only used for the brief CR3 swap (push/pop rdx). The real kernel
 * stack at gs:[96] is used for all subsequent syscall handling.
 * ═══════════════════════════════════════════════════════════════════════ */
.align 4096
.global kpti_scratch_stack_bottom
kpti_scratch_stack_bottom:
    .space 4096
.global kpti_scratch_stack_top
kpti_scratch_stack_top:
