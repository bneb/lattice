/* kernel/arch/x86_64/preempt_stub.S
 *
 * Preemptive ABI Wrapper — Makes hardware threads look like async step_fn calls.
 *
 * This file implements the "assembly illusion" that unifies preemptive and
 * cooperative scheduling behind a single function pointer ABI:
 *
 *   invoke_preemptive_thread(ctx_ptr: u64) -> i64
 *
 * The scheduler calls this exactly like an async step_fn. It resumes the
 * preemptive thread via IRETQ. When the timer fires, the ISR saves state
 * and returns POLL_PENDING (0) to the scheduler as if the function returned
 * normally. When the thread exits, it returns POLL_READY (1).
 *
 * IRETQ Unification:
 *   Both first-launch and resume-after-preemption use the same stack frame:
 *   [GPR15..GPR0] + [RIP, CS, RFLAGS, RSP, SS]
 *   This erases the distinction between "new" and "preempted" threads.
 *
 * Per-CPU Side Channels (GS segment):
 *   GS:[48] = sched_rsp   — Scheduler's RSP, set before entering thread.
 *                            Non-zero = "a unified fiber is running."
 *   GS:[56] = saved_thread_rsp — Thread's saved RSP after preemption.
 *                                 Read by sched_yield() to update fiber.stack_ptr.
 *
 * GDT Selectors (from kernel/arch/x86/boot.S):
 *   0x08 = Kernel Code 64 (DPL 0, L=1)
 *   0x10 = Kernel Data    (DPL 0)
 */
.intel_syntax noprefix

.set KERNEL_CS, 0x08
.set KERNEL_SS, 0x10
.set GS_SCHED_RSP, 48
.set GS_SAVED_THREAD_RSP, 56

/* ====================================================================
 * invoke_preemptive_thread(ctx_ptr: u64) -> i64
 * ====================================================================
 * RDI = ctx_ptr (pointer to thread's saved stack: GPRs + IRETQ frame)
 * Returns: RAX = POLL_PENDING (0) or POLL_READY (1)
 *
 * The scheduler calls this via invoke_task(step_fn, ctx_ptr).
 * invoke_task moves ctx_ptr into RDI before calling us.
 *
 * Stack frame on entry (ctx_ptr points here):
 *   [ctx_ptr + 0]    RAX   (saved GPR)
 *   [ctx_ptr + 8]    RBX
 *   [ctx_ptr + 16]   RCX
 *   [ctx_ptr + 24]   RDX
 *   [ctx_ptr + 32]   RSI
 *   [ctx_ptr + 40]   RDI
 *   [ctx_ptr + 48]   RBP
 *   [ctx_ptr + 56]   R8
 *   [ctx_ptr + 64]   R9
 *   [ctx_ptr + 72]   R10
 *   [ctx_ptr + 80]   R11
 *   [ctx_ptr + 88]   R12
 *   [ctx_ptr + 96]   R13
 *   [ctx_ptr + 104]  R14
 *   [ctx_ptr + 112]  R15
 *   [ctx_ptr + 120]  RIP   (IRETQ frame)
 *   [ctx_ptr + 128]  CS    (0x08)
 *   [ctx_ptr + 136]  RFLAGS
 *   [ctx_ptr + 144]  RSP   (thread's own stack pointer)
 *   [ctx_ptr + 152]  SS    (0x10)
 * ==================================================================== */
.global invoke_preemptive_thread
invoke_preemptive_thread:
    /* 1. Save scheduler's callee-saved registers.
     *    These will be restored by preempt_return_to_scheduler
     *    when the timer fires, or by preemptive_exit_trampoline
     *    when the thread completes. */
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15

    /* 2. Save scheduler RSP to GS:[48].
     *    This is the sentinel: non-zero means "a unified fiber is running."
     *    The timer ISR checks this to decide whether to take the fast path. */
    mov  gs:[GS_SCHED_RSP], rsp

    /* 3. Switch to thread's saved stack.
     *    RDI holds ctx_ptr — the top of the thread's saved frame.
     *    CLI to prevent a timer interrupt during the stack swap. */
    cli
    mov  rsp, rdi

    /* 4. Pop all 15 software-saved GPRs */
    pop  rax
    pop  rbx
    pop  rcx
    pop  rdx
    pop  rsi
    pop  rdi
    pop  rbp
    pop  r8
    pop  r9
    pop  r10
    pop  r11
    pop  r12
    pop  r13
    pop  r14
    pop  r15

    /* 5. IRETQ atomically pops: RIP, CS, RFLAGS, RSP, SS.
     *    This resumes the thread at its saved instruction pointer.
     *    RFLAGS restoration re-enables interrupts (IF=1 from 0x202). */
    iretq


/* ====================================================================
 * preempt_return_to_scheduler
 * ====================================================================
 * Called from the modified timer_isr_wrapper when GS:[48] != 0.
 * At this point, the CPU has already pushed the 5-word IRETQ frame
 * (RIP, CS, RFLAGS, RSP, SS) onto the THREAD's stack.
 *
 * Our job:
 *   1. Save all thread GPRs on top of the hardware frame
 *   2. Record the thread's new saved RSP in GS:[56]
 *   3. Swap back to the scheduler stack
 *   4. EOI the PIC
 *   5. Return POLL_PENDING (0) to the scheduler
 * ==================================================================== */
.global preempt_return_to_scheduler
preempt_return_to_scheduler:
    /* 1. Save all 15 thread GPRs (on top of hardware IRETQ frame) */
    push r15
    push r14
    push r13
    push r12
    push r11
    push r10
    push r9
    push r8
    push rbp
    push rdi
    push rsi
    push rdx
    push rcx
    push rbx
    push rax

    /* 2. Side-channel: write thread's saved RSP to GS:[56]
     *    sched_yield() reads this to update fiber.stack_ptr */
    mov  gs:[GS_SAVED_THREAD_RSP], rsp

    /* 3. Restore scheduler RSP and clear the sentinel */
    mov  rsp, gs:[GS_SCHED_RSP]
    mov  qword ptr gs:[GS_SCHED_RSP], 0

    /* 4. Send EOI to Master PIC (same as existing ISR) */
    mov  al, 0x20
    out  0x20, al

    /* 5. Return POLL_PENDING (0) to the scheduler.
     *    Restore scheduler's callee-saved registers and return. */
    xor  eax, eax

    pop  r15
    pop  r14
    pop  r13
    pop  r12
    pop  rbp
    pop  rbx

    ret


/* ====================================================================
 * preemptive_exit_trampoline
 * ====================================================================
 * This is the hidden return address planted below the thread's entry
 * point by preemptive_stack_init. When the thread's entry function
 * executes `ret`, it lands here instead of returning to garbage.
 *
 * Our job: signal POLL_READY (1) to the scheduler.
 *
 * At this point we are still on the thread's stack. We need to
 * restore the scheduler stack and return POLL_READY.
 * ==================================================================== */
.global preemptive_exit_trampoline
preemptive_exit_trampoline:
    /* The thread has returned. We're on the thread's stack.
     * The scheduler RSP is in GS:[48]. */
    cli

    /* Restore scheduler stack and clear sentinel */
    mov  rsp, gs:[GS_SCHED_RSP]
    mov  qword ptr gs:[GS_SCHED_RSP], 0

    /* Clear saved_thread_rsp — no stack to save (thread is done) */
    mov  qword ptr gs:[GS_SAVED_THREAD_RSP], 0

    /* Return POLL_READY (1) */
    mov  eax, 1

    /* Restore scheduler's callee-saved registers */
    pop  r15
    pop  r14
    pop  r13
    pop  r12
    pop  rbp
    pop  rbx

    ret


/* ====================================================================
 * preemptive_stack_init(stack_top: u64, entry_addr: u64) -> u64
 * ====================================================================
 * Initialize a new preemptive thread's stack with a fake IRETQ frame
 * + zeroed GPRs. The layout exactly matches what invoke_preemptive_thread
 * expects.
 *
 * RDI = stack_top (highest usable address in the slab)
 * RSI = entry_addr (thread's starting function)
 * Returns: RAX = initial ctx_ptr (saved RSP for fiber.stack_ptr)
 *
 * Stack layout built (high to low):
 *   [stack_top - 8]    (trampoline return addr — hidden below IRETQ RSP)
 *   [stack_top - 16]   SS       = 0x10 (Kernel Data)
 *   [stack_top - 24]   RSP      = stack_top - 8 (thread stack; ret → trampoline)
 *   [stack_top - 32]   RFLAGS   = 0x202 (IF=1, reserved bit 1)
 *   [stack_top - 40]   CS       = 0x08 (Kernel Code 64)
 *   [stack_top - 48]   RIP      = entry_addr
 *   [stack_top - 56]   RAX      = 0
 *   [stack_top - 64]   RBX      = 0
 *   ...
 *   [stack_top - 168]  R15      = 0
 *                      ^ returned value (ctx_ptr)
 * ==================================================================== */
.global preemptive_stack_init
preemptive_stack_init:
    mov  rax, rdi
    and  rax, -16                /* 16-byte align stack_top */

    /* Plant the exit trampoline as a hidden return address.
     * When the thread's entry function does `ret`, the CPU pops
     * this address. IRETQ's RSP will point here so the thread's
     * stack frame sees preemptive_exit_trampoline as its return address. */
    sub  rax, 8
    lea  rcx, [rip + preemptive_exit_trampoline]
    mov  [rax], rcx

    /* Thread RSP for IRETQ frame: points at the trampoline address.
     * When thread entry does `ret`, it pops the trampoline. */
    mov  rdx, rax               /* rdx = thread RSP for IRETQ */

    /* Build IRETQ frame (pushed in reverse order: SS, RSP, RFLAGS, CS, RIP) */
    sub  rax, 8
    mov  qword ptr [rax], KERNEL_SS     /* SS = 0x10 */

    sub  rax, 8
    mov  [rax], rdx                      /* RSP = thread stack top (with trampoline) */

    sub  rax, 8
    mov  qword ptr [rax], 0x202          /* RFLAGS = IF=1 */

    sub  rax, 8
    mov  qword ptr [rax], KERNEL_CS     /* CS = 0x08 */

    sub  rax, 8
    mov  [rax], rsi                      /* RIP = entry_addr */

    /* 15 zeroed GPRs: RAX, RBX, RCX, RDX, RSI, RDI, RBP, R8-R15 */
    sub  rax, 120                        /* 15 * 8 = 120 bytes */
    /* Zero them all */
    mov  rcx, 15
    lea  rdi, [rax]
    xor  edx, edx
.Lzero_gprs:
    mov  [rdi], rdx
    add  rdi, 8
    dec  rcx
    jnz  .Lzero_gprs

    /* RAX = ctx_ptr (bottom of frame = initial fiber.stack_ptr) */
    ret


/* ====================================================================
 * read_u64(addr: u64) -> u64
 * ====================================================================
 * Read a u64 from an arbitrary memory address.
 * RDI = address to read
 * Returns: RAX = value at that address
 * ==================================================================== */
.global read_u64
read_u64:
    mov  rax, [rdi]
    ret


/* ====================================================================
 * user_stack_init(stack_top: u64, entry_addr: u64, user_rsp: u64) -> u64
 * ====================================================================
 * Initialize a Ring 3 thread's stack with an IRETQ frame + zeroed GPRs.
 * This is the Ring 3 counterpart of preemptive_stack_init.
 *
 * The CRITICAL difference: segment selectors use DPL 3 (Ring 3):
 *   SS = 0x23 (User Data 64 | RPL 3)
 *   CS = 0x2B (User Code 64 | RPL 3, SYSRET-compatible)
 *
 * RDI = stack_top (highest usable kernel stack address)
 * RSI = entry_addr (user-space entry point, e.g. _ring3_start)
 * RDX = user_rsp (user-space stack pointer)
 * Returns: RAX = initial ctx_ptr (saved RSP for invoke_preemptive_thread)
 *
 * Stack layout built (high to low):
 *   [top - 8]    SS       = 0x23 (User Data)
 *   [top - 16]   RSP      = user_rsp (user stack)
 *   [top - 24]   RFLAGS   = 0x202 (IF=1)
 *   [top - 32]   CS       = 0x2B (User Code 64)
 *   [top - 40]   RIP      = entry_addr
 *   [top - 48]   RAX      = 0
 *   ...
 *   [top - 160]  R15      = 0
 *                ^ returned value (ctx_ptr)
 * ==================================================================== */
.set USER_CS, 0x2B
.set USER_SS, 0x23

.global user_stack_init
user_stack_init:
    mov  rax, rdi
    and  rax, -16                /* 16-byte align stack_top */

    /* Build IRETQ frame (pushed in reverse order: SS, RSP, RFLAGS, CS, RIP) */
    sub  rax, 8
    mov  qword ptr [rax], USER_SS       /* SS = 0x23 */

    sub  rax, 8
    mov  [rax], rdx                      /* RSP = user_rsp */

    sub  rax, 8
    mov  qword ptr [rax], 0x202          /* RFLAGS = IF=1 */

    sub  rax, 8
    mov  qword ptr [rax], USER_CS       /* CS = 0x2B */

    sub  rax, 8
    mov  [rax], rsi                      /* RIP = entry_addr */

    /* 15 zeroed GPRs: RAX, RBX, RCX, RDX, RSI, RDI, RBP, R8-R15 */
    sub  rax, 120                        /* 15 * 8 = 120 bytes */
    mov  rcx, 15
    lea  rdi, [rax]
    xor  edx, edx
.Lzero_user_gprs:
    mov  [rdi], rdx
    add  rdi, 8
    dec  rcx
    jnz  .Lzero_user_gprs

    /* RAX = ctx_ptr (bottom of frame = initial fiber.stack_ptr) */
    ret

