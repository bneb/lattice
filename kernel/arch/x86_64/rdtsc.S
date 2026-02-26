/* src/kernel/arch/x86_64/rdtsc.S */
.intel_syntax noprefix
.global rdtsc

/*
 * fn rdtsc() -> u64
 * Returns the Time Stamp Counter (TSC).
 */
rdtsc:
    rdtsc               /* EDX:EAX = TSC */
    shl rdx, 32
    or rax, rdx         /* RAX = (RDX << 32) | EAX */
    ret
