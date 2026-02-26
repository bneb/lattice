/* user/user_linker.ld
 * Linker script for Ring 3 user-space ELF binaries.
 *
 * Places the program at virtual address 0x400000 (4MB), well within
 * the user-half of the address space. exec_user.salt's ELF loader
 * reads PT_LOAD segments and maps them with user page tables.
 *
 * IMPORTANT: This address must be ABOVE the identity-map cutoff
 * (boot.S only identity-maps 0..1GB via 2MB huge pages at PML4[0]).
 * The ELF loader creates fresh 4KB PTEs for these addresses.
 */
ENTRY(main)

SECTIONS
{
    . = 0x400000;

    .text : {
        *(.text)
        *(.text.*)
    }

    . = ALIGN(4096);
    .rodata : {
        *(.rodata)
        *(.rodata.*)
    }

    . = ALIGN(4096);
    .data : {
        *(.data)
        *(.data.*)
        *(.got)
    }

    . = ALIGN(4096);
    .bss : {
        *(.bss)
        *(COMMON)
    }

    _end = .;
}
