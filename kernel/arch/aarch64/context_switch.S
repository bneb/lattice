/* =============================================================================
 * kernel/arch/aarch64/context_switch.S
 * AArch64 Exception Vector Table + Preemptive Context Switch
 * =============================================================================
 *
 * ARMv8-A Exception Model:
 *   When an exception fires (timer IRQ, SVC, etc.), the CPU:
 *     1. Saves PC to ELR_EL1 (Exception Link Register)
 *     2. Saves PSTATE to SPSR_EL1 (Saved Program Status Register)
 *     3. Jumps to the VBAR_EL1 + offset (based on exception type + source EL)
 *
 * Register Convention:
 *   X0-X30:  31 general-purpose registers (X30 = LR)
 *   SP_EL0:  User-mode stack pointer
 *   SP_EL1:  Kernel-mode stack pointer
 *   ELR_EL1: Saved PC for exception return
 *   SPSR_EL1: Saved PSTATE for exception return
 *
 * Context Frame Layout (288 bytes):
 *   [SP + 0]    X0
 *   [SP + 8]    X1
 *   ...
 *   [SP + 240]  X30 (LR)
 *   [SP + 248]  XZR (padding for 16-byte alignment)
 *   [SP + 256]  ELR_EL1
 *   [SP + 264]  SPSR_EL1
 *   [SP + 272]  SP_EL0 (user stack, saved for EL0 → EL1 transitions)
 *   [SP + 280]  (padding)
 *
 * VBAR_EL1 Alignment: Each vector entry is 128 bytes (32 instructions).
 * The table base must be 2KB (0x800) aligned.
 * =========================================================================== */

/* ===========================================================================
 * Exception Vector Table — VBAR_EL1
 * ===========================================================================
 * Offsets:
 *   0x000: Synchronous, Current EL with SP_EL0
 *   0x080: IRQ,         Current EL with SP_EL0
 *   0x100: FIQ,         Current EL with SP_EL0
 *   0x180: SError,      Current EL with SP_EL0
 *   0x200: Synchronous, Current EL with SP_ELx
 *   0x280: IRQ,         Current EL with SP_ELx
 *   0x300: FIQ,         Current EL with SP_ELx
 *   0x380: SError,      Current EL with SP_ELx
 *   0x400: Synchronous, Lower EL using AArch64
 *   0x480: IRQ,         Lower EL using AArch64
 *   0x500: FIQ,         Lower EL using AArch64
 *   0x580: SError,      Lower EL using AArch64
 *   0x600: Synchronous, Lower EL using AArch32
 *   0x680: IRQ,         Lower EL using AArch32
 *   0x700: FIQ,         Lower EL using AArch32
 *   0x780: SError,      Lower EL using AArch32
 * =========================================================================== */

.section .text
.global aarch64_vector_table
.balign 2048                    /* VBAR_EL1 requires 2KB alignment */

aarch64_vector_table:

/* --- Current EL with SP_EL0 (unused in kernel, we use SP_ELx) --- */
.balign 128
curr_el_sp0_sync:
    b   unhandled_exception

.balign 128
curr_el_sp0_irq:
    b   unhandled_exception

.balign 128
curr_el_sp0_fiq:
    b   unhandled_exception

.balign 128
curr_el_sp0_serror:
    b   unhandled_exception

/* --- Current EL with SP_ELx (kernel-to-kernel exceptions) --- */
.balign 128
curr_el_spx_sync:
    b   aarch64_trap_entry_sync

.balign 128
curr_el_spx_irq:
    b   aarch64_trap_entry_irq

.balign 128
curr_el_spx_fiq:
    b   unhandled_exception

.balign 128
curr_el_spx_serror:
    b   unhandled_exception

/* --- Lower EL using AArch64 (EL0 → EL1, user-to-kernel) --- */
.balign 128
lower_el_aarch64_sync:
    b   aarch64_trap_entry_sync

.balign 128
lower_el_aarch64_irq:
    b   aarch64_trap_entry_irq

.balign 128
lower_el_aarch64_fiq:
    b   unhandled_exception

.balign 128
lower_el_aarch64_serror:
    b   unhandled_exception

/* --- Lower EL using AArch32 (not supported) --- */
.balign 128
lower_el_aarch32_sync:
    b   unhandled_exception

.balign 128
lower_el_aarch32_irq:
    b   unhandled_exception

.balign 128
lower_el_aarch32_fiq:
    b   unhandled_exception

.balign 128
lower_el_aarch32_serror:
    b   unhandled_exception


/* ===========================================================================
 * aarch64_trap_entry_irq — Timer IRQ Entry Point
 * ===========================================================================
 * 1. Allocate 288-byte context frame on kernel stack
 * 2. Save all 31 GPRs using STP (Store Pair) for max memory bandwidth
 * 3. Save ELR_EL1, SPSR_EL1, SP_EL0
 * 4. Pass context pointer (SP) to Salt scheduler in X0
 * 5. On return, X0 = new context pointer (may be different core/fiber)
 * 6. Restore all state and ERET
 * =========================================================================== */
.global aarch64_trap_entry_irq
aarch64_trap_entry_irq:
    /* 1. Allocate context frame (288 bytes, 16-byte aligned) */
    sub     sp, sp, #288

    /* 2. Save General Purpose Registers using Store Pair */
    stp     x0,  x1,  [sp, #0]
    stp     x2,  x3,  [sp, #16]
    stp     x4,  x5,  [sp, #32]
    stp     x6,  x7,  [sp, #48]
    stp     x8,  x9,  [sp, #64]
    stp     x10, x11, [sp, #80]
    stp     x12, x13, [sp, #96]
    stp     x14, x15, [sp, #112]
    stp     x16, x17, [sp, #128]
    stp     x18, x19, [sp, #144]
    stp     x20, x21, [sp, #160]
    stp     x22, x23, [sp, #176]
    stp     x24, x25, [sp, #192]
    stp     x26, x27, [sp, #208]
    stp     x28, x29, [sp, #224]
    stp     x30, xzr, [sp, #240]    /* X30 = LR, XZR = padding */

    /* 3. Save Exception State */
    mrs     x21, elr_el1
    mrs     x22, spsr_el1
    mrs     x23, sp_el0
    stp     x21, x22, [sp, #256]
    stp     x23, xzr, [sp, #272]

    /* 4. Pass context pointer to the Salt scheduler */
    mov     x0, sp
    bl      salt_handle_irq

    /* salt_handle_irq returns the new context pointer in X0.
     * If the scheduler switched fibers, X0 != old SP. */
    mov     sp, x0

    /* 5. Restore Exception State */
    ldp     x21, x22, [sp, #256]
    ldp     x23, xzr, [sp, #272]
    msr     elr_el1, x21
    msr     spsr_el1, x22
    msr     sp_el0, x23

    /* 6. Restore General Purpose Registers */
    ldp     x30, xzr, [sp, #240]
    ldp     x28, x29, [sp, #224]
    ldp     x26, x27, [sp, #208]
    ldp     x24, x25, [sp, #192]
    ldp     x22, x23, [sp, #176]
    ldp     x20, x21, [sp, #160]
    ldp     x18, x19, [sp, #144]
    ldp     x16, x17, [sp, #128]
    ldp     x14, x15, [sp, #112]
    ldp     x12, x13, [sp, #96]
    ldp     x10, x11, [sp, #80]
    ldp     x8,  x9,  [sp, #64]
    ldp     x6,  x7,  [sp, #48]
    ldp     x4,  x5,  [sp, #32]
    ldp     x2,  x3,  [sp, #16]
    ldp     x0,  x1,  [sp, #0]

    /* Deallocate context frame */
    add     sp, sp, #288

    /* 7. Exception Return — restores PC from ELR_EL1, PSTATE from SPSR_EL1 */
    eret


/* ===========================================================================
 * aarch64_trap_entry_sync — Synchronous Exception Entry
 * ===========================================================================
 * Handles SVC (system calls), data aborts, instruction aborts, etc.
 * Same save/restore pattern as IRQ, but calls salt_handle_sync.
 * =========================================================================== */
.global aarch64_trap_entry_sync
aarch64_trap_entry_sync:
    sub     sp, sp, #288

    stp     x0,  x1,  [sp, #0]
    stp     x2,  x3,  [sp, #16]
    stp     x4,  x5,  [sp, #32]
    stp     x6,  x7,  [sp, #48]
    stp     x8,  x9,  [sp, #64]
    stp     x10, x11, [sp, #80]
    stp     x12, x13, [sp, #96]
    stp     x14, x15, [sp, #112]
    stp     x16, x17, [sp, #128]
    stp     x18, x19, [sp, #144]
    stp     x20, x21, [sp, #160]
    stp     x22, x23, [sp, #176]
    stp     x24, x25, [sp, #192]
    stp     x26, x27, [sp, #208]
    stp     x28, x29, [sp, #224]
    stp     x30, xzr, [sp, #240]

    mrs     x21, elr_el1
    mrs     x22, spsr_el1
    mrs     x23, sp_el0
    stp     x21, x22, [sp, #256]
    stp     x23, xzr, [sp, #272]

    mov     x0, sp
    bl      salt_handle_sync

    mov     sp, x0

    ldp     x21, x22, [sp, #256]
    ldp     x23, xzr, [sp, #272]
    msr     elr_el1, x21
    msr     spsr_el1, x22
    msr     sp_el0, x23

    ldp     x30, xzr, [sp, #240]
    ldp     x28, x29, [sp, #224]
    ldp     x26, x27, [sp, #208]
    ldp     x24, x25, [sp, #192]
    ldp     x22, x23, [sp, #176]
    ldp     x20, x21, [sp, #160]
    ldp     x18, x19, [sp, #144]
    ldp     x16, x17, [sp, #128]
    ldp     x14, x15, [sp, #112]
    ldp     x12, x13, [sp, #96]
    ldp     x10, x11, [sp, #80]
    ldp     x8,  x9,  [sp, #64]
    ldp     x6,  x7,  [sp, #48]
    ldp     x4,  x5,  [sp, #32]
    ldp     x2,  x3,  [sp, #16]
    ldp     x0,  x1,  [sp, #0]

    add     sp, sp, #288
    eret


/* ===========================================================================
 * unhandled_exception — Infinite loop for unimplemented vectors
 * =========================================================================== */
unhandled_exception:
    wfe
    b       unhandled_exception


/* ===========================================================================
 * aarch64_install_vectors — Write VBAR_EL1
 * ===========================================================================
 * Called during kernel init to install the exception vector table.
 * =========================================================================== */
.global aarch64_install_vectors
aarch64_install_vectors:
    adrp    x0, aarch64_vector_table
    add     x0, x0, :lo12:aarch64_vector_table
    msr     vbar_el1, x0
    isb
    ret
