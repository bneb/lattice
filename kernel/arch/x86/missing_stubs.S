// =============================================================================
// kernel/arch/x86/missing_stubs.S
// Weak fallback stubs for symbols provided by Salt source files
// =============================================================================
// These .weak stubs serve as safe fallbacks during boot if the corresponding
// Salt implementations aren't linked. When syscall.salt compiles (which it
// now does), the linker automatically picks the strong Salt definitions.
//
// REMOVED (now provided by kernel/mem/slab.salt):
//   - push_stack, pop_stack, slab_watermark
// =============================================================================

.intel_syntax noprefix

// virtio_isr_wrapper — requires assembly for iretq (always .global)
.global virtio_isr_wrapper
virtio_isr_wrapper:
    iretq

// handle_syscall(syscall_nr: u64, a1: u64, a2: u64, a3: u64) -> u64
// Weak fallback — real implementation in syscall.salt
.weak handle_syscall
handle_syscall:
    xor eax, eax
    ret

// process_exit() — Weak fallback
.weak process_exit
process_exit:
    cli
    hlt
    jmp process_exit

// timer_preempt() — Weak fallback
.weak timer_preempt
timer_preempt:
    ret

// kernel__core__syscall__set_kernel_pml4() — Weak fallback
.weak kernel__core__syscall__set_kernel_pml4
kernel__core__syscall__set_kernel_pml4:
    ret

// =============================================================================
// memset — LLVM codegen support
// =============================================================================
// LLVM optimizes large zero-init arrays ([0; N]) into memset calls.
// System V ABI: rdi=dest, esi=byte, rdx=count. Returns rdi (dest).
// =============================================================================
.global memset
memset:
    mov     r8, rdi         // save dest for return value
    movzx   eax, sil        // zero-extend byte to eax
    mov     rcx, rdx        // count -> rcx
    rep stosb               // fill [rdi] with al, rcx times
    mov     rax, r8         // return dest
    ret

// =============================================================================
// irq_save / irq_restore — RFLAGS-Preserving Interrupt Masking
// =============================================================================
// irq_save():    Saves RFLAGS, disables interrupts. Returns old RFLAGS in RAX.
// irq_restore(): Restores RFLAGS from RDI. Re-enables IF only if it was set.
//
// This is safe for nested calls: if pop_stack() is called from scheduler
// context where IF=0, irq_restore won't accidentally re-enable interrupts.
//
// Salt declaration:
//   extern fn irq_save() -> u64;
//   extern fn irq_restore(saved: u64);
// =============================================================================
.global irq_save
irq_save:
    pushfq
    pop rax
    cli
    ret

.global irq_restore
irq_restore:
    push rdi
    popfq
    ret
