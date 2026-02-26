// =============================================================================
// kernel/arch/x86/ap_trampoline.S
// AP Boot Trampoline — 16-bit Real → 32-bit Protected → 64-bit Long Mode
// =============================================================================
//
// Copied by smp.salt::copy_trampoline() to physical address 0x8000.
// Each AP begins executing here after receiving a SIPI from the BSP.
//
// The 16-bit section is hand-encoded with .byte directives because Clang's
// integrated assembler in .code16 mode misencodes lgdt and far jumps.
//
// Mailbox offsets MUST match smp.salt MBOX_* constants.
// =============================================================================

.section .ap_trampoline, "ax"

// Shared constants — single source of truth with smp.salt
.set TRAMP_BASE,      0x8000
.set MBOX_IDTR,       0xFA0
.set MBOX_GDTR,       0xFB0
.set MBOX_LOGICAL_ID, 0xFC0
.set MBOX_STACK_TOP,  0xFC8
.set MBOX_ENTRY,      0xFD0
.set MBOX_CR3,        0xFD8

// Pre-computed absolute mailbox addresses
.set MBOX_IDTR_ABS,      TRAMP_BASE + MBOX_IDTR
.set MBOX_GDTR_ABS,      TRAMP_BASE + MBOX_GDTR
.set MBOX_CR3_ABS,       TRAMP_BASE + MBOX_CR3
.set MBOX_STACK_ABS,     TRAMP_BASE + MBOX_STACK_TOP
.set MBOX_ENTRY_ABS,     TRAMP_BASE + MBOX_ENTRY

.global _ap_trampoline_start
.global _ap_trampoline_end

// =============================================================================
// Phase 1: 16-bit Real Mode (hand-encoded .byte directives)
// =============================================================================
// AP wakes via SIPI at CS:IP = 0x0800:0x0000 = physical 0x8000.
// =============================================================================
_ap_trampoline_start:

// cli                          FA
.byte 0xFA
// cld                          FC
.byte 0xFC
// xor ax, ax                   31 C0
.byte 0x31, 0xC0
// mov ds, ax                   8E D8
.byte 0x8E, 0xD8
// mov es, ax                   8E C0
.byte 0x8E, 0xC0
// mov ss, ax                   8E D0
.byte 0x8E, 0xD0

// --- DEFINITIVE 0x7F TEST: proves AP reached 0x8000 ---
// mov dx, 0x3F8               BA F8 03
.byte 0xBA, 0xF8, 0x03
// mov al, 0x7F                B0 7F
.byte 0xB0, 0x7F
// out dx, al                  EE
.byte 0xEE

// lgdt [gdt_ptr]              66 0F 01 16 <addr16>
// 66 prefix → load 6-byte descriptor (2-byte limit + 4-byte base)
// 0F 01 /2 with ModRM=0x16 → [disp16]
.byte 0x66
.byte 0x0F, 0x01, 0x16
.word TRAMP_BASE + ap_tramp_gdt_ptr_off

// mov eax, cr0                0F 20 C0
.byte 0x0F, 0x20, 0xC0
// or al, 1                    0C 01
.byte 0x0C, 0x01
// mov cr0, eax                0F 22 C0
.byte 0x0F, 0x22, 0xC0

// ljmp 0x08:tramp32           66 EA <offset32> <selector16>
// 66 prefix → 32-bit offset in the far pointer
.byte 0x66, 0xEA
.long TRAMP_BASE + ap_tramp_32_off
.word 0x08

// =============================================================================
// Phase 2: 32-bit Protected Mode
// =============================================================================
.code32
ap_tramp_32:
    movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs

    // Serial diagnostic: write 0x01 to COM1 to prove PM transition
    movw $0x3F8, %dx
    movb $0x01, %al     // 0x01 = reached 32-bit
    outb %al, %dx

    // Enable PAE (CR4.bit5)
    movl %cr4, %eax
    orl  $(1 << 5), %eax
    movl %eax, %cr4

    // Load CR3 from mailbox — hand-encoded to avoid Clang address issues
    // mov eax, [0x8FF0]        A1 F0 8F 00 00
    .byte 0xA1
    .long MBOX_CR3_ABS
    movl %eax, %cr3

    // Enable Long Mode: EFER.LME (bit 8)
    movl $0xC0000080, %ecx
    rdmsr
    orl  $(1 << 8), %eax
    wrmsr

    // Enable Paging (CR0.bit31)
    movl %cr0, %eax
    orl  $(1 << 31), %eax
    movl %eax, %cr0

    // Far jump to 64-bit (selector 0x18)
    // Hand-encoded: EA <offset32> <selector16>
    .byte 0xEA
    .long TRAMP_BASE + ap_tramp_64_off
    .word 0x18

// =============================================================================
// Phase 3: 64-bit Long Mode
// =============================================================================
.code64
ap_tramp_64:
    // 0x02 = reached 64-bit mode
    movw $0x3F8, %dx
    movb $0x02, %al
    outb %al, %dx

    // Load stack from mailbox
    movq (MBOX_STACK_ABS), %rsp

    // 0x03 = stack loaded (non-zero check)
    movw $0x3F8, %dx
    testq %rsp, %rsp
    jz .Lbad_stack
    movb $0x03, %al
    outb %al, %dx
    jmp .Lcheck_entry
.Lbad_stack:
    movb $0xFE, %al
    outb %al, %dx
    cli
    hlt

.Lcheck_entry:
    // Load entry point from mailbox directly into r15
    movq (MBOX_ENTRY_ABS), %r15

    // 0x04 = entry loaded (non-zero check)
    movw $0x3F8, %dx
    testq %r15, %r15
    jz .Lbad_entry
    movb $0x04, %al
    outb %al, %dx
    
    // Check if it's a higher half address (top byte = 0xFF)
    movq %r15, %rbx
    shrq $56, %rbx
    cmpb $0xFF, %bl
    jne .Lbad_entry_addr
    movw $0x3F8, %dx
    movb $0x0A, %al  // 0x0A = Top byte is 0xFF (OK)
    outb %al, %dx
    
    jmp .Ldo_jump
.Lbad_entry_addr:
    movw $0x3F8, %dx
    movb $0xFC, %al  // 0xFC = Garbage pointer!
    outb %al, %dx
    cli
    hlt
.Lbad_entry:
    movb $0xFD, %al
    outb %al, %dx
    cli
    hlt

.Ldo_jump:
    // 0x05 = before lidt
    movw $0x3F8, %dx
    movb $0x05, %al
    outb %al, %dx

    lidt (MBOX_IDTR_ABS)

    // 0x06 = after lidt
    movw $0x3F8, %dx
    movb $0x06, %al
    outb %al, %dx

    // NOTE: We intentionally skip lgdt + lretq (CS reload).
    // The trampoline's CS descriptor cache (selector 0x18) has identical
    // attributes to the BSP's (selector 0x08): both are 64-bit code segments
    // with L=1, D=0. The cached descriptor is what the CPU uses, not the
    // selector value. Reloading CS via lretq crashes because the push/pop
    // addresses the virtual stack which conflicts with the GDT transition.

    // 0x07 = about to enable SSE
    movw $0x3F8, %dx
    movb $0x07, %al
    outb %al, %dx

    // --- ENABLE FPU / SSE ON THE AP ---
    movq %cr0, %rcx
    andq $~0x4, %rcx
    orq  $0x22, %rcx
    movq %rcx, %cr0
    movq %cr4, %rcx
    orq  $(3 << 9), %rcx
    movq %rcx, %cr4

    // Align stack to 16 bytes
    andq $-16, %rsp

    // 0x08 = about to jump to ap_entry
    movw $0x3F8, %dx
    movb $0x08, %al
    outb %al, %dx

    // Jump to ap_entry()
    jmpq *%r15

// =============================================================================
// Temporary GDT
// =============================================================================
.balign 8
ap_tramp_gdt:
    .quad 0x0000000000000000   // 0x00: Null
    .quad 0x00CF9A000000FFFF   // 0x08: 32-bit Code (D=1, G=1)
    .quad 0x00CF92000000FFFF   // 0x10: Data (D=1, G=1)
    .quad 0x00AF9A000000FFFF   // 0x18: 64-bit Code (L=1, D=0)

ap_tramp_gdt_ptr:
    .short ap_tramp_gdt_ptr - ap_tramp_gdt - 1  // limit (31)
    .long  TRAMP_BASE + ap_tramp_gdt_off         // base (physical)

_ap_trampoline_end:

// Label offsets
.set ap_tramp_32_off,       ap_tramp_32       - _ap_trampoline_start
.set ap_tramp_64_off,       ap_tramp_64       - _ap_trampoline_start
.set ap_tramp_gdt_off,      ap_tramp_gdt      - _ap_trampoline_start
.set ap_tramp_gdt_ptr_off,  ap_tramp_gdt_ptr  - _ap_trampoline_start
