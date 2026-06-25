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
.section .kpti_trampoline_text, "ax"
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

    /* Store current RSP to *old_rsp_ptr if old_rsp_ptr != 0 */
    test rdi, rdi
    jz 1f
    mov [rdi], rsp
1:
    /* Switch to new kernel stack */
    mov rsp, rsi

    /* Save CR3 (RDX) before debug serial output clobbers dx */
    push rdx

    mov dx, 0x3F8
    mov al, 0x31  /* '1' */
    out dx, al
    mov al, 10
    out dx, al

    /* Restore CR3 value and switch address space */
    pop rdx
    mov cr3, rdx

    mov dx, 0x3F8
    mov al, 0x32  /* '2' */
    out dx, al
    mov al, 10
    out dx, al

    /* Update kernel_rsp0_scratch for SYSCALL entry path (Per-CPU) */
    mov gs:[96], rcx

    mov dx, 0x3F8
    mov al, 0x33  /* '3' */
    out dx, al
    mov al, 10
    out dx, al

    /* Restore callee-saved registers from new stack */
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp

    /* Clear scratch registers to prevent information leaks to the new process */
    xor eax, eax
    xor ecx, ecx
    xor edx, edx
    xor esi, esi
    xor edi, edi
    xor r8d, r8d
    xor r9d, r9d
    xor r10d, r10d
    xor r11d, r11d

    ret

/* =============================================================================
 * proc_trampoline — First-time entry to a new Ring 3 process
 *
 * Stack has IRETQ frame: RIP, CS(0x1B), RFLAGS(0x202), RSP, SS(0x23)
 * ============================================================================= */
proc_trampoline:
    mov dx, 0x3F8
    mov al, 0x54  /* 'T' */
    out dx, al
    mov al, 10
    out dx, al

    /* Ring 3 transition: Data segments */
    mov ax, 0x23
    mov ds, ax
    mov es, ax
    mov fs, ax

    mov dx, 0x3F8
    mov al, 0x53  /* 'S' */
    out dx, al
    mov al, 10
    out dx, al

    swapgs

    mov dx, 0x3F8
    mov al, 0x49  /* 'I' */
    out dx, al
    mov al, 10
    out dx, al

.global proc_trampoline_iretq
    /* Clear CR0.TS so SSE instructions don't #NM in Ring 3 */
    clts
    /* Flush TLB before IRETQ: reload CR3 to ensure user page walks succeed */
    mov rax, cr3
    mov cr3, rax
proc_trampoline_iretq:
    iretq

/* =============================================================================
 * moe_trampoline — Entry to Ring 3 with 3 Arguments for MoE Pipeline
 *
 * Stack has IRETQ frame + 3 arguments:
 *   [rsp]      = arg1 (RDI) - state_ptr
 *   [rsp+8]    = arg2 (RSI) - bar_out
 *   [rsp+16]   = arg3 (RDX) - poll_out
 *   [rsp+24..] = IRETQ frame (RIP, CS, RFLAGS, RSP, SS)
 * ============================================================================= */
.global moe_trampoline
moe_trampoline:
    pop rdi
    pop rsi
    pop rdx
    mov ax, 0x23
    mov ds, ax
    mov es, ax
    mov fs, ax
    swapgs
.global moe_trampoline_iretq
moe_trampoline_iretq:
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
    mov dx, 0x3F8
    mov al, 0x33  /* '3' */
    out dx, al
    pop rdi
    pop rsi
    pop rdx
    mov dx, 0x3F8
    mov al, 0x34  /* '4' */
    out dx, al
    sti
    mov dx, 0x3F8
    mov al, 0x35  /* '5' */
    out dx, al
    pop rax
    call rax
    /* If the kernel thread returns, yield to the scheduler instead of
     * crashing via ud2. This allows daemons at higher slots to run. */
.extern schedule_next
    call schedule_next
    /* schedule_next should never return here — it switches context.
     * If it does (all processes exited), halt forever. */
1:  hlt
    jmp 1b
