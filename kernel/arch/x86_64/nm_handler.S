.intel_syntax noprefix

# =============================================================================
# nm_handler.S — #NM (Device Not Available) ISR for Lazy FPU Switching
# =============================================================================
#
# Fired when a fiber executes an FPU/SSE instruction after a context switch
# set CR0.TS. This is the demand-load path:
#
#   1. clts           — clear CR0.TS so FPU instructions work
#   2. Save old FPU owner's state via FXSAVE (if any)
#   3. Restore current fiber's FPU state via FXRSTOR (if any)
#   4. Update FPU_OWNER to the current fiber
#   5. iretq
#
# Global variables (defined in scheduler or nm_fpu.salt):
#   FPU_OWNER      — fiber ID of the last FPU user (u64, -1 = none)
#   FPU_BUFFERS    — array of 512-byte aligned buffers per fiber
#   CURRENT_FIBER  — current fiber ID (from scheduler)
# =============================================================================

.global nm_isr_wrapper
.extern nm_handler_c

nm_isr_wrapper:
    # ─── SWAPGS Entry Check ───
    # If interrupted from Ring 3 (CS & 3 == 3), swap to kernel GS
    test qword ptr [rsp + 8], 3
    jz .L_nm_from_kernel
    swapgs
.L_nm_from_kernel:

    # Save scratch registers (interrupt context)
    push rax
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11

    # Call the Salt/C handler that manages FPU ownership
    call nm_handler_c

    # Restore scratch registers
    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rax

    # ─── SWAPGS Exit Check ───
    # If returning to Ring 3, restore the user GS
    test qword ptr [rsp + 8], 3
    jz .L_nm_return_kernel
    swapgs
.L_nm_return_kernel:
    iretq
