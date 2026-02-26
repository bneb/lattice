.intel_syntax noprefix
.code64
.global timer_isr_wrapper
.extern kernel__core__scheduler__notify_timer_tick
.extern preempt_return_to_scheduler

/* 
 * Register-Perfect ISR Wrapper (Intel Syntax):
 * Meticulously saves all GPRs to protect Salt's SSA induction variables.
 *
 * UNIFIED FIBER CHECK:
 *   If GS:[48] (sched_rsp) is non-zero, the timer interrupted a
 *   preemptive fiber running through invoke_preemptive_thread.
 *   Jump to preempt_return_to_scheduler to save thread state,
 *   swap back to the scheduler stack, and return POLL_PENDING.
 *
 *   If GS:[48] == 0, this is a normal interrupt (kernel code or
 *   Ring 3 process). Fall through to the existing ISR logic.
 */
timer_isr_wrapper:
    # ─── SWAPGS Check ───
    # CS is always pushed at [rsp + 8] by hardware.
    # If CS & 3 == 3, we came from Ring 3. Must SWAPGS before touching GS.
    test qword ptr [rsp + 8], 3
    jz .L_timer_from_kernel
    swapgs
.L_timer_from_kernel:

    # 0. Unified fiber sentinel check.
    #    Compare GS:[48] (sched_rsp) directly against zero.
    #    This does NOT touch any registers or the stack.
    #    If non-zero, a unified fiber is running — save its state
    #    and return POLL_PENDING to the scheduler.
    cmp  qword ptr gs:[48], 0
    jne  preempt_return_to_scheduler

    # ── Legacy ISR path (kernel/Ring 3 code interrupted) ──

    # 1. Save all GPRs (Reverse order for Context struct)
    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push rbp
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15

.extern timer_preempt

    # 2. Handshake with the Salt Scheduler
    # Pass pointer to the stack (Context)
    mov rdi, rsp
    call timer_isr

    # 3. Send EOI (End of Interrupt) to Master PIC
    mov al, 0x20
    out 0x20, al

    # 4. Ring 3 process preemption (round-robin scheduling)
    call timer_preempt

    # 4. Restore state (LIFO - Reverse of Push)
    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rbp
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rbx
    pop rax

    # 5. Return from interrupt (Long Mode)
    # ─── SWAPGS Return Check ───
    # Check if we are returning to Ring 3 (CS & 3 == 3)
    test qword ptr [rsp + 8], 3
    jz .L_timer_return_to_kernel
    swapgs
.L_timer_return_to_kernel:
    iretq

