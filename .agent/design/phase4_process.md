# Phase 4: Process Management & Preemption

## Objective
Transition KeuOS from a single-tasking OS that executes a single `/hello.elf` program and halts, into a fully multitasking OS capable of preemptively scheduling multiple Ring 3 processes, spawning new programs dynamically via syscalls, and providing a shell (`grit`) as the initial process (PID 1).

## Component 1: `sys_spawn`
Implement `sys_spawn(path_ptr: u64, path_len: u64) -> u64`
1. Resolves `path_ptr` (from user space) using `is_valid_user_ptr`.
2. Looks up the path in the VFS (`sys_vfs_lookup`).
3. Allocates a new Process slot.
4. Creates a new PML4 and maps the ELF segments (`exec_spawn_process_from_inode`).
5. Sets the new process state to `PROC_READY` and increments the active process count.
6. Returns the allocated `PID` (slot index) to the caller.

## Component 2: `sys_wait`
Implement `sys_wait(pid: u64) -> u64`
1. The calling process blocks if the target `pid` is `PROC_RUNNING` or `PROC_READY`.
2. To block, the caller's state is set to `PROC_WAITING` and `proc_context_switch` is invoked to switch to the next ready process.
3. Once the target `pid` exits, it becomes `PROC_ZOMBIE`.
4. `sys_wait` reaps the zombie process (freeing its metadata and slot) and returns the exit code to the caller.

## Component 3: Preemptive Process Scheduling
Currently, the APIC Timer ISR triggers `scheduler_tick` which handles Ring 0 Fibers. Ring 3 processes are entirely excluded from preemptive scheduling.
We will hook the APIC Timer ISR (or create a new `sys_yield` path) to multiplex Ring 3 processes:
1. When the APIC Timer fires in Ring 3 (CS = 0x2B), the hardware saves User RIP, CS, RFLAGS, RSP, SS onto the Kernel Stack.
2. The `syscall_entry_fast.S` or `isr_wrapper.S` will save the GPRs.
3. We will modify `process.salt` to export a `process_tick(saved_rsp: u64) -> u64` function.
4. If there are multiple `PROC_READY` processes, `process_tick` will round-robin schedule them, returning the `new_rsp` for the ISR to `iretq` into.

## Component 4: The `grit` Shell
Update `kernel/core/main.salt` to load `/grit.elf` instead of `/hello.elf`.
`grit` will run as the system's `init` process (PID 1). The shell will utilize `sys_read` (VirtIO keyboard input), `sys_write` (VirtIO screen output), and `sys_spawn` to launch other test programs like `/hello.elf`.
