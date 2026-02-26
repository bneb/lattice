/* kernel/arch/x86_64/async_call.S
 *
 * invoke_task(fn_addr: u64, task_frame: u64) -> i64
 *
 * Universal Task Invocation Primitive. Calls a function pointer with one
 * argument and returns the result. This is the kernel's equivalent of
 * `call *%rax` — a 3-instruction primitive that enables zero-branch
 * dispatch for both async state machines and (eventually) preemptive
 * thread trampolines.
 *
 * System V AMD64 ABI:
 *   rdi = fn_addr (step function address)
 *   rsi = task_frame (opaque pointer, becomes arg0 of step_fn)
 *   rax = return value (Poll status: 0=Pending, 1=Ready)
 */
.intel_syntax noprefix

.global invoke_task
invoke_task:
    mov  rax, rdi       /* rax = fn_addr                    */
    mov  rdi, rsi       /* rdi = task_frame (step_fn arg0)  */
    call rax            /* call *rax → step_fn(task_frame)  */
    ret                 /* rax = step_fn return value        */
