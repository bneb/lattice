/* kernel/arch/x86/linker.ld */
ENTRY(start)

SECTIONS
{
    /* 1. Identity Mapped Boot Section (Physical = Virtual) */
    . = 0x00100000;
    _phys_base = .;

    .boot :
    {
        *(.multiboot)
        *(.entry)
    }

    /* 2. Higher Half Kernel Section (-2GB) */
    . = 0xFFFFFFFF80101000;
    _virt_base = .;

    .text : AT(ADDR(.text) - 0xFFFFFFFF80000000)
    {
        _code = .;
        *(.text)
        . = ALIGN(4096);
    }

    /* KPTI TRAMPOLINE TEXT - Surgical Isolation */
    . = ALIGN(4096);
    kpti_trampoline_text_start = .;
    .kpti_trampoline_text : AT(ADDR(.kpti_trampoline_text) - 0xFFFFFFFF80000000)
    {
        *(.kpti_trampoline_text)
        . = ALIGN(4096);
    }
    kpti_trampoline_text_end = .;

    /* KPTI TRAMPOLINE DATA - Currently empty, reserved for static trampoline data */
    . = ALIGN(4096);
    kpti_trampoline_data_start = .;
    .kpti_trampoline_data : AT(ADDR(.kpti_trampoline_data) - 0xFFFFFFFF80000000)
    {
        *(.kpti_trampoline_data)
        . = ALIGN(4096);
    }
    kpti_trampoline_data_end = .;

    /* KPTI RING 3 TESTING SECTIONS - Explicitly mapped to User PML4 */
    . = ALIGN(4096);
    user_text_start = .;
    .user_text : AT(ADDR(.user_text) - 0xFFFFFFFF80000000)
    {
        *(.user_text)
        . = ALIGN(4096);
    }
    user_text_end = .;

    . = ALIGN(4096);
    user_data_start = .;
    .user_data : AT(ADDR(.user_data) - 0xFFFFFFFF80000000)
    {
        *(.user_data)
        . = ALIGN(4096);
    }
    user_data_end = .;

    .rodata : AT(ADDR(.rodata) - 0xFFFFFFFF80000000)
    {
        _rodata = .;
        *(.rodata)
        . = ALIGN(4096);
    }

    .data : AT(ADDR(.data) - 0xFFFFFFFF80000000)
    {
        _data = .;
        *(.data)
        *(.got)
        . = ALIGN(4096);
        _data_end = .;
    }

    /* AP Boot Trampoline — source bytes, copied to 0x8000 at runtime.
     * MUST be before .bss — BSS has FileSiz=0 in ELF, so any section
     * after it won't be loaded by QEMU's multiboot loader. */
    .ap_trampoline : AT(ADDR(.ap_trampoline) - 0xFFFFFFFF80000000)
    {
        _ap_trampoline_start = .;
        *(.ap_trampoline)
        _ap_trampoline_end = .;
        . = ALIGN(4096);
    }

    .bss : AT(ADDR(.bss) - 0xFFFFFFFF80000000)
    {
        _bss = .;
        *(.bss)
        *(COMMON)
        . = ALIGN(4096);
        _bss_end = .;
    }

    _end = .;

    /* Physical address aliases for boot.S (using direct subtraction of the higher-half offset) */
    pml4_phys = pml4 - 0xFFFFFFFF80000000;
    pdpt_phys = pdpt - 0xFFFFFFFF80000000;
    pd_phys   = pd - 0xFFFFFFFF80000000;
    stack_top_phys = stack_top - 0xFFFFFFFF80000000;
    long_mode_start_phys = long_mode_start;
    gdt64_ptr_phys = gdt64_ptr - 0xFFFFFFFF80000000;
    
    /* Multiboot AOUT Kludge dynamically-sized physical end addresses */
    _data_end_phys = _data_end - 0xFFFFFFFF80000000;
    _bss_phys = _bss - 0xFFFFFFFF80000000;
    _bss_end_phys = _bss_end - 0xFFFFFFFF80000000;
}
