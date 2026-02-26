/* kernel/arch/x86_64/syscall_msr_init.S
 * SYSCALL/SYSRET MSR Configuration
 *
 * Programs the four MSRs required for the SYSCALL/SYSRET fast path:
 *   1. EFER.SCE    — Enable SYSCALL instruction
 *   2. STAR        — Segment selectors for SYSCALL/SYSRET transitions
 *   3. LSTAR       — RIP target for SYSCALL (entry point)
 *   4. FMASK       — RFLAGS bits cleared on SYSCALL entry
 *
 * Must be called AFTER GDT/TSS init and BEFORE any Ring 3 transitions.
 *
 * Salt declaration:
 *   extern fn init_syscall_msrs();
 */
.intel_syntax noprefix
.global init_syscall_msrs

/* MSR addresses */
.set MSR_EFER,  0xC0000080
.set MSR_STAR,  0xC0000081
.set MSR_LSTAR, 0xC0000082
.set MSR_FMASK, 0xC0000084

/* EFER bits */
.set EFER_SCE, 0x01   /* System Call Enable */

/* STAR layout:
 *   [63:48] = SYSRET CS base selector  (0x18 — User Code 32, per GDT layout)
 *   [47:32] = SYSCALL CS selector      (0x08 — Kernel Code 64)
 *   [31:0]  = Reserved (SYSCALL target EIP for compat mode, unused in 64-bit)
 *
 * SYSRET computes from [63:48]:
 *   CS (64-bit) = STAR[63:48] + 16 | RPL=3 = 0x18 + 0x10 | 3 = 0x2B ✓
 *   SS          = STAR[63:48] + 8  | RPL=3 = 0x18 + 0x08 | 3 = 0x23 ✓
 */

init_syscall_msrs:
    /* ──── 1. Enable SYSCALL in EFER ──── */
    mov ecx, MSR_EFER
    rdmsr                           /* Read current EFER into EDX:EAX */
    or  eax, EFER_SCE               /* Set SCE bit */
    wrmsr                           /* Write back */

    /* ──── 2. STAR: Segment selectors ──── */
    mov ecx, MSR_STAR
    xor eax, eax                    /* Low 32 bits: reserved (0) */
    mov edx, 0x00180008             /* High 32: SYSRET_base=0x18 | SYSCALL_CS=0x08 */
    wrmsr

    /* ──── 3. LSTAR: SYSCALL entry point ──── */
    mov ecx, MSR_LSTAR
    lea rax, [rip + syscall_entry_fast]
    mov rdx, rax
    shr rdx, 32                     /* EDX = high 32 bits of address */
    wrmsr                           /* EAX already has low 32 bits */

    /* ──── 4. FMASK: Clear IF, DF, TF on SYSCALL entry ──── */
    /*     IF (bit 9)  = 0x200  — disable interrupts in kernel */
    /*     DF (bit 10) = 0x400  — ensure forward string ops */
    /*     TF (bit 8)  = 0x100  — disable single-step */
    mov ecx, MSR_FMASK
    mov eax, 0x700                  /* IF | DF | TF */
    xor edx, edx
    wrmsr

    ret

/* ====================================================================
 * set_syscall_kernel_rsp0 — Update SYSCALL kernel stack pointer
 * ====================================================================
 * Called during context switches to keep syscall_entry_fast's
 * kernel_rsp0_scratch in sync with TSS.rsp0.
 *
 * Salt declaration:
 *   extern fn set_syscall_kernel_rsp0(rsp0: u64);
 * ==================================================================== */
.global set_syscall_kernel_rsp0
set_syscall_kernel_rsp0:
    mov gs:[96], rdi
    ret
