.intel_syntax noprefix
# Multiboot1 Header with AOUT Kludge for QEMU -kernel compatibility
.set MB_MAGIC, 0x1BADB002
.set MB_FLAGS, 0x00010003  # Page align + Memory info + AOUT Kludge
.set MB_CHECKSUM, -(MB_MAGIC + MB_FLAGS)

.section .multiboot, "ax"
.align 4
multiboot_header:
    .long MB_MAGIC
    .long MB_FLAGS
    .long MB_CHECKSUM
    # AOUT Kludge fields
    # Set load_end_addr and bss_end_addr to 0 to instruct the Multiboot loader
    # to simply load the entire OS image file blindly. (Lattice zeroes its own BSS).
    .long 0x100000           # Header addr (physical)
    .long 0x100000           # Load addr (1MB)
    .long 0                    # Load end addr (0 = load whole file)
    .long 0                    # BSS end addr (0 = handled by kernel)
    .long 0x100020           # Entry addr (Right after 32-byte header)

.section .entry, "ax"
.code32
.global start
.extern kernel__core__main__kmain
.global outb
.global inb
.global outw
.global capture_state
.global pml4
.global pdpt
.global pd
.global stack_top
.global long_mode_start
.global gdt64
.global gdt64_ptr
.extern syscall_entry_fast

start:
    # ROBUST DIAGNOSTIC: 'Y' (32-bit entry)
    mov dx, 0x3F8 + 5      # LSR Register
wait_y:
    in al, dx
    test al, 0x20          # Is THR empty?
    jz wait_y
    mov dx, 0x3F8          # Data Register
    mov al, 'Y'
    out dx, al

    # 1. Setup initial physical stack
    mov eax, offset stack_top_phys
    mov esp, eax

    # ROBUST DIAGNOSTIC: '1' (Stack Set)
    mov dx, 0x3F8 + 5
wait_1:
    in al, dx
    test al, 0x20
    jz wait_1
    mov dx, 0x3F8
    mov al, '1'
    out dx, al

    # 1.5 Zero out the ENTIRE BSS Section (including Page Tables)
    .extern _bss_phys
    .extern _bss_end_phys
    mov edi, offset _bss_phys
    mov ecx, offset _bss_end_phys
    sub ecx, edi
    shr ecx, 2             # Convert byte count to dword count
    xor eax, eax
    rep stosd

    # ROBUST DIAGNOSTIC: '2' (BSS Zeroed)
    mov dx, 0x3F8 + 5
wait_2:
    in al, dx
    test al, 0x20
    jz wait_2
    mov dx, 0x3F8
    mov al, '2'
    out dx, al

    # 2. Setup Page Tables
    # Link PML4[0] (identity) and PML4[511] (higher half) -> PDPT
    mov eax, offset pdpt_phys
    or eax, 0x7            # Present + Writable + User
    mov edi, offset pml4_phys
    mov [edi], eax
    mov [edi + 511 * 8], eax

    # Link PDPT[0] (identity) and PDPT[510..511] (higher half -2GB) -> PD
    # We allocate 2 Page Directories (8192 bytes) to map 2GB total.
    mov eax, offset pd_phys
    or eax, 0x7
    mov edi, offset pdpt_phys
    mov [edi], eax             # PDPT[0] (identity map)
    mov [edi + 510 * 8], eax   # PDPT[510] (higher half, first 1GB)
    
    # Second PD for the next 1GB
    add eax, 4096
    mov [edi + 511 * 8], eax   # PDPT[511] (higher half, second 1GB)

    # Map PD[0] - PD[1023] to 0-2GB (1024 Huge Pages)
    mov edi, offset pd_phys
    mov eax, 0x87          # Start at physical 0 + Present+RW+User+PageSize
    mov ecx, 1024          # Map 1024 Entries (2GB)
map_pd_loop:
    mov [edi], eax
    add eax, 0x200000      # Increment physical address by 2MB
    add edi, 8             # Next entry
    loop map_pd_loop

    # Map PDPT[1]-PDPT[3] as 1GB huge pages for 1GB-4GB (identity map)
    # Required for APIC MMIO at physical 0xFEE00000 (~4GB)
    # Uses PS bit (bit 7) at PDPT level for 1GB pages
    mov edi, offset pdpt_phys
    mov eax, 0x40000087     # 1GB + Present+RW+User+PS
    mov [edi + 1 * 8], eax
    mov dword ptr [edi + 1 * 8 + 4], 0
    mov eax, 0x80000087     # 2GB
    mov [edi + 2 * 8], eax
    mov dword ptr [edi + 2 * 8 + 4], 0
    mov eax, 0xC0000087     # 3GB — covers 0xFEE00000 (APIC MMIO)
    mov [edi + 3 * 8], eax
    mov dword ptr [edi + 3 * 8 + 4], 0

    # ROBUST DIAGNOSTIC: 'Z' (Paging Configured)
    mov dx, 0x3F8 + 5
wait_z:
    in al, dx
    test al, 0x20
    jz wait_z
    mov dx, 0x3F8
    mov al, 'Z'
    out dx, al

    # 3. Enable Long Mode
    # Set CR4.PAE
    # Set CR4.PAE (5) and CR4.OSFXSR (9) for SSE/FXSAVE
    mov eax, cr4
    or eax, (1 << 5) | (1 << 9)
    mov cr4, eax

    # Set EFER.LME (Long Mode Enable)
    mov ecx, 0xC0000080
    rdmsr
    or eax, 1 << 8
    wrmsr

    # Load CR3 with PML4
    mov eax, offset pml4_phys
    mov cr3, eax

    # Enable Paging (CR0.PG) and FPU (MP=1, EM=0, NE=1)
    mov eax, cr0
    or eax, 1 << 31        # PG
    or eax, 1 << 1         # MP (Monitor Coprocessor)
    or eax, 1 << 5         # NE (Numeric Error)
    and eax, ~(1 << 2)     # Clear EM (Emulation)
    mov cr0, eax

    # ROBUST DIAGNOSTIC: '7' (Paging Enabled)
    mov dx, 0x3F8 + 5
wait_7:
    in al, dx
    test al, 0x20
    jz wait_7
    mov dx, 0x3F8
    mov al, '7'
    out dx, al

    # Set CR4.OSXMMEXCPT (10) for unmasked SIMD exceptions
    # (We already set PAE and OSFXSR earlier)
    mov eax, cr4
    or eax, 1 << 10
    mov cr4, eax

    # 4. Global Descriptor Table (64-bit)
    lgdt [gdt64_ptr_phys]

    # Initialize segments for 64-bit mode (using 32-bit compatibility)
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov ss, ax

    # ROBUST DIAGNOSTIC: '8' (GDT Loaded)
    mov dx, 0x3F8 + 5
wait_8:
    in al, dx
    test al, 0x20
    jz wait_8
    mov dx, 0x3F8
    mov al, '8'
    out dx, al

    # 5. Far Return to 64-bit Long Mode
    push 0x8                                     # Selector
    mov eax, offset long_mode_start_phys
    push eax                                     # Offset

    # ROBUST DIAGNOSTIC: '9' (Far Jump Ready)
    mov dx, 0x3F8 + 5
wait_9:
    in al, dx
    test al, 0x20
    jz wait_9
    mov dx, 0x3F8
    mov al, '9'
    out dx, al

    retf

.code64
long_mode_start:
    # 0. ROBUST '!' Output (Confirm we are in 64-bit)
    mov dx, 0x3F8 + 5
wait_bang:
    in al, dx
    test al, 0x20
    jz wait_bang
    mov dx, 0x3F8
    mov al, '!'
    out dx, al

    # ROBUST DIAGNOSTIC: 'X' (64-bit entry)
    mov dx, 0x3F8 + 5
wait_x_diag:
    in al, dx
    test al, 0x20
    jz wait_x_diag
    mov dx, 0x3F8
    mov al, 'X'
    out dx, al
    mov al, 10             # Newline
    out dx, al

    # 6. Re-base Stack to Higher-Half Virtual Address
    mov rsp, offset stack_top

    # 6.5 Configure SYSCALL/SYSRET MSRs
    # Enable SCE (System Call Enable) in EFER
    mov ecx, 0xC0000080         # IA32_EFER
    rdmsr
    or eax, 1                   # SCE = bit 0
    wrmsr

    # STAR: Kernel CS=0x08 in bits [47:32], User CS32=0x18 in bits [63:48]
    # SYSRET math: CS = STAR[63:48]+16 = 0x18+16 = 0x28 (UserCS64)
    #              SS = STAR[63:48]+8  = 0x18+8  = 0x20 (UserDS)
    mov ecx, 0xC0000081         # IA32_STAR
    mov edx, 0x00180008         # (UserCS32 << 16) | KernelCS
    xor eax, eax                # Lower 32 bits unused in Long Mode
    wrmsr

    # LSTAR: 64-bit entry point for SYSCALL
    # NOTE: Must use movabs (64-bit immediate), NOT lea [rip+...].
    # At this point RIP is at ~0x100xxx (identity-mapped), but
    # syscall_entry_fast is at 0xFFFFFFFF801xxxxx. The displacement
    # exceeds the ±2GB limit of a 32-bit RIP-relative lea.
    mov ecx, 0xC0000082         # IA32_LSTAR
    movabs rax, offset syscall_entry_fast
    mov rdx, rax
    shr rdx, 32                 # High 32 bits in EDX
    wrmsr                       # Low 32 bits in EAX

    # FMASK: Mask IF on SYSCALL entry (disable interrupts)
    mov ecx, 0xC0000084         # IA32_FMASK
    mov eax, 0x200              # IF = bit 9
    xor edx, edx
    wrmsr

    # 6.6 Configure TSS for Ring 3 → Ring 0 interrupt delivery
    # SYSRET drops to Ring 3. When PIT fires, CPU needs TSS.RSP0
    # to find the kernel stack for the privilege-level change.

    # Fill TSS.RSP0 (offset 4) with kernel stack pointer
    lea rax, [rip + tss64]
    mov qword ptr [rax + 4], offset stack_top    # RSP0

    # Build 16-byte TSS Descriptor in GDT at offset 0x30
    # Format: see Intel SDM Vol 3A, Table 3-2 (System-Segment Descriptor)
    # Limit = sizeof(TSS) - 1 = 103 = 0x67
    # Type = 0x9 (64-bit TSS Available), DPL=0, Present=1
    # Access byte = 0x89 (P=1, DPL=0, Type=1001)
    lea rdi, [rip + tss64]          # TSS base address
    mov rsi, rdi                    # Save a copy

    # Low quad: [limit_15:0 | base_15:0 | base_23:16 | access | limit_19:16+flags | base_31:24]
    mov rax, 0x67                   # Limit bits [15:0]
    mov rcx, rsi
    and ecx, 0xFFFF                 # Base bits [15:0]
    shl rcx, 16
    or rax, rcx

    mov rcx, rsi
    shr rcx, 16
    and ecx, 0xFF                   # Base bits [23:16]
    shl rcx, 32
    or rax, rcx

    mov rcx, 0x89                   # Access: Present, DPL=0, 64-bit TSS Available
    shl rcx, 40
    or rax, rcx

    # Limit bits [19:16] = 0, Flags (G=0, AVL=0) = 0x00
    # Base bits [31:24]
    mov rcx, rsi
    shr rcx, 24
    and ecx, 0xFF
    shl rcx, 56
    or rax, rcx

    lea rdi, [rip + gdt64_tss_lo]
    mov [rdi], rax                  # Write low quad

    # High quad: [base_63:32 | reserved]
    mov rax, rsi
    shr rax, 32                     # Base bits [63:32]
    lea rdi, [rip + gdt64_tss_hi]
    mov [rdi], rax                  # Write high quad

    # Reload GDT with new size (now includes TSS descriptor)
    lea rax, [rip + gdt64]
    sub rsp, 16
    mov word ptr [rsp], 0x3F        # New limit: 8 entries * 8 bytes - 1 = 63
    mov [rsp + 2], rax              # GDT base (virtual)
    lgdt [rsp]
    add rsp, 16

    # Load TSS
    mov ax, 0x30                    # TSS descriptor offset in GDT
    ltr ax

    # 6.7 Initialize SYSCALL kernel RSP scratch variable
    # (Removed: now handled per-CPU via GS segment)

    # 7. Call Salt kmain
    # Magic in EAX (MB1: 0x2BADB002), Info ptr in EBX (preserved)
    mov rdi, rax
    mov rsi, rbx
    call kmain

hang:
    hlt
    jmp hang

# I/O Port Stubs
# outb(rdi: port, rsi: val)
outb:
    mov dx, di
    mov al, sil
    out dx, al
    ret

# inb(rdi: port) -> rax: val
inb:
    mov dx, di
    in al, dx
    movzx rax, al
    ret

# outw(rdi: port, rsi: val)
outw:
    mov dx, di
    mov ax, si
    out dx, ax
    ret

# dump_registers stubs (placeholder for MVP)
capture_state:
    ret

.global sys_send
.global sys_recv

# sys_send(payload: Region, target_id: u32)
sys_send:
    ret

# sys_recv() -> Region
sys_recv:
    # Return (offset potato_buffer, 4096)
    # ABI: RAX = ptr, RDX = size (for struct { u64, u64 })
    mov rax, offset potato_buffer
    mov rdx, 4096
    ret

.global sys_yield
sys_yield:
    jmp sched_yield

.global kernel__core__io__outb
kernel__core__io__outb:
    jmp outb

.global kernel__core__io__inb
kernel__core__io__inb:
    jmp inb

.global kernel__core__io__outw
kernel__core__io__outw:
    jmp outw

.global lidt_wrapper
lidt_wrapper:
    # RDI = limit (u16 embedded in u64), RSI = base (u64)
    push rbp
    mov rbp, rsp
    sub rsp, 16         # Reserve space
    mov [rsp], di       # Store Limit (2 bytes) at offset 0
    mov [rsp+2], rsi    # Store Base (8 bytes) at offset 2
    lidt [rsp]          # Load from [rsp] which is now Limit+Base packed
    leave
    ret

.global lgdt_wrapper
lgdt_wrapper:
    # RDI = limit (u16 embedded in u64), RSI = base (u64)
    push rbp
    mov rbp, rsp
    sub rsp, 16
    mov [rsp], di
    mov [rsp+2], rsi
    lgdt [rsp]
    leave
    ret

.global ltr_wrapper
ltr_wrapper:
    # RDI = selector (u16 embedded in u64)
    mov ax, di
    ltr ax
    ret

.global get_cycles_asm
get_cycles_asm:
    rdtsc
    shl rdx, 32
    or rax, rdx
    ret

.global enable_interrupts
enable_interrupts:
    sti
    ret

.global disable_interrupts
disable_interrupts:
    cli
    ret

.global kernel_halt
kernel_halt:
    cli
.Lhalt_loop:
    hlt
    jmp .Lhalt_loop

.section .data
.align 8
.global gdt64
gdt64:
    .quad 0x0000000000000000 # 0x00: Null Descriptor
    .quad 0x00209a0000000000 # 0x08: Kernel Code 64 (DPL 0)
    .quad 0x00CF92000000FFFF # 0x10: Kernel Data    (DPL 0) — flat 4GB, G=1 D/B=1
    .quad 0x0020FA0000000000 # 0x18: User Code 64   (DPL 3) — CS for IRETQ (L=1)
    .quad 0x0000F20000000000 # 0x20: User Data      (DPL 3) — SS for SYSRET
    .quad 0x0020FA0000000000 # 0x28: User Code 64   (DPL 3) — CS for SYSRET (L=1)
gdt64_tss_lo:
    .quad 0x0000000000000000 # 0x30: TSS Descriptor low  (filled at runtime)
gdt64_tss_hi:
    .quad 0x0000000000000000 # 0x38: TSS Descriptor high (filled at runtime)
.global gdt64_ptr
gdt64_ptr:
    .short . - gdt64 - 1
    # GDT base MUST be 32-bit physical address during 32-bit boot
    .long gdt64 - 0xFFFFFFFF80000000

.section .bss

# ---- Kernel Stack (FIRST in BSS so overflow goes into guard, not page tables) ----
# Stack grows DOWNWARD: stack_top is the initial RSP, stack_bottom is the limit.
# 64KB for Ring 3 GDT/TSS/scheduler init which copies large structs onto stack.
.align 4096
stack_guard:
    .skip 4096           # 4KB guard zone — overflow hits here, not page tables
stack_bottom:
    .skip 65536          # 64KB kernel stack
stack_top:

# ---- Gap to prevent stack_top aliasing with pml4 ----
.align 4096
stack_pml4_guard:
    .skip 4096

# ---- Page Tables (AFTER stack — safe from downward stack growth) ----
.align 4096
.global pml4
pml4:
    .skip 4096
.global pdpt
pdpt:
    .skip 4096
.global pd
pd:
    .skip 8192

# ---- Misc ----
.align 4096
potato_buffer:
    .skip 4096

# Task State Segment — 104 bytes, zeroed by BSS
.align 16
.global tss64
tss64:
    .skip 104
