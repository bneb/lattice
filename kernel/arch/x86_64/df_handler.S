.intel_syntax noprefix
.code64

# =============================================================================
# df_handler.S — #DF (Double Fault) Panic Handler with IST 2
# =============================================================================
#
# Double faults are unrecoverable. This handler prints diagnostic info
# via serial and halts the core. It runs on IST 2, guaranteeing a
# known-good stack even when the original stack was exhausted.
#
# Hardware-pushed #DF IST frame (48 bytes):
#   RSP+0:  Error Code (always 0 for #DF)
#   RSP+8:  RIP  (faulting instruction)
#   RSP+16: CS
#   RSP+24: RFLAGS
#   RSP+32: RSP  (faulting stack)
#   RSP+40: SS
#
# CRITICAL ABI NOTE:
#   The System V x86-64 ABI requires RSP to be 16-byte aligned BEFORE
#   the `call` instruction (which pushes 8 bytes, so the callee enters
#   with RSP ≡ 8 mod 16). We explicitly align RSP before calling the
#   Salt/C handler to prevent #GP from SSE instructions that LLVM may
#   emit for memory operations inside df_handler_c.
# =============================================================================

.global df_isr_wrapper
.extern df_handler_c

df_isr_wrapper:
    # --- RAW HARDWARE TRACE --- (Proof of Life)
    mov dx, 0x3F8
    mov al, 0x41      # 'A'
    out dx, al

    # ── Extract diagnostic info from IST frame ─────────────────────────
    # Error code is at RSP+0, RIP at RSP+8, faulting RSP at RSP+32.
    # We save these before touching RSP.
    mov     rdi, [rsp + 8]      # arg0 = faulting RIP
    mov     rsi, [rsp + 32]     # arg1 = faulting RSP

    mov dx, 0x3F8
    mov al, 0x42      # 'B'
    out dx, al

    # ── Align RSP for System V ABI ─────────────────────────────────────
    # CRITICAL: and rsp, -16 guarantees 16-byte alignment.
    # Without this, LLVM-emitted SSE instructions in df_handler_c will #GP.
    and     rsp, -16

    # ── Call Salt panic handler ────────────────────────────────────────
    # df_handler_c(rip: u64, rsp: u64) — prints diagnostic and never returns.
    call    df_handler_c

    # ── Should never return, but halt defensively ──────────────────────
    cli
.Ldf_halt_loop:
    hlt
    jmp     .Ldf_halt_loop
