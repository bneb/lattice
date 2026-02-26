/*
 * fiber_loop.S
 * Assembly implementation of the fiber workload for Ring of Fire benchmark.
 * Implemented in ASM to avoid complex interaction between Salt codegen 
 * (func.constant) and salt-opt lowering in this specific recursive/loop context.
 *
 * The fiber checks the global `fiber_loop_done` flag each iteration.
 * When the benchmark sets this flag to 1, the fiber calls fiber_exit()
 * to cleanly terminate and reclaim its stack slot.
 */

.global fiber_loop
.global fiber_loop_done
.extern sched_yield
.extern fiber_exit

.section .rodata
.align 16
val_init: .float 1.0
val_mul:  .float 1.000001

.section .data
.align 8
fiber_loop_done: .quad 0

.section .text
fiber_loop:
    /* Allocate stack space for saving FPU state */
    subq $16, %rsp
    
    /* Initialize XMM0 with 1.0 */
    movss val_init(%rip), %xmm0
    /* Initialize XMM1 with multiplier */
    movss val_mul(%rip), %xmm1

.Lloop:
    /* Check if the benchmark has signalled us to stop */
    movq fiber_loop_done(%rip), %rax
    testq %rax, %rax
    jnz .Lexit

    /* x = x * 1.000001 */
    mulss %xmm1, %xmm0
    
    /* Save XMM0 to stack (simulate context preservation requirement) */
    movss %xmm0, (%rsp)
    
    /* Yield control */
    call sched_yield
    
    /* Restore XMM0 from stack */
    movss (%rsp), %xmm0
    
    jmp .Lloop

.Lexit:
    /* Clean termination: restore stack and call fiber_exit */
    addq $16, %rsp
    call fiber_exit
    /* fiber_exit never returns */

/* -------------------------------------------------------------------------- */
/* Control API — called from Salt benchmarks                                  */
/* -------------------------------------------------------------------------- */

.global fiber_loop_stop
fiber_loop_stop:
    movq $1, fiber_loop_done(%rip)
    ret

.global fiber_loop_reset
fiber_loop_reset:
    movq $0, fiber_loop_done(%rip)
    ret
