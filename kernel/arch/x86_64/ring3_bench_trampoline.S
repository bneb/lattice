/* kernel/arch/x86_64/ring3_bench_trampoline.S
 * Ring 3 Benchmark Trampoline — Surgical CPL Drop for Syscall Measurement
 *
 * This module provides the hardware mechanism to:
 *   1. DROP from Ring 0 to Ring 3 via IRETQ (the only safe privilege de-escalation)
 *   2. EXECUTE the benchmark loop natively in Ring 3
 *   3. ESCAPE back to Ring 0 via a sentinel SYSCALL that restores the test runner
 *
 * The x86-64 IRETQ instruction pops 5 values from the stack in hardware-defined order:
 *   [RSP+0]  → RIP    (benchmark function entry point)
 *   [RSP+8]  → CS     (User Code selector with RPL=3)
 *   [RSP+16] → RFLAGS (interrupts enabled)
 *   [RSP+24] → RSP    (Ring 3 stack pointer)
 *   [RSP+32] → SS     (User Data selector with RPL=3)
 *
 * GDT Selectors (verified against boot.S gdt64):
 *   0x1B = GDT[3] (0x18) | RPL 3 = User Code 64 (L=1, DPL=3)
 *   0x23 = GDT[4] (0x20) | RPL 3 = User Data    (DPL=3)
 *
 * Salt declaration:
 *   extern fn drop_to_ring3_and_bench(ring3_func: u64, ring3_stack: u64);
 */
.intel_syntax noprefix

/* ====================================================================
 * SENTINEL — Special syscall number to exit the benchmark
 * Must not collide with any real Linux syscall number.
 * ==================================================================== */
.set BENCH_EXIT_SENTINEL, 0xBEEF

/* ====================================================================
 * drop_to_ring3_and_bench — Ring 0 → Ring 3 Trampoline
 * ====================================================================
 * RDI = target RIP (Ring 3 benchmark function)
 * RSI = target RSP (Ring 3 stack top)
 *
 * This function appears to "return normally" after the benchmark
 * completes, thanks to the escape hatch restoring our saved state.
 * ==================================================================== */
.global drop_to_ring3_and_bench
drop_to_ring3_and_bench:
    /* 1. Preserve the Ring 0 test runner's callee-saved state */
    push rbp
    push rbx
    push r12
    push r13
    push r14
    push r15

    /* 2. Save the exact Ring 0 stack pointer for the escape hatch */
    mov [rip + bench_ring0_rsp_save], rsp

    /* 3. Update kernel_rsp0_scratch so the SYSCALL handler in the
     *    benchmark loop has a valid kernel stack that won't clobber
     *    our saved registers (they're below this RSP). */
    mov gs:[96], rsp

    /* 4. Build the IRETQ hardware frame (pushed in reverse order) */
    push 0x23               /* SS  = User Data (GDT[4]) | RPL 3 */
    push rsi                /* RSP = ring3_stack (passed in RSI) */
    push 0x2                /* RFLAGS = reserved only; IF=0 (no PIT interrupts in Ring 3) */
    push 0x2B               /* CS  = User Code 64 (GDT[5]) | RPL 3 */
    push rdi                /* RIP = ring3_func (passed in RDI) */

    /* Diagnostic: mark that trampoline reached IRETQ (1 = Ring 0 reached here) */
    mov qword ptr [rip + bench_diag_phase], 1

    /* SWAPGS: Kernel GS → User GS before entering Ring 3 */
    swapgs

    /* 5. Execute the privilege drop. CPU is now in Ring 3. */
    iretq

/* ====================================================================
 * ring3_bench_loop_asm — Full Ring 3 Benchmark Loop (pure assembly)
 * ====================================================================
 * The Salt-compiled ring3_bench_loop crashes in its prologue due to
 * PIC relocation issues when executing at Ring 3. This assembly version
 * runs the full benchmark: warmup + measurement + escape.
 *
 * Globals written (all assembly-owned, read via extern fn from Salt):
 *   bench_diag_phase:     1=entered, 2=warmup done, 3=measured
 *   bench_asm_total:      sum of valid cycle measurements
 *   bench_asm_valid:      number of valid samples
 *   bench_asm_min:        minimum valid cycle count
 *   bench_asm_max:        maximum valid cycle count
 *   bench_asm_loops:      total loop iterations executed
 *   bench_asm_first_t0:   first rdtsc reading (diagnostic)
 *   bench_asm_first_elapsed: first elapsed value (diagnostic)
 * ==================================================================== */
.set WARMUP_COUNT, 10
.set SAMPLE_COUNT, 100
.set MIN_VALID, 10
.set MAX_VALID, 500000

.section .user_text, "ax"

.global ring3_bench_loop_asm
ring3_bench_loop_asm:
    /* Mark: entered Ring 3 */
    mov qword ptr [rip + bench_diag_phase], 1

    /* ── Warmup: prime SYSCALL/SYSRET pipeline ── */
    mov ecx, WARMUP_COUNT
.Lwarmup:
    push rcx
    /* inline syscall_noop: save RCX/R11 around SYSCALL */
    push rcx
    push r11
    xor eax, eax
    syscall
    pop r11
    pop rcx
    pop rcx
    dec ecx
    jnz .Lwarmup

    mov qword ptr [rip + bench_diag_phase], 2

    /* ── Measurement: rdtsc → SYSCALL(0) → SYSRET → rdtsc ── */
    /* Use callee-saved registers to survive SYSCALL clobber:
     *   R12 = loop counter
     *   R13 = valid sample count
     *   R14 = total cycles
     *   R15 = min cycles
     *   RBX = max cycles (also first_t0 temporarily)
     */
    xor r12d, r12d          /* loop counter = 0 */
    xor r13d, r13d          /* valid = 0 */
    xor r14d, r14d          /* total = 0 */
    mov r15, 0xFFFFFFFFFFFFFFFF  /* min = MAX */
    xor ebx, ebx            /* max = 0 */

.Lmeasure_loop:
    cmp r12d, SAMPLE_COUNT
    jge .Lmeasure_done

    /* rdtsc → RBP (t0) — we save it in RBP */
    rdtsc
    shl rdx, 32
    or rax, rdx
    push rbp
    mov rbp, rax            /* t0 in RBP */

    /* Save first t0 for diagnostics */
    test r12d, r12d
    jnz .Lskip_first_t0
    mov [rip + bench_asm_first_t0], rax
.Lskip_first_t0:

    /* syscall_noop (inline) */
    push rcx
    push r11
    xor eax, eax
    syscall
    pop r11
    pop rcx

    /* rdtsc → RAX (t1) */
    rdtsc
    shl rdx, 32
    or rax, rdx

    /* elapsed = t1 - t0 */
    sub rax, rbp
    pop rbp                 /* restore RBP */

    /* Save first elapsed for diagnostics */
    test r12d, r12d
    jnz .Lskip_first_elapsed
    mov [rip + bench_asm_first_elapsed], rax
.Lskip_first_elapsed:

    /* Increment loop counter */
    inc r12d
    mov qword ptr [rip + bench_asm_loops], r12

    /* Filter: MIN_VALID <= elapsed <= MAX_VALID */
    cmp rax, MIN_VALID
    jb .Lmeasure_loop
    cmp rax, MAX_VALID
    ja .Lmeasure_loop

    /* Valid sample — accumulate */
    add r14, rax            /* total += elapsed */
    inc r13d                /* valid++ */
    cmp rax, r15
    cmovb r15, rax          /* min = min(min, elapsed) */
    cmp rax, rbx
    cmova rbx, rax          /* max = max(max, elapsed) */
    jmp .Lmeasure_loop

.Lmeasure_done:
    /* Store results to assembly-owned globals */
    mov [rip + bench_asm_total], r14
    mov [rip + bench_asm_valid], r13
    mov [rip + bench_asm_min], r15
    mov [rip + bench_asm_max], rbx
    mov qword ptr [rip + bench_diag_phase], 3

    /* Escape to Ring 0 */
    mov eax, BENCH_EXIT_SENTINEL
    syscall
    ud2

/* ====================================================================
 * bench_ring0_restore — Ring 0 Escape Hatch
 * ====================================================================
 * Jumped to from syscall_entry_fast when it detects the BENCH_EXIT_SENTINEL.
 * Restores the Ring 0 test runner's execution state and returns to the
 * call site of drop_to_ring3_and_bench as if it were a normal function call.
 * ==================================================================== */
.section .text, "ax"

.global bench_ring0_restore
bench_ring0_restore:
    /* Restore the Ring 0 test runner's stack pointer.
     * This discards any SYSCALL handler stack state — we don't need it. */
    mov rsp, [rip + bench_ring0_rsp_save]

    /* Restore callee-saved registers (reverse of push order) */
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp

    /* Return to the Lattice benchmark suite as if nothing happened */
    ret

/* ====================================================================
 * syscall_noop — Ring 3 Null Syscall Stub
 * ====================================================================
 * Executes SYSCALL with RAX=0 (null syscall). The hardware:
 *   1. Saves RIP → RCX, RFLAGS → R11
 *   2. Loads RIP from MSR_LSTAR (→ syscall_entry_fast)
 *   3. Clears RFLAGS per MSR_FMASK
 *   4. syscall_entry_fast detects RAX=0 → fast return via SYSRET
 *   5. SYSRET restores RCX → RIP, R11 → RFLAGS
 *
 * Total round-trip: the quantity we are benchmarking.
 *
 * Salt declaration:
 *   extern fn syscall_noop();
 * ==================================================================== */
.section .user_text, "ax"

.global syscall_noop
syscall_noop:
    push rcx                 /* Save caller's RCX (SYSCALL clobbers: RCX ← return RIP) */
    push r11                 /* Save caller's R11 (SYSCALL clobbers: R11 ← RFLAGS) */
    xor eax, eax             /* RAX = 0 (null syscall) */
    syscall                  /* Ring 3 → Ring 0 → Ring 3 */
    pop r11                  /* Restore caller's R11 */
    pop rcx                  /* Restore caller's RCX */
    ret

/* ====================================================================
 * syscall_escalate_ring0 — Benchmark Exit via Sentinel SYSCALL
 * ====================================================================
 * Invokes SYSCALL with the sentinel value (0xBEEF). The handler
 * detects this and jumps to bench_ring0_restore instead of SYSRET.
 * This function NEVER RETURNS to the Ring 3 caller.
 *
 * Salt declaration:
 *   extern fn syscall_escalate_ring0();
 * ==================================================================== */
.section .user_text, "ax"

.global syscall_escalate_ring0
syscall_escalate_ring0:
    mov eax, BENCH_EXIT_SENTINEL
    syscall                  /* → syscall_entry_fast → bench_ring0_restore */
    /* UNREACHABLE: bench_ring0_restore returns directly to Ring 0 test runner */
    ud2

/* ====================================================================
 * spin_loop_hint — x86 PAUSE for CAS Spin-Wait Loops
 * ====================================================================
 * Prevents pipeline flooding in lock cmpxchg retry loops. On Skylake,
 * PAUSE delays ~140 cycles, giving the cache coherence protocol time
 * to settle before the next CAS attempt.
 *
 * Salt declaration:
 *   extern fn spin_loop_hint();
 * ==================================================================== */
.section .text, "ax"

.global spin_loop_hint
spin_loop_hint:
    pause
    ret

/* ====================================================================
 * Scratch state for the Ring 0 ↔ Ring 3 handoff
 * ==================================================================== */
.section .user_data, "aw"
.global bench_ring0_rsp_save
bench_ring0_rsp_save:
    .quad 0

.global bench_diag_phase
bench_diag_phase:
    .quad 0

.global bench_asm_total
bench_asm_total:
    .quad 0

.global bench_asm_valid
bench_asm_valid:
    .quad 0

.global bench_asm_min
bench_asm_min:
    .quad 0xFFFFFFFFFFFFFFFF

.global bench_asm_max
bench_asm_max:
    .quad 0

.global bench_asm_loops
bench_asm_loops:
    .quad 0

.global bench_asm_first_t0
bench_asm_first_t0:
    .quad 0

.global bench_asm_first_elapsed
bench_asm_first_elapsed:
    .quad 0

/* ====================================================================
 * Accessor functions for Salt (extern fn)
 * ==================================================================== */
.text

.global read_bench_diag_phase
read_bench_diag_phase:
    mov rax, [rip + bench_diag_phase]
    ret

.global read_bench_asm_total
read_bench_asm_total:
    mov rax, [rip + bench_asm_total]
    ret

.global read_bench_asm_valid
read_bench_asm_valid:
    mov rax, [rip + bench_asm_valid]
    ret

.global read_bench_asm_min
read_bench_asm_min:
    mov rax, [rip + bench_asm_min]
    ret

.global read_bench_asm_max
read_bench_asm_max:
    mov rax, [rip + bench_asm_max]
    ret

.global read_bench_asm_loops
read_bench_asm_loops:
    mov rax, [rip + bench_asm_loops]
    ret

.global read_bench_asm_first_t0
read_bench_asm_first_t0:
    mov rax, [rip + bench_asm_first_t0]
    ret

.global read_bench_asm_first_elapsed
read_bench_asm_first_elapsed:
    mov rax, [rip + bench_asm_first_elapsed]
    ret

/* ====================================================================
 * ring3_test_exit_payload — Ring 3 Test Payload for TDD Gate 3
 * ====================================================================
 * Minimal Ring 3 function that invokes the test-exit sentinel.
 *   RAX = 0xDEAD (test exit sentinel)
 *   RDI = 42     (exit code)
 *
 * syscall_entry_fast detects 0xDEAD, stores RDI in ring3_test_exit_code,
 * and jumps to bench_ring0_restore to return to the Ring 0 test runner.
 *
 * Salt declaration:
 *   extern fn ring3_test_exit_payload();
 * ==================================================================== */
.section .user_text, "ax"

.global ring3_test_exit_payload
ring3_test_exit_payload:
    mov rax, 0xDEAD             /* Test exit sentinel */
    mov rdi, 42                 /* Exit code = 42 */
    syscall                     /* Ring 3 → syscall_entry_fast → bench_ring0_restore */
    /* UNREACHABLE */
    ud2

/* ====================================================================
 * read_ring3_test_exit_code — Read the captured exit code
 * ====================================================================
 * Returns the value stored in ring3_test_exit_code by the test sentinel
 * handler in syscall_entry_fast.
 *
 * Salt declaration:
 *   extern fn read_ring3_test_exit_code() -> u64;
 * ==================================================================== */
.section .text, "ax"

.global read_ring3_test_exit_code
read_ring3_test_exit_code:
    mov rax, gs:[120]
    ret

/* ====================================================================
 * ring3_swapgs_test_payload — Asynchronous NMI SWAPGS Test
 * ====================================================================
 * Spins while waiting for the APIC Timer to deliver an NMI.
 * If the Kernel NMI handler executes SWAPGS properly and IRETQs back,
 * we eventually escape here and return exit code 43.
 * ==================================================================== */
.section .user_text, "ax"

.global ring3_swapgs_test_payload
ring3_swapgs_test_payload:
    mov rcx, 10000000           /* Spin for a few million cycles */
.Lswapgs_spin:
    pause
    dec rcx
    jnz .Lswapgs_spin

    mov rax, 0xDEAD             /* Test exit sentinel */
    mov rdi, 43                 /* Exit code = 43 (SWAPGS PASS) */
    syscall                     /* Escape to Ring 0 */
    ud2

