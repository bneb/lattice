/* ============================================================================
 * user/kernel/linker.ld
 * Physical Memory Map — Freestanding Exokernel Layout
 * ============================================================================
 *
 * Target: x86_64, loaded at 1MB by Multiboot2-compliant bootloader.
 *
 * Memory Map:
 *   0x00100000 (1M)    .text         Code (multiboot_header first!)
 *   aligned             .rodata       Read-only data
 *   aligned             .data         Initialized data
 *   aligned             .bss          Zero-initialized data
 *   aligned (4MB)       LOC_ARENA     64MB kernel arena
 *   aligned              STACK_TOP    Kernel stack (64KB)
 *
 * The .multiboot_header MUST be within the first 32KB of the binary.
 * We KEEP() it to prevent dead-code elimination by the linker.
 *
 * ============================================================================ */

ENTRY(_start)

SECTIONS {
    /* ================================================================
     * Bootloaders load the kernel at 1MB (0x100000)
     * ================================================================ */
    . = 1M;

    /* ================================================================
     * .text — Executable code
     * The multiboot_header MUST be first for GRUB/QEMU recognition
     * ================================================================ */
    .text : ALIGN(4K) {
        KEEP(*(.multiboot_header))
        *(.text .text.*)
    }

    /* ================================================================
     * .rodata — Read-only data (const tables, string literals)
     * ================================================================ */
    .rodata : ALIGN(4K) {
        *(.rodata .rodata.*)
    }

    /* ================================================================
     * .data — Initialized mutable data
     * ================================================================ */
    .data : ALIGN(4K) {
        *(.data .data.*)
    }

    /* ================================================================
     * .bss — Zero-initialized data + kernel resources
     * ================================================================ */
    .bss : ALIGN(4K) {
        *(COMMON)
        *(.bss .bss.*)

        /* ============================================================
         * Kernel Stack — 64KB, grows downward
         * ============================================================ */
        . = ALIGN(4K);
        KERNEL_STACK_BOTTOM = .;
        . += 64K;
        KERNEL_STACK_TOP = .;

        /* ============================================================
         * LOC_ARENA — 64MB, the keuos memory substrate
         *
         * All zero-allocation data structures live here:
         *   - HardwareManifest (32 bytes)
         *   - WorkDeque array (one per core)
         *   - ExpertRoute routing table
         *   - MoE task states
         *   - Tensor buffers
         *   - Statistical sample arrays
         *
         * Aligned to 4MB for potential huge-page mapping.
         * ============================================================ */
        . = ALIGN(4M);
        LOC_ARENA_START = .;
        . += 64M;
        LOC_ARENA_END = .;
    }

    /* Export key symbols for runtime queries */
    _kernel_end = .;
    _arena_size = LOC_ARENA_END - LOC_ARENA_START;

    /* Discard debug and comment sections in release builds */
    /DISCARD/ : {
        *(.comment)
        *(.note.GNU-stack)
    }
}
