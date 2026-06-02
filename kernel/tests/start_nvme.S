; Multiboot1 Header with AOUT Kludge for QEMU -kernel compatibility
MB_MAGIC  equ 0x1BADB002
MB_FLAGS  equ 0x00010003  ; Page align + Memory info + AOUT Kludge
MB_CHECKSUM equ -(MB_MAGIC + MB_FLAGS)

section .multiboot_header
align 4
multiboot_header:
    dd MB_MAGIC
    dd MB_FLAGS
    dd MB_CHECKSUM
    dd 0x100000           ; Header addr (physical)
    dd 0x100000           ; Load addr (1MB)
    dd 0                  ; Load end addr (0 = load whole file)
    dd 0                  ; BSS end addr (0 = handled by kernel)
    dd 0x100020           ; Entry addr (Right after 32-byte header)

section .text
align 4

global _start
extern kernel__tests__nvme_test__kmain

[BITS 32]
_start:
    ; We are in 32-bit protected mode.
    cli
    ; Set up initial stack
    mov esp, stack_top

    ; 1. Set up page tables (identity map first 2MB)
    mov eax, pml4_table
    mov cr3, eax
    
    ; 2. Enable PAE (Physical Address Extension)
    mov eax, cr4
    or eax, 1 << 5      ; PAE bit
    mov cr4, eax

    ; 3. Set LME (Long Mode Enable) in EFER MSR
    mov ecx, 0xC0000080 ; EFER MSR
    rdmsr
    or eax, 1 << 8      ; LME bit
    wrmsr

    ; 4. Enable Paging
    mov eax, cr0
    or eax, 1 << 31     ; PG bit
    mov cr0, eax

    ; 5. Load 64-bit GDT
    lgdt [gdt64_ptr]

    ; 6. Jump to 64-bit code segment
    jmp 0x08:long_mode_start

align 8
[BITS 64]
long_mode_start:
    ; Update segment registers
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    ; Call our Salt kernel code!
    call kernel__tests__nvme_test__kmain

halt_loop:
    hlt
    jmp halt_loop

global outb
global inb

outb:
    mov dx, di
    mov al, sil
    out dx, al
    ret

inb:
    mov dx, di
    in al, dx
    movzx rax, al
    ret

section .data
align 4096
pml4_table:
    dd pdpt_table + 3, 0      ; Present + Write
    times 511 dq 0
pdpt_table:
    ; Map first 4GB using four 1GB huge pages
    dd 0x00000083, 0          ; 0 - 1GB
    dd 0x40000083, 0          ; 1GB - 2GB
    dd 0x80000083, 0          ; 2GB - 3GB
    dd 0xC0000083, 0          ; 3GB - 4GB
    times 508 dq 0

gdt64:
    dq 0 ; Null
    dq 0x0020980000000000 ; Code
    dq 0x0000920000000000 ; Data
gdt64_ptr:
    dw $ - gdt64 - 1
    dd gdt64

section .bss
align 16
    resb 8192
stack_top:
