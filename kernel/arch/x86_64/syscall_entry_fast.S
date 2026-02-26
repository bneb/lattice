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

    /* Benchmark escape hatch (RAX=0xBEEF) */
    cmp eax, 0xBEEF
    je .Lbench_escape

    /* Test exit sentinel (RAX=0xDEAD) — store exit code, return to Ring 0 */
    cmp eax, 0xDEAD
    je .Ltest_exit

    /* Fast path — null syscall (RAX=0) */
    test rax, rax
    jz .Lfast_null_ret

    /* sys_exit (RAX=60) — process lifecycle, never returns */
    cmp rax, 60
    je .Lsys_exit

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

    call handle_syscall

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
    mov cr3, rdx                /* Switch to User PML4 */
    mov rdx, gs:[112]           /* Restore RDX from scratch */

    /* SWAPGS: Kernel GS → User GS (before returning to Ring 3) */
    swapgs
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
 * KPTI Scratch Stack — 4096 bytes in .kpti_trampoline_data
 * ═══════════════════════════════════════════════════════════════════════
 * This stack is mapped in BOTH the User PML4 and the Kernel PML4.
 * It provides a safe pivot point for the CR3 swap window between
 * swapgs and mov cr3 in the SYSCALL entry path.
 *
 * Only used for the brief CR3 swap (push/pop rdx). The real kernel
 * stack at gs:[96] is used for all subsequent syscall handling.
 * ═══════════════════════════════════════════════════════════════════════ */
.section .kpti_trampoline_data, "aw"
.align 4096
.global kpti_scratch_stack_bottom
kpti_scratch_stack_bottom:
    .space 4096
.global kpti_scratch_stack_top
kpti_scratch_stack_top:
