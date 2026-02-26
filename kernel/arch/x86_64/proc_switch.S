/* =============================================================================
 * kernel/arch/x86_64/proc_switch.S
 * Process Context Switch with CR3 update
 * =============================================================================
 *
 * proc_context_switch(old_rsp_ptr: *mut u64, new_rsp: u64,
 *                     new_cr3: u64, new_rsp0: u64)
 *
 * Saves callee-saved registers, stores RSP, switches to new RSP + CR3,
 * updates kernel_rsp0_scratch for SYSCALL entry, restores registers, returns.
 *
 * proc_trampoline()
 *
 * Sets user data segments and IRETs into Ring 3 (first-time dispatch).
 * =============================================================================
 */

.intel_syntax noprefix
.global proc_context_switch
.global proc_trampoline

proc_context_switch:
    /* RDI = old_rsp_ptr, RSI = new_rsp, RDX = new_cr3, RCX = new_rsp0 */

    /* Save callee-saved registers */
    push rbp
    push rbx
    push r12
    push r13
    push r14
    push r15

    /* Store current RSP to *old_rsp_ptr */
    mov [rdi], rsp

    /* Switch to new kernel stack */
    mov rsp, rsi

    /* Switch CR3 (address space) */
    mov cr3, rdx

    /* Update kernel_rsp0_scratch for SYSCALL entry path (Per-CPU) */
    mov gs:[96], rcx

    /* Restore callee-saved registers from new stack */
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp

    ret

/* =============================================================================
 * proc_trampoline — First-time entry to a new Ring 3 process
 *
 * Stack has IRETQ frame: RIP, CS(0x1B), RFLAGS(0x202), RSP, SS(0x23)
 * ============================================================================= */
proc_trampoline:
    mov ax, 0x23
    mov ds, ax
    mov es, ax
    mov fs, ax
    swapgs
    iretq

/* =============================================================================
 * kernel_trampoline — First-time entry to a Ring 0 kernel thread (Task 0)
 *
 * Unlike proc_trampoline which IRETs to Ring 3, this stays in Ring 0.
 * The stack has the kernel thread's entry function address at [rsp].
 * We enable interrupts (IF=1) so hardware can preempt event processing,
 * then call the entry function (which should never return).
 * ============================================================================= */
.global kernel_trampoline

kernel_trampoline:
    sti                     /* Enable interrupts — Task 0 runs with IF=1 */
    pop rax                 /* Pop the entry function address from the stack */
    call rax                /* Call kernel_event_loop (never returns) */
    ud2                     /* Should never reach here */
