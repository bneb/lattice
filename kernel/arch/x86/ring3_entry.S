// =============================================================================
// kernel/arch/x86/ring3_entry.S
// The Ring 0 -> Ring 3 Transition Stub
// =============================================================================
//
// Constructs a hardware IRETQ frame and transitions the CPU to Ring 3.
// This function never returns.
//
// Salt declaration:
//   extern fn jump_to_ring3(entry_point: u64, user_stack_top: u64);
// =============================================================================

.intel_syntax noprefix

.global jump_to_ring3
.type jump_to_ring3, @function

// System V AMD64 ABI:
//   RDI = entry_point  (becomes RIP in Ring 3)
//   RSI = user_stack_top (becomes RSP in Ring 3)
jump_to_ring3:
    // 1. Disable interrupts during the transition
    cli

    // 2. Load User Data Selector (0x23) into data segments
    mov ax, 0x23
    mov ds, ax
    mov es, ax
    mov fs, ax
    // NOTE: Do NOT load GS here — Kernel GS base (PerCpuData) must be
    // preserved. swapgs in the trampoline handles the GS swap.

    // 3. Build the IRETQ hardware frame (push in reverse order)
    push 0x23           // SS:  User Data Selector (GDT[4], RPL 3)
    push rsi            // RSP: user_stack_top
    push 0x202          // RFLAGS: IF=1, reserved bit 1
    push 0x2B           // CS:  User Code 64 Selector (GDT[5], RPL 3)
    push rdi            // RIP: entry_point

    // 4. The Hardware Switch
    iretq
