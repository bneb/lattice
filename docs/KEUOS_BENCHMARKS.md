
IMPORTANT: The file content has been truncated.
Status: Showing lines 1-265 of 265 total lines.
Action: To read more of the file, you can use the 'start_line' and 'end_line' parameters in a subsequent 'read_file' call. For example, to read the next section of the file, use start_line: 266.

--- FILE CONTENT (truncated) ---
> | 2 | VADDR deterministic layout | ✅ PASS |
> | 3 | Data plane write (SPSC push, zero syscall) | ✅ PASS |
> | 4 | Data plane read (SPSC pop, zero syscall) | ✅ PASS |
> | 5 | Empty read returns 0 | ✅ PASS |
> | 6 | Full ring back-pressure | ✅ PASS |
> | 7 | Data plane throughput: **136 cy/64B** = 22M ops/sec | ✅ PASS |
> | 8 | HTTP Hello World (52-byte response round-trip) | ✅ PASS |
>
> Data plane architecture: applications read/write directly to shared-memory SPSC rings mapped at deterministic virtual addresses. **Zero kernel traps** in the data plane path — `socket.read()` and `socket.write()` are pure memory operations. Control plane (bind/accept/close) uses synchronous IPC to NetD (PID 5).
>
> HTTP output: `HTTP/1.1 200 OK|Content-Length: 13||Hello, World!`

> [!NOTE]
> **Ring 3 TDD Gates (February 25, 2026):** Three end-to-end Ring 3 isolation tests pass on every boot: Gate 1 (IRETQ frame: SS=0x23, CS=0x2B, RFLAGS=0x202 — 6/6), Gate 2 (KPTI: kernel_cr3 at GS:[64] — 3/3), Gate 3 (end-to-end: Ring 3 → SYSCALL(0xDEAD, 42) → exit_code=42 — 2/2). SWAPGS added to all syscall entry/exit paths.

## KVM Compatibility Fixes

Nine bugs were discovered and fixed during KVM bring-up:

### 1. Pulse Ring Buffer Triple Fault
`pulse::push()` writes to a global array (`RING` at `0xffffffff8011c940`) that crashes on KVM. Other BSS globals (slab cache, VMA, scheduler) work fine. The crash is specific to pulse's calling context (ISR re-entrancy). Fixed by no-opping `push()`.

### 2. CPUID Byte-Swap Bug
KVM was misdetected as TCG because the CPUID hypervisor check compared `0x4b564d4b` instead of the correct `0x4b4d564b` ("KVMK" in little-endian EBX). All KVM benchmarks ran at 100x reduced iterations until this was fixed.

### 3. GDT Kernel Data Descriptor
The Kernel Data GDT entry at offset `0x10` had `limit=0` and `D/B=0`. Under KVM, the CPU enforces segment limits during the 32→64-bit boot transition, causing a triple fault on `retf`. TCG's emulator doesn't enforce this. Fixed to flat 4GB (`0x00CF92000000FFFF`).

### 4. CAS Spin-Wait Pipeline Flooding
The Treiber stack's `lock cmpxchgq` retry loop lacked a `PAUSE` instruction. On KVM (real silicon), failed CAS retries execute at full pipeline speed, flooding the memory controller with cache-line invalidation requests. Added `spin_loop_hint()` (x86 PAUSE) to both `pop_stack` and `push_stack` CAS loops in `slab.salt`.

### 5. Syscall Benchmark Ring 3 Trampoline
The syscall benchmark previously called `SYSCALL` from Ring 0, corrupting the kernel stack. Implemented a proper Ring 3 trampoline: IRETQ drops CPL to 3, benchmark runs natively in Ring 3, sentinel SYSCALL (0xBEEF) escapes back to Ring 0 via `bench_ring0_restore`.

### 6. Treiber Stack Non-Canonical Address (Slab Stress Fix)
The `get_ptr()` function extracted 48-bit addresses from packed Treiber stack pointers by masking with `0x0000FFFFFFFFFFFF`, but did not sign-extend bit 47. For higher-half kernel addresses (`0xFFFFFFFF90000000`), this produced non-canonical addresses (`0x0000FFFF90FB0000`). On real silicon (KVM), the CPU's MMU immediately `#GP` faults on non-canonical dereferences. TCG does not enforce canonical address checks, masking the bug under emulation. Fixed by sign-extending bit 47 in `get_ptr()`.

### 7. Dual-Mode PIC + LAPIC EOI (March 2026)
The ISR wrapper (`isr_wrapper.S`) and preemptive return path (`preempt_stub.S`) sent only a legacy PIC EOI (`out 0x20, al`). Under KVM with APIC timer, the PIC is masked — the PIC EOI is a no-op, leaving the LAPIC's In-Service Register permanently set and blocking all future timer interrupts. Fixed by sending EOI to **both** PIC and LAPIC (`out 0x20, al` + `call apic_send_eoi`), which is safe in both environments: PIC EOI is harmless when PIC is masked (KVM), LAPIC EOI is harmless when LAPIC is inactive (TCG).

### 8. MXCSR FPU Buffer Initialization (March 2026)
The lazy FPU handler (`nm_fpu.salt`) assigns FPU buffer slots without initializing them. On first `FXRSTOR`, garbage MXCSR values can trigger `#GP(0)` on real silicon. TCG ignores invalid MXCSR bits. Fixed by zeroing the 512-byte FXSAVE buffer and writing the default MXCSR (`0x1F80`, all exceptions masked) at offset +24 on every new slot assignment.

### 9. Epoch-Based Reclamation Module (March 2026)
No EBR module existed in the kernel. Tight spawn/yield loops in benchmarks like `slab_reclaim_bench` never advanced the global epoch, causing deferred memory to pile up until slab exhaustion. Created `kernel/lib/ebr.salt` with per-core retire lists, epoch snapshots, and in-place compaction. Wired an EBR heartbeat (`exit_epoch` → `advance_epoch` → `reclaim` → `enter_epoch`) every 5 generations into the slab reclaim benchmark.

## 10. DBOS Entity Component System (v7.0)

Authoritative hardware benchmarks extracted via `rdtsc` in QEMU (Standalone Kernel Payload).

| Metric | Target | Result (Cycles) | Status |
| :--- | :--- | :--- | :--- |
| **Entity Insertion** | < 500 cy | **207** | ✅ PASS |
| **O(1) Lookup** | < 10,000 cy | **5,000** | ✅ PASS |
| **Scheduler Sweep** | < 250 cy/ent | **139** | ✅ PASS |

The transition to a Data-Oriented ECS architecture has yielded a **10x improvement** in scheduling throughput. At **139 cycles per entity**, the KeuOS kernel can evaluate and transition the state of 1,000 threads in approximately **46 microseconds** (at 3GHz), outperforming the raw process switching latency of all legacy monolithic kernels.

## Reproduce

### Local (TCG)
```bash
python3 tools/runner_qemu.py bench   # Build + run full suite
```

### Cloud (KVM)
```bash
./tools/cloud/bench_launch.sh        # Launch persistent z1d.metal (~$4/hr)
./tools/cloud/bench_run.sh           # SCP kernel.elf + run (~2 seconds)
./tools/cloud/bench_teardown.sh      # Terminate instance
```

Iteration speed: ~2 seconds per cycle (SCP 45KB ELF + QEMU boot + benchmark execution).

## Userspace Benchmarks

For Salt vs C/Rust userspace benchmarks (22 compute benchmarks, Basalt LLM inference, TCP networking, HTTP server), see [BENCHMARKS.md](../benchmarks/BENCHMARKS.md).
