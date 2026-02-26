// =============================================================================
// kernel/arch/x86/io_stubs.S
// I/O Port Access Stubs (outl, inl, inw, outb, inb)
// =============================================================================
// These are required by the VirtIO PCI driver for port-mapped I/O.
// =============================================================================

.intel_syntax noprefix

.global outl
.global inl
.global inw

// outl(port: u16, value: u32)
// RDI = port, RSI = value
outl:
    mov edx, edi
    mov eax, esi
    out dx, eax
    ret

// inl(port: u16) -> u32
// RDI = port, returns in EAX
inl:
    mov edx, edi
    in eax, dx
    ret

// inw(port: u16) -> u16
// RDI = port, returns in AX
inw:
    mov edx, edi
    in ax, dx
    movzx eax, ax
    ret

// =============================================================================
// TLB Management
// =============================================================================

.global invlpg_addr

// invlpg_addr(vaddr: u64)
// RDI = virtual address to invalidate
invlpg_addr:
    invlpg [rdi]
    ret
