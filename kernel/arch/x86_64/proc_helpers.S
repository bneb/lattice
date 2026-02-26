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
