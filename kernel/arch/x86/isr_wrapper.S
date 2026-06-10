.intel_syntax noprefix
.code64
.section .kpti_trampoline_text, "ax"
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
    /* KPTI CR3 Switch to Kernel PML4 */
    push rax
    mov rax, cr3
    mov gs:[128], rax
    mov rax, gs:[64]
    /* bts rax, 63 removed */
    mov cr3, rax
    pop rax
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
    # Ensure 16-byte stack alignment for C ABI
    mov rbx, rsp
    and rsp, -16
    call timer_isr
    mov rsp, rbx

    # 3. Send EOI to BOTH PIC and LAPIC for dual-mode compatibility.
    mov al, 0x20
    out 0x20, al
    mov rbx, rsp
    and rsp, -16
    call apic_send_eoi
    mov rsp, rbx

    # 4. Ring 3 process preemption (round-robin scheduling)
    # Check if we interrupted Ring 3. CS is at [rsp + 15*8 + 8] = [rsp + 128]
    test qword ptr [rsp + 128], 3
    jz .L_skip_preempt
    mov rbx, rsp
    and rsp, -16
    call timer_preempt
    mov rsp, rbx
.L_skip_preempt:

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
    /* KPTI CR3 Switch to User PML4 */
    push rax
    mov rax, gs:[128]
    /* bts rax, 63 removed */
    mov cr3, rax
    pop rax
    swapgs
.L_timer_return_to_kernel:
    iretq


.global tlb_shootdown_wrapper
.extern apic_send_eoi
.extern ipi_ack_tlb_shootdown

/*
 * TLB Shootdown IPI Handler
 * Reloads CR3 to flush the TLB across cores, then acknowledges the APIC.
 */
tlb_shootdown_wrapper:
    # ─── SWAPGS Check ───
    test qword ptr [rsp + 8], 3
    jz .L_tlb_from_kernel
    swapgs
    /* KPTI CR3 Switch to Kernel PML4 */
    push rax
    mov rax, cr3
    mov gs:[128], rax
    mov rax, gs:[64]
    /* bts rax, 63 removed */
    mov cr3, rax
    pop rax
.L_tlb_from_kernel:

    # Save caller-saved registers before any C calls or register clobbering
    push rax
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11

    # 1. Handle TLB shootdown (reloads CR3 and decrements ACK count)
    call ipi_ack_tlb_shootdown

    # 2. Handshake with APIC (Send EOI)
    call apic_send_eoi

    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rax

    # ─── SWAPGS Return Check ───
    test qword ptr [rsp + 8], 3
    jz .L_tlb_return_to_kernel
    /* KPTI CR3 Switch to User PML4 */
    push rax
    mov rax, gs:[128]
    /* bts rax, 63 removed */
    mov cr3, rax
    pop rax
    swapgs
.L_tlb_return_to_kernel:
    iretq
