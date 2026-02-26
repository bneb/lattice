// =============================================================================
// kernel/arch/x86/embedded_user.S
// Embedded User ELF Binaries -- Three Distinct Ring 3 Programs
// =============================================================================
//
// Embeds three user-space ELF binaries into the kernel's .rodata section.
// Process A, B, and C have different payloads to demonstrate interleaving.
//
// In kmain():
//   &user_elf_a_start = Process A ELF bytes (test_memory)
//   &user_elf_b_start = Process B ELF bytes (ring3_test_b)
//   &user_elf_c_start = Process C ELF bytes (hello)
// =============================================================================

.section .rodata

// --- Process A: Memory Proof of Life Test ---
.global user_elf_a_start
.global user_elf_a_end

user_elf_a_start:
    .incbin "/tmp/test_memory"
user_elf_a_end:

// --- Process B ---
.global user_elf_b_start
.global user_elf_b_end

user_elf_b_start:
    .incbin "/tmp/ring3_test_b"
user_elf_b_end:

// --- Process C: Structured Lifecycle Verification ---
.global user_elf_c_start
.global user_elf_c_end

user_elf_c_start:
    .incbin "/tmp/hello"
user_elf_c_end:

// --- Mode B SIP: Ring 0 Software-Isolated Process ---
.global sip_elf_start
.global sip_elf_end

sip_elf_start:
    .incbin "/tmp/sip_app"
sip_elf_end:

