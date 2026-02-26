.intel_syntax noprefix
// =============================================================================
// kernel/arch/x86/smp_helpers.S
// SMP Assembly Helpers — MSR, MMIO, Atomics, Per-CPU, PIC, Delays
// =============================================================================
// Assembly primitives required by the SMP subsystem:
//   - smp.salt:     delay_cycles
//   - apic.salt:    rdmsr, wrmsr, mmio_read32, mmio_write32, disable_pic
//   - smp_test.salt: read_gs_u64
//   - percpu.salt:  write_gs_base
//   - slab.salt:    atomic_fetch_add_i32, atomic_load_i32
// =============================================================================

// =============================================================================
// delay_cycles(rdi: count)
// Busy-wait loop with PAUSE hint — used for INIT-SIPI-SIPI timing.
// =============================================================================
.global delay_cycles
delay_cycles:
    test rdi, rdi
    jz .Ldelay_done
.Ldelay_loop:
    pause
    dec rdi
    jnz .Ldelay_loop
.Ldelay_done:
    ret

// =============================================================================
// MSR Access: rdmsr / wrmsr
// =============================================================================
// rdmsr(ecx: msr_index) -> rax: value
// Reads the 64-bit MSR into EDX:EAX, returns combined in RAX.
.global rdmsr
rdmsr:
    mov ecx, edi            // MSR index from first arg
    rdmsr                   // EDX:EAX = MSR[ECX]
    shl rdx, 32
    or  rax, rdx            // Combine into 64-bit RAX
    ret

// wrmsr(edi: msr_index, rsi: value)
// Writes 64-bit value to MSR. Splits into EDX:EAX.
.global wrmsr
wrmsr:
    mov ecx, edi            // MSR index
    mov eax, esi            // Low 32 bits
    mov rdx, rsi
    shr rdx, 32             // High 32 bits
    wrmsr
    ret

// =============================================================================
// MMIO Access: mmio_read32 / mmio_write32
// =============================================================================
// Used by apic.salt for Local APIC register access.

// mmio_read32(rdi: addr) -> eax: value
.global mmio_read32
mmio_read32:
    mov eax, dword ptr [rdi]
    ret

// mmio_write32(rdi: addr, esi: val)
.global mmio_write32
mmio_write32:
    mov dword ptr [rdi], esi
    ret

// =============================================================================
// GS Segment: write_gs_base / read_gs_u64
// =============================================================================
// Per-CPU data access. GS.base MSR = 0xC0000101.

// write_gs_base(rdi: base_addr)
// Sets the GS.base MSR for per-CPU data access (used by percpu.init_ap).
.global write_gs_base
write_gs_base:
    mov ecx, 0xC0000101     // IA32_GS_BASE
    mov eax, edi            // Low 32 bits
    mov rdx, rdi
    shr rdx, 32             // High 32 bits
    wrmsr
    ret

// read_gs_base() -> rax: base_addr
// Reads the GS.base MSR.
.global read_gs_base
read_gs_base:
    mov ecx, 0xC0000101     // IA32_GS_BASE
    rdmsr                   // EDX:EAX = MSR
    shl rdx, 32
    or rax, rdx
    ret

// read_gs_u64(rdi: offset) -> rax: value
// Reads a u64 from GS segment at the given offset.
.global read_gs_u64
read_gs_u64:
    mov rax, qword ptr gs:[rdi]
    ret

// =============================================================================
// Atomics: atomic_fetch_add_i32 / atomic_load_i32
// =============================================================================

// atomic_fetch_add_i32(rdi: ptr, esi: val) -> eax: old_value
.global atomic_fetch_add_i32
atomic_fetch_add_i32:
    lock xadd dword ptr [rdi], esi
    mov eax, esi            // xadd puts old value in src operand
    ret

// atomic_load_i32(rdi: ptr) -> eax: value
.global atomic_load_i32
atomic_load_i32:
    mov eax, dword ptr [rdi]
    ret

// atomic_store_i32(rdi: ptr, esi: val)
// Sequential-consistency store via xchg (implicit LOCK prefix).
.global atomic_store_i32
atomic_store_i32:
    xchg dword ptr [rdi], esi
    ret

// =============================================================================
// atomic_cas_u64(rdi: ptr, rsi: expected, rdx: desired) -> rax: old_value
// =============================================================================
// 64-bit Compare-And-Swap. Atomically:
//   if (*ptr == expected) { *ptr = desired; return expected; }
//   else { return *ptr; }
//
// Used by PMM cross-core page stealing to safely mutate a remote core's
// free list head without CLI (which only protects the local core).
// =============================================================================
.global atomic_cas_u64
atomic_cas_u64:
    mov     rax, rsi            // RAX = expected (cmpxchg compares RAX)
    lock cmpxchg qword ptr [rdi], rdx   // if [rdi]==RAX then [rdi]=RDX
    ret                         // RAX = old value (== expected on success)

// =============================================================================
// disable_pic — Mask all legacy 8259 PIC interrupts
// =============================================================================
// Write 0xFF to both PIC data ports to mask all IRQs.
// Called by apic.salt::init() before enabling the LAPIC.
.global disable_pic
disable_pic:
    mov al, 0xFF
    out 0x21, al            // PIC1 data port (IRQ 0-7)
    out 0xA1, al            // PIC2 data port (IRQ 8-15)
    ret

// =============================================================================
// memcpy — LLVM codegen support
// =============================================================================
// LLVM emits memcpy calls for struct copies and array init.
// System V ABI: rdi=dest, rsi=src, rdx=count. Returns dest in rax.
.global memcpy
memcpy:
    mov rax, rdi            // save dest for return value
    mov rcx, rdx            // count -> rcx
    rep movsb               // copy [rsi] -> [rdi], rcx bytes
    ret

// =============================================================================
// scheduler_spawn — Weak fallback (real impl in scheduler.salt)
// =============================================================================
.weak scheduler_spawn
scheduler_spawn:
    xor eax, eax
    ret

// scheduler_yield_now — Weak fallback (real impl in scheduler.salt)
.weak scheduler_yield_now
scheduler_yield_now:
    ret

// =============================================================================
// ap_entry_asm — Pure-assembly AP entry point (bypasses Salt/LLVM codegen)
// =============================================================================
// Emits 0x7E, push rbp, emits 0x7D. Tests stack on AP.
.global ap_entry_asm
ap_entry_asm:
    mov dx, 0x3F8
    mov al, 0x7E
    out dx, al

    push rbp
    mov rbp, rsp

    mov dx, 0x3F8
    mov al, 0x7D
    out dx, al
1:
    hlt
    jmp 1b

// =============================================================================
// sidt_store(rdi: dest_addr) — Save current IDTR to memory
// =============================================================================
// Stores 10 bytes (2-byte limit + 8-byte base) at [rdi].
.global sidt_store
sidt_store:
    sidt [rdi]
    ret

// =============================================================================
// sgdt_store(rdi: dest_addr) — Save current GDTR to memory
// =============================================================================
.global sgdt_store
sgdt_store:
    sgdt [rdi]
    ret

// =============================================================================
// trigger_double_fault() — Force a #DF for diagnostic testing
// =============================================================================
// Zeros the stack pointer and triggers an exception to exhaust the stack.
// This proves that IST 2 (#DF) works correctly even when RSP is corrupted.
.global trigger_double_fault
trigger_double_fault:
    xor rsp, rsp
    // trigger #BP by int 3; push will #PF because rsp=0; #PF push will #DF
    int 3
    ret
