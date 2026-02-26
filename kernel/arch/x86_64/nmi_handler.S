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

nmi_isr_wrapper:
    # ── Paranoid GS Entry ──────────────────────────────────────────────
    # IST frame layout (pushed by hardware):
    #   RSP+0:  RIP
    #   RSP+8:  CS
    #   RSP+16: RFLAGS
    #   RSP+24: RSP (interrupted)
    #   RSP+32: SS
    #
    # Check CS from interrupted frame to determine SWAPGS state.
    # If RPL (bits 1:0 of CS) is non-zero, we interrupted Ring 3.
    test    byte ptr [rsp + 8], 3
    jz      .Lnmi_kernel_gs

    # Ring 3 was interrupted: swap to kernel GS
    swapgs

.Lnmi_kernel_gs:
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
    # We cannot safely preempt it here (NMI is not re-entrant and the
    # fiber's stack may be in a half-swapped state). Set the pending
    # flag for the scheduler to handle later.
    mov     rax, qword ptr gs:[48]
    test    rax, rax
    jz      .Lnmi_no_fiber

    # Unified fiber was running — set nmi_pending flag
    mov     qword ptr gs:[80], 1

.Lnmi_no_fiber:
    # ── NMI acknowledgment is automatic (no EOI needed for NMI) ────────
    inc qword ptr [rip + nmi_diag_count]

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

    # ── Paranoid GS Exit ───────────────────────────────────────────────
    # Mirror the entry check: if we swapped GS on entry, swap back.
    test    byte ptr [rsp + 8], 3
    jz      .Lnmi_no_swapgs_exit
    swapgs

.Lnmi_no_swapgs_exit:
    iretq

.global read_nmi_diag_count
read_nmi_diag_count:
    mov rax, [rip + nmi_diag_count]
    ret

.global clear_nmi_diag_count
clear_nmi_diag_count:
    mov qword ptr [rip + nmi_diag_count], 0
    ret
