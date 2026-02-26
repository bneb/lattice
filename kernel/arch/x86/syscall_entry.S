/* kernel/arch/x86/syscall_entry.S */
.intel_syntax noprefix
.global syscall_entry
.extern handle_syscall

syscall_entry:
    /* State Preservation: Save all general-purpose registers */
    /* This ensures SSA induction variables and other optimizations are preserved */
    push rdi
    push rsi
    push rdx
    push rcx
    push rax
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15
    push rbp
    push rbx

    /* Syscall number is in EAX (RAX) */
    mov rdi, rax
    call handle_syscall

    /* RAX now holds the syscall return value.
     * We must skip restoring the saved RAX (at stack offset)
     * so the return value propagates to Ring 3 via IRETQ. */

    /* Restore all registers EXCEPT RAX */
    pop rbx
    pop rbp
    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    add rsp, 8              /* Skip saved RAX — keep return value */
    pop rcx
    pop rdx
    pop rsi
    pop rdi

    /* Return from interrupt */
    iretq
