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

// virtio_isr_wrapper — real ISR that calls virtio_net_isr() and sends EOI
// Handles SWAPGS for Ring 3 interrupts. No fiber preemption needed.
.section .kpti_trampoline_text, "ax"
.global virtio_isr_wrapper
.extern apic_send_eoi
virtio_isr_wrapper:
    // SWAPGS check: CS is at [rsp+8]. If CPL=3, swap GS.
    test qword ptr [rsp + 8], 3
    jz .L_virtio_from_kernel
    swapgs
    /* KPTI CR3 Switch to Kernel PML4 */
    push rax
    mov rax, cr3
    mov gs:[128], rax
    mov rax, gs:[64]
    /* bts rax, 63 removed for QEMU TCG compatibility */
    mov cr3, rax
    pop rax
.L_virtio_from_kernel:

    // Save caller-saved registers
    push rax
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11

    // Call the Salt-compiled ISR handler (drains RX, reclaims TX, wakes MoE)
    call virtio_net_isr

    // Send LAPIC EOI
    call apic_send_eoi

    // Restore registers
    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rax

    // SWAPGS return check
    test qword ptr [rsp + 8], 3
    jz .L_virtio_return_to_kernel
    /* KPTI CR3 Switch to User PML4 */
    push rax
    mov rax, gs:[128]
    /* bts rax, 63 removed for QEMU TCG compatibility */
    mov cr3, rax
    pop rax
    swapgs
.L_virtio_return_to_kernel:
    iretq

// ps2_isr_wrapper — ISR for PS/2 Keyboard RX
.global ps2_isr_wrapper
.extern ps2_kbd_irq_handler

ps2_isr_wrapper:
    test qword ptr [rsp + 8], 3
    jz .L_ps2_from_kernel
    swapgs
    /* KPTI CR3 Switch to Kernel PML4 */
    push rax
    mov rax, cr3
    mov gs:[128], rax
    mov rax, gs:[64]
    /* bts rax, 63 removed for QEMU TCG compatibility */
    mov cr3, rax
    pop rax
.L_ps2_from_kernel:
    push rax
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11

    call ps2_kbd_irq_handler

    // The Salt implementation of ps2_kbd_irq_handler handles PIC EOI: outb(0x20, 0x20)
    // We also send LAPIC EOI just in case APIC is routing it
    call apic_send_eoi

    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rax

    test qword ptr [rsp + 8], 3
    jz .L_ps2_return_to_kernel
    /* KPTI CR3 Switch to User PML4 */
    push rax
    mov rax, gs:[128]
    /* bts rax, 63 removed for QEMU TCG compatibility */
    mov cr3, rax
    pop rax
    swapgs
.L_ps2_return_to_kernel:
    iretq

// serial_isr_wrapper — ISR for COM1 Serial RX
.global serial_isr_wrapper
.extern serial_irq_handler

serial_isr_wrapper:
    test qword ptr [rsp + 8], 3
    jz .L_serial_from_kernel
    swapgs
    /* KPTI CR3 Switch to Kernel PML4 */
    push rax
    mov rax, cr3
    mov gs:[128], rax
    mov rax, gs:[64]
    /* bts rax, 63 removed for QEMU TCG compatibility */
    mov cr3, rax
    pop rax
.L_serial_from_kernel:
    push rax
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11

    call serial_irq_handler

    // Send LAPIC EOI
    call apic_send_eoi

    // Also send PIC EOI since it's IRQ4
    mov al, 0x20
    out 0x20, al

    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rax

    test qword ptr [rsp + 8], 3
    jz .L_serial_return_to_kernel
    /* KPTI CR3 Switch to User PML4 */
    push rax
    mov rax, gs:[128]
    /* bts rax, 63 removed for QEMU TCG compatibility */
    mov cr3, rax
    pop rax
    swapgs
.L_serial_return_to_kernel:
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

// =============================================================================
// __salt_contract_violation — Z3 requires()/ensures() Runtime Trap
// =============================================================================
// Called when a requires() or ensures() contract fails at runtime.
// In production, this should panic with a diagnostic. For the QEMU
// benchmark harness, we halt — any contract violation is a hard error.
// =============================================================================
.weak __salt_contract_violation
__salt_contract_violation:
    cli
    hlt
    jmp __salt_contract_violation

// =============================================================================
// disable_nic_rings — KeuOS Reclaim NIC Stub
// =============================================================================
// Called by keuos_reclaim.salt Phase 2. The Salt compiler DCE's the
// implementation if VirtIO is not linked, so we provide an empty stub here
// to satisfy the linker for bare-metal/non-virtio test targets.
.weak kernel__core__keuos_reclaim__disable_nic_rings
kernel__core__keuos_reclaim__disable_nic_rings:
    ret

// =============================================================================
// Teardown / IRQ Stubs — WIP until proper implementations are extracted
// =============================================================================

// unmap_flat_bitmap — WIP stub for teardown.salt Phase 3
.weak unmap_flat_bitmap
unmap_flat_bitmap:
    ret

// disable_dma_queues — WIP stub for teardown.salt Phase 2
.weak disable_dma_queues
disable_dma_queues:
    ret

// tlb_flush_all — WIP stub for teardown.salt Phase 3
.weak tlb_flush_all
tlb_flush_all:
    ret

// lapic_eoi — WIP stub for irq.salt (LAPIC EOI)
// Real implementation in kernel/arch/x86/apic.salt
.weak lapic_eoi
lapic_eoi:
    ret

// =============================================================================
// Syscall Aliases for C-ABI Internal Kernel Linkage
// =============================================================================
// sys_write: now @no_mangle in kernel/core/syscall_io.salt
// sys_spawn: now @no_mangle in kernel/core/syscall_sched.salt
// sys_wait: now @no_mangle in kernel/core/syscall_sched.salt
// sys_brk: now @no_mangle in kernel/core/syscall_sched.salt
// sys_mmap: now @no_mangle in kernel/core/syscall_sched.salt

// Dummy traps for syscalls not implemented in C for kernel threads
.global sys_exit
sys_exit:
    ud2

// sys_brk: now @no_mangle in kernel/core/syscall_sched.salt
// sys_mmap: now @no_mangle in kernel/core/syscall_sched.salt

.global sys_ipc_get_msg0
sys_ipc_get_msg0:
    ud2

.global sys_ipc_get_msg1
sys_ipc_get_msg1:
    ud2

.global sys_ipc_get_msg2
sys_ipc_get_msg2:
    ud2

// sys_moe_init_trampoline — defined in user/reactor/tasks/moe_pipeline.salt
// Weak stub for kernel-side linkage during boot sequence
.global sys_moe_init_trampoline
.weak sys_moe_init_trampoline
sys_moe_init_trampoline:
    ret

.global sys_ipc_reg_send
sys_ipc_reg_send:
    ud2

.global sys_ipc_ring_await
sys_ipc_ring_await:
    ud2

.global sys_ipc_recv_full
sys_ipc_recv_full:
    ud2

// =============================================================================
// Weak stubs for functions originally planned for kernel/mem/slab.salt (WiP)
// =============================================================================
// slab.salt is uncommitted WiP with cmpxchg16b i128 support not yet available
// in salt-opt. These weak stubs keep the kernel buildable until slab is ready.

.global __salt_overflow_panic
.weak __salt_overflow_panic
__salt_overflow_panic:
    ud2

.global pop_stack
.weak pop_stack
pop_stack:
    xor eax, eax
    ret

.global push_stack
.weak push_stack
push_stack:
    ret

.global slab_watermark
.weak slab_watermark
slab_watermark:
    xor eax, eax
    ret

.global init_cores
.weak init_cores
init_cores:
    ret
