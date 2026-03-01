/* kernel/arch/x86_64/proc_helpers.S
 * Assembly helpers for process management.
 */
.intel_syntax noprefix

/* idle_halt()
 * Enable interrupts and halt. Returns when an interrupt fires.
 */
.global idle_halt
idle_halt:
    sti
    hlt
    ret

/* cpu_pause()
 * Emits the x86 PAUSE instruction. Reduces power in spin-wait loops
 * and prevents memory-order violations on hyperthreaded cores.
 * On ARM, this would be YIELD.
 */
.global cpu_pause
cpu_pause:
    pause
    ret

/* broadcast_nmi_halt()
 * Sends NMI IPI to all sibling cores via Local APIC ICR.
 * This immediately halts all other cores, preventing them from
 * corrupting shared state after a kernel panic.
 *
 * APIC ICR (Interrupt Command Register):
 *   ICR High (0xFEE00310): Destination field (0 for broadcast)
 *   ICR Low  (0xFEE00300): Vector + delivery mode + destination shorthand
 *     Bits [10:8]  = 100 (NMI delivery mode)
 *     Bits [19:18] = 11  (all-excluding-self shorthand)
 *     Bit  [14]    = 1   (level = assert)
 *
 * Uses higher-half virtual addresses (kernel base + APIC MMIO).
 */
.global broadcast_nmi_halt
broadcast_nmi_halt:
    /* ICR High: destination = 0 (broadcast) */
    mov rdi, 0xFFFFFFFFFEE00310
    mov dword ptr [rdi], 0
    /* ICR Low: NMI | all-excluding-self | level assert */
    /* 0x000C4400 = shorthand:all-excl-self(11) | level:assert(1) | NMI(100) */
    mov rdi, 0xFFFFFFFFFEE00300
    mov dword ptr [rdi], 0x000C4400
    ret

/* virtio_block_dump_flight_recorder(header_ptr: u64, header_size: u64) -> bool
 * Weak stub — returns false (no block device) if not overridden by
 * the VirtIO-blk driver. In QEMU configs with virtio-blk, the strong
 * symbol from virtio.salt takes priority.
 */
.weak virtio_block_dump_flight_recorder
virtio_block_dump_flight_recorder:
    xor eax, eax
    ret

/* get_proc_trampoline_addr() -> u64
 * Returns the address of proc_trampoline (defined in proc_switch.S).
 * Salt can't take address of extern asm functions via func.constant.
 */
.global get_proc_trampoline_addr
get_proc_trampoline_addr:
    lea rax, [rip + proc_trampoline]
    ret

/* get_kernel_trampoline_addr() -> u64
 * Returns the address of kernel_trampoline (defined in proc_switch.S).
 * Used when spawning Ring 0 kernel threads (Task 0).
 */
.global get_kernel_trampoline_addr
get_kernel_trampoline_addr:
    lea rax, [rip + kernel_trampoline]
    ret

/* get_kernel_event_loop_addr() -> u64
 * Returns the address of kernel_event_loop (defined in dispatcher.salt).
 * Used to pass the entry function to spawn_kernel_thread.
 */
.global get_kernel_event_loop_addr
get_kernel_event_loop_addr:
    lea rax, [rip + kernel_event_loop]
    ret

/* get_user_elf_a_addr() -> u64 */
.global get_user_elf_a_addr
get_user_elf_a_addr:
    lea rax, [rip + user_elf_a_start]
    ret

/* get_user_elf_b_addr() -> u64 */
.global get_user_elf_b_addr
get_user_elf_b_addr:
    lea rax, [rip + user_elf_b_start]
    ret

/* get_user_elf_c_addr() -> u64 */
.global get_user_elf_c_addr
get_user_elf_c_addr:
    lea rax, [rip + user_elf_c_start]
    ret

/* get_sip_elf_addr() -> u64
 * Returns the address of the embedded Mode B SIP ELF binary.
 */
.global get_sip_elf_addr
get_sip_elf_addr:
    lea rax, [rip + sip_elf_start]
    ret

/* enable_interrupts is already defined in boot.S */
