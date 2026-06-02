.intel_syntax noprefix
.code64

# =============================================================================
# nmi_handler.S — NMI (Non-Maskable Interrupt) Handler with IST + Paranoid Entry
# =============================================================================
#
# This handler runs on IST 1, guaranteeing a known-good stack regardless of
# the interrupted context's stack state. This is critical because NMIs can
# fire during:
#   - CLI windows (invoke_preemptive_thread GPR pop sequence)
#   - Between SYSCALL entry and SWAPGS
#   - During any kernel or user code
#
# PARANOID ENTRY PATTERN (from Linux kernel):
#   The NMI can interrupt Ring 3 before SWAPGS has executed, leaving GS
#   pointing at user-space data. We check CS from the hardware-pushed
#   interrupt frame to determine which GS base is active:
#     CS & 3 == 3  →  Ring 3 interrupted, need SWAPGS to get kernel GS
#     CS & 3 == 0  →  Ring 0 interrupted, kernel GS already active
#
# UNIFIED FIBER AWARENESS:
#   If GS:[48] (sched_rsp) is non-zero, a unified fiber was running via
#   invoke_preemptive_thread. We set GS:[80] (nmi_pending) so the scheduler
#   can handle deferred NMI processing on the next tick.
#
# CRITICAL: This handler must NOT:
#   - Allocate memory (PMM may be mid-mutation)
#   - Touch lock-free structures (may be mid-CAS)
#   - Call any Salt function that allocates
#   It is strictly: save regs → check state → set flag → restore → iretq
# =============================================================================

.section .data
.global nmi_diag_count
nmi_diag_count:
    .quad 0

.text
.global nmi_isr_wrapper
.extern syscall_entry_fast
.extern syscall_sysretq_point
.extern proc_trampoline_iretq
.extern moe_trampoline_iretq
.extern bench_trampoline_iretq

nmi_isr_wrapper:
    # ── Paranoid GS Entry ──────────────────────────────────────────────
    # IST frame layout (pushed by hardware):
    #   RSP+0:  RIP
    #   RSP+8:  CS
    #   RSP+16: RFLAGS
    #   RSP+24: RSP (interrupted)
    #   RSP+32: SS
    
    # We must preserve ALL registers. Push two scratch registers so we can check RIP
    push    r11
    push    r10

    # If RPL (bits 1:0 of CS) is non-zero, we interrupted Ring 3.
    # In this case, we definitely need to SWAPGS.
    test    byte ptr [rsp + 16 + 8], 3
    jnz     .Lentry_needs_swapgs

    # If CS is 0 (Ring 0), we STILL might need to SWAPGS if we hit the precise window
    # between SYSCALL and SWAPGS, or SWAPGS and SYSRETQ.
    mov     r11, [rsp + 16] # RIP is at RSP + 16 (since we pushed two 8-byte regs)
    lea     r10, [rip + syscall_entry_fast]
    cmp     r11, r10
    je      .Lentry_needs_swapgs

    lea     r10, [rip + syscall_sysretq_point]
    cmp     r11, r10
    je      .Lentry_needs_swapgs

    lea     r10, [rip + proc_trampoline_iretq]
    cmp     r11, r10
    je      .Lentry_needs_swapgs

    lea     r10, [rip + moe_trampoline_iretq]
    cmp     r11, r10
    je      .Lentry_needs_swapgs

    lea     r10, [rip + bench_trampoline_iretq]
    cmp     r11, r10
    je      .Lentry_needs_swapgs

    # No SWAPGS needed
    jmp     .Lentry_no_swapgs

.Lentry_needs_swapgs:
    swapgs

.Lentry_no_swapgs:
    # Pop scratch registers so we can use the regular save sequence
    pop     r10
    pop     r11

    # ── Save scratch registers ─────────────────────────────────────────
    push    rax
    push    rcx
    push    rdx
    push    rsi
    push    rdi
    push    r8
    push    r9
    push    r10
    push    r11

    # ── Check unified fiber sentinel ───────────────────────────────────
    # GS:[48] = sched_rsp. If non-zero, a unified fiber is running.
    mov     rax, qword ptr gs:[48]
    test    rax, rax
    jz      .Lnmi_no_fiber

    # Unified fiber was running — set nmi_pending flag
    mov     qword ptr gs:[80], 1

.Lnmi_no_fiber:
    # ── NMI acknowledgment is automatic (no EOI needed for NMI) ────────
    inc qword ptr [rip + nmi_diag_count]

    # ── Paranoid GS Exit Check (BEFORE POPPING REGS) ───────────────────
    # We pushed 9 registers (72 bytes). Hardware frame starts at RSP + 72.
    test    byte ptr [rsp + 72 + 8], 3
    jnz     .Lexit_needs_swapgs

    mov     r11, [rsp + 72] # RIP
    lea     r10, [rip + syscall_entry_fast]
    cmp     r11, r10
    je      .Lexit_needs_swapgs

    lea     r10, [rip + syscall_sysretq_point]
    cmp     r11, r10
    je      .Lexit_needs_swapgs

    lea     r10, [rip + proc_trampoline_iretq]
    cmp     r11, r10
    je      .Lexit_needs_swapgs

    lea     r10, [rip + moe_trampoline_iretq]
    cmp     r11, r10
    je      .Lexit_needs_swapgs

    lea     r10, [rip + bench_trampoline_iretq]
    cmp     r11, r10
    je      .Lexit_needs_swapgs

    jmp     .Lexit_no_swapgs

.Lexit_needs_swapgs:
    # We swapped on entry, so swap back on exit.
    swapgs

.Lexit_no_swapgs:
    # ── Restore scratch registers ──────────────────────────────────────
    pop     r11
    pop     r10
    pop     r9
    pop     r8
    pop     rdi
    pop     rsi
    pop     rdx
    pop     rcx
    pop     rax

    iretq

.global read_nmi_diag_count
read_nmi_diag_count:
    mov rax, [rip + nmi_diag_count]
    ret

.global clear_nmi_diag_count
clear_nmi_diag_count:
    mov qword ptr [rip + nmi_diag_count], 0
    ret
