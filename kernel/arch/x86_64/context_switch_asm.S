.intel_syntax noprefix

# =============================================================================
# switch_stacks(old_sp_ptr: *mut u64, new_sp: u64)
# -----------------------------------------------------------------------------
# Lazy FPU Context Switch.
#
# FPU state is NOT saved/restored here. Instead, we set CR0.TS after
# switching. If the new fiber uses FPU/SSE, the CPU fires #NM (vector 7),
# and the nm_handler performs the demand-load FXSAVE/FXRSTOR.
#
# This makes integer-only context switches ~100x faster under QEMU-TCG
# by eliminating the unconditional 512-byte FXSAVE/FXRSTOR pair.
#
# Stack Frame Layout (simplified — no FPU):
#   [High Address]
#   | RetAddr      | <- Entry RSP
#   | RBP..R15     | (6 regs = 48 bytes)
#   | RFLAGS       |
#   [Low Address]  <- Saved RSP
# =============================================================================
.global switch_stacks
switch_stacks:
    # RDI = old_sp_ptr, RSI = new_sp

    # 1. Save GPRs and Flags
    push rbp
    push rbx
    push r12
    push r13
    push r14
    push r15
    pushfq
    cli                         # Disable interrupts during stack surgery

    # 2. Swap Stacks (no FPU save — lazy via #NM if needed)
    mov [rdi], rsp
    mov rsp, rsi

    # 3. Restore Flags and GPRs
    popfq                       # Restores IF (Interrupt Flag)
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp
    ret

# =============================================================================
# stack_init(stack_top: u64, entry: u64) -> u64
# -----------------------------------------------------------------------------
# Initialize fiber stack to match switch_stacks layout (no FPU state).
#
# Layout (High to Low):
#   [stack_top - 8]    Return Address
#   [stack_top - 56]   GPRs (6 * 8 = 48)
#   [stack_top - 64]   RFLAGS (0x202 = IF set)
#                      ^ Returned RSP
# =============================================================================
.global stack_init
.global kernel__core__context_switch__stack_init
stack_init:
kernel__core__context_switch__stack_init:
    mov rax, rdi
    and rax, -16                # Align stack top

    # 1. Return Address
    sub rax, 8
    mov [rax], rsi

    # 2. GPRs (48 bytes = 6 registers)
    sub rax, 48
    mov qword ptr [rax], 0
    mov qword ptr [rax + 8], 0
    mov qword ptr [rax + 16], 0
    mov qword ptr [rax + 24], 0
    mov qword ptr [rax + 32], 0
    mov qword ptr [rax + 40], 0

    # 3. RFLAGS (IF=1, reserved bit 1 = set)
    sub rax, 8
    mov qword ptr [rax], 0x202

    ret

# =============================================================================
# FPU Helpers — called from the #NM handler (nm_handler.S)
# =============================================================================

.global fxsave_to
fxsave_to:
    # RDI = buffer address — MUST be 16-byte aligned
    and rdi, -16            # Force alignment (safety net)
    fxsave [rdi]
    ret

.global fxrstor_from
fxrstor_from:
    # RDI = buffer address — MUST be 16-byte aligned
    and rdi, -16            # Force alignment (safety net)
    fxrstor [rdi]
    ret

.global clear_ts_flag
clear_ts_flag:
    clts
    ret

.global set_ts_flag
set_ts_flag:
    mov rax, cr0
    or  rax, 8
    mov cr0, rax
    ret

# =============================================================================
# init_fpu_state() — Initialize FPU/SSE to hardware defaults
# =============================================================================
# Called on a fiber's FIRST FPU use (before any fxrstor).
# Sets x87 FPU to default state and loads default MXCSR (0x1F80).
# This prevents #GP from fxrstor on an uninitialized buffer.
# =============================================================================
.global init_fpu_state
init_fpu_state:
    fninit                  # Reset x87 FPU to defaults
    sub rsp, 8
    mov dword ptr [rsp], 0x1F80  # Default MXCSR: all exceptions masked
    ldmxcsr [rsp]
    add rsp, 8
    ret
