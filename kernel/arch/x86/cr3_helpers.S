// =============================================================================
// kernel/arch/x86/cr3_helpers.S
// CR3 (Page Table Base) Read/Write Helpers
// =============================================================================
//
// Salt declarations:
//   extern fn read_cr3() -> u64;
//   extern fn switch_cr3(pml4_phys: u64);
// =============================================================================

.intel_syntax noprefix

.global read_cr3
.type read_cr3, @function

.global switch_cr3
.type switch_cr3, @function

read_cr3:
    mov rax, cr3
    ret

// RDI = new PML4 physical address (System V ABI)
switch_cr3:
    mov cr3, rdi
    ret

.global invlpg_stub
.type invlpg_stub, @function
invlpg_stub:
    invlpg [rdi]
    ret

// =============================================================================
// PCID (Process Context Identifiers) Support
// =============================================================================
// CR4.PCIDE (bit 17) enables PCID.
// CR3 bit 63 = NOFLUSH: skip TLB flush on CR3 write.
// CR3 bits [11:0] = PCID (12-bit process identifier).
// =============================================================================

.global read_cr4
.type read_cr4, @function
read_cr4:
    mov rax, cr4
    ret

.global write_cr4
.type write_cr4, @function
write_cr4:
    mov cr4, rdi
    ret

// enable_pcid — Set CR4.PCIDE (bit 17)
// PRECONDITION: CR3 PCID field must be 0 when first enabling PCIDE.
// Intel SDM Vol 3A, §4.10.1: #GP if CR3[11:0] != 0 when setting PCIDE.
.global enable_pcid
.type enable_pcid, @function
enable_pcid:
    mov rax, cr4
    or  rax, 0x20000       // Bit 17 = PCIDE
    mov cr4, rax
    ret

// switch_cr3_pcid — Write CR3 with PCID + optional NOFLUSH
// RDI = PML4 physical address | PCID [11:0] | NOFLUSH [63]
// When bit 63 is set, the CPU does NOT flush TLB entries tagged
// with the new PCID — only entries with a different PCID are
// naturally evicted. This is the context-switch fast path.
.global switch_cr3_pcid
.type switch_cr3_pcid, @function
switch_cr3_pcid:
    mov cr3, rdi
    ret
