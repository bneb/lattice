# Lattice OS Kernel Benchmarks

Self-hosted benchmarks measuring Lattice unikernel primitives on real x86 hardware. Salt compiles directly to kernel code via MLIR → LLVM IR → ELF. No OS abstraction layer, no libc, no runtime.

## Platforms

| Platform | Hardware | Hypervisor | Purpose |
|:---------|:---------|:-----------|:--------|
| **KVM** | AWS z1d.metal, Intel Xeon 8151 (Skylake, 4.0 GHz) | QEMU 8.2 + KVM (`-cpu host`) | Authoritative cycle counts |
| **TCG** | Apple M4, QEMU x86_64 software emulation | None (interpreted) | Development iteration |

## KVM Results (February 24, 2026)

> [!IMPORTANT]
> KVM runs kernel instructions on real x86 silicon via hardware-assisted virtualization. These cycle counts reflect actual CPU pipeline behavior, cache effects, and branch prediction.

| Benchmark | Avg (cycles) | Min | Max | Samples | What It Measures |
|:----------|------------:|---------:|---------:|--------:|:-----------------|
| **Arena alloc** | **60** | 58 | 180 | 1,000 | Bump-allocator slot throughput |
| **PMM alloc/free** | **78** | 70 | 410 | 500 | Physical page alloc + free pair |
| **Null syscall** | **102** | 100 | 118 | 100 | SYSCALL/SYSRET round-trip (Ring 3 → 0 → 3) |
| **Slab pop/push** | **103** | 98 | 310 | 200 | Treiber stack CAS pop + push pair |
| **UTP invoke_task** | **29** | 28 | 84 | 1,000 | Direct async dispatch primitive (3 instructions) |
| **UTP async yield** | **111** | 100 | 578 | 100 | Full sched_yield round-trip for async fiber |
| **UTP spawn (async)** | **99** | 92 | 386 | 100 | Spawn async fiber (bitmap + slab + frame init) |
| **UTP spawn (preempt)** | **116** | 108 | 494 | 100 | Spawn preemptive fiber (+ IRETQ frame setup) |
| **UTP preempt dispatch** | **430** | 420 | 984 | 100 | Full IRETQ chain: save/restore + ring transition |
| **IPC ping-pong** | **284** | 218 | 750 | 10 | Fiber-to-fiber yield round-trip (4 fibers) |
| **Ctx switch (4 fibers)** | **494** | — | — | 1,000 | Context switch latency (full FXSAVE, 3-tier: 4/16/64) |
| **SMP AP boot** | — | — | — | 1 | AP count verification (ACPI → APIC → INIT-SIPI-SIPI) |
| **Per-core PMM** | ≈BSP | — | — | 1,000 | Per-core page alloc/free (zero CAS contention) |
| **Per-core Slab** | ≈BSP | — | — | 1,000 | Per-core slab pop/push (zero CAS contention) |
| **IRQ latency** | **33.9M** | 33.1M | 34.0M | 10 | PIT interrupt delivery latency |

### Industry Comparison

All comparison numbers are from published benchmarks on x86-64 Skylake-class hardware at comparable clock speeds. Cycle counts at 4.0 GHz where only nanosecond figures were available.

| Operation | Lattice | Linux 6.x | seL4 | Notes |
|:----------|--------:|----------:|-----:|:------|
| **Null syscall** | **102 cy** | ~760 cy | — | Linux `getpid()` on Skylake-X: 191ns. KPTI + Spectre mitigations add ~100ns. |
| **Slab alloc+free** | **103 cy** | ~1,200 cy | — | Linux glibc `malloc`/`free` pair: ~300ns. `tcmalloc` hot path: ~200cy. |
| **UTP invoke_task** | **29 cy** | — | — | Bare function-pointer dispatch. Tokio task poll: ~50-100cy. Go goroutine resume: ~100-300cy. |
| **UTP async yield** | **111 cy** | — | — | Full cooperative scheduling round-trip including bitmap scan and cleanup. |
| **UTP spawn** | **99 cy** | — | — | Async fiber creation. Go `go func(){}`: ~300-500cy. Linux `clone()`: ~10,000+cy. |
| **UTP preempt dispatch** | **430 cy** | ~5,200 cy | ~600 cy | Full IRETQ chain with GPR save/restore. Linux context switch: ~1.3µs. seL4 IPC: <1,000cy. |
| **Context switch** | **494 cy** | ~5,200 cy | ~600 cy | Linux pipe-based: ~1.3µs with CPU pinning. seL4 full IPC: <1,000cy on x86-64. |
| **IPC round-trip** | **284 cy** | ~12,000 cy | ~400 cy | Linux pipe IPC: ~3µs. seL4 `ReplyRecv`: <1,000cy. Lattice: shared address space, no TLB flush. |
| **Bump alloc** | **60 cy** | ~80 cy | — | `tcmalloc` small-object fast path: ~20-30ns. Lattice is a raw pointer bump. |

> [!NOTE]
> **These comparisons are as fair as possible at the moment.** The Linux numbers are from published benchmarks on comparable Skylake-class hardware, not the same z1d.metal instance. A true apples-to-apples comparison would run `lmbench` on the same box. Some caveats:
>
> - **Unikernel vs general-purpose OS**: Lattice is a single-address-space unikernel. Linux pays for process isolation (separate page tables, TLB flushes, KPTI). The speedups measure the *cost of that isolation*, not that Lattice is a "better kernel."
> - **No security mitigations**: Lattice has no KPTI, no Spectre/Meltdown mitigations. A pre-KPTI Linux kernel would be closer to ~400cy for `getpid`, cutting the gap roughly in half.
> - **Single-core benchmarks, multi-core boot**: Lattice now boots all APs via INIT-SIPI-SIPI (SMP bring-up verified). Current benchmarks run on the BSP only. Multi-core parallel benchmarks require AP scheduler integration (Phase 3).
> - **Microbenchmarks, not workloads**: These measure minimum primitive cost with warm caches. Real workloads add cache pollution, working set pressure, and contention that change the picture.
>
> That said, the architectural advantages are real: no TLB flush on IPC (shared address space), lock-free CAS allocation (no arena locks), O(1) bitmap scheduler (no red-black tree walk). These are legitimate properties of the unikernel model.

### What the Numbers Mean

**Null syscall (102 cy / ~26 ns)**: A Ring 3 → Ring 0 → Ring 3 round-trip using SYSCALL/SYSRET. The benchmark runs entirely in Ring 3 assembly: RDTSC → SYSCALL (noop handler) → RDTSC. At 102 cycles, Lattice is **7.4× faster** than Linux's `getpid()` (191ns/764cy on Skylake-X), because there is no KPTI page table switch or Spectre mitigation overhead.

**Slab pop/push (103 cy / ~26 ns)**: A `lock cmpxchgq` CAS pop from the Treiber stack followed by a CAS push back. This is the fundamental allocation primitive for fiber stacks. At 103 cycles, it is **11.7× faster** than glibc `malloc`/`free` (300ns/1,200cy) and **2× faster** than `tcmalloc`'s hot path (~200cy).

**Arena alloc (60 cy / ~15 ns)**: A single arena slot allocation. Pure L1 cache territory: pointer increment, bounds check, return. This is the theoretical minimum for an allocator with no metadata bookkeeping.

**PMM alloc/free (78 cy / ~20 ns)**: A physical page allocation and free pair (LIFO stack). The benchmark pops and pushes in reverse order, so the hardware prefetcher predicts access patterns perfectly.

**IPC ping-pong (284 cy / ~71 ns)**: A sender-receiver pair yields back and forth via `sched_yield`. Each round-trip involves writing to a shared mailbox and two context switches. Zero-copy, zero privilege transition (shared address space unikernel). At 284 cycles, it is **42× faster** than Linux pipe IPC (~3µs) and competitive with seL4's verified IPC.

**Context switch (494 cy / ~124 ns at 4 fibers)**: The benchmark runs 3 tiers (4, 16, 64 fibers) to measure scheduler scaling. Each context switch includes a full `FXSAVE`/`FXRSTOR` (512 bytes of FPU/SSE state) and GPR save/restore. At 494 cycles (4-fiber baseline), it is **10.5× faster** than Linux context switches (1.3µs with CPU pinning). The gap between IPC (284 cy, 2 switches, no FPU) and the context switch (494 cy, 1 switch, with FPU) isolates the FXSAVE/FXRSTOR cost at ~350 cycles.

### UTP (Universal Task Pointer) Benchmarks

The UTP benchmarks measure the unified dispatch architecture where async and preemptive fibers share a single code path through `invoke_task(step_fn, ctx)`. All UTP results are from the February 24, 2026 KVM run.

**invoke_task direct (29 cy / ~7 ns)**: The bare dispatch primitive: `mov rax,rdi; mov rdi,rsi; call rax`. This is the cost of dispatching to any fiber type through a function pointer, regardless of whether the target is an async coroutine step function or a preemptive thread. At 29 cycles with 1,000 samples and a min of 28cy, this is **2-4× faster** than Tokio's task poll (~50-100cy) and **4-10× faster** than Go's goroutine resume (~100-300cy).

**Async yield round-trip (111 cy / ~28 ns)**: The full `sched_yield()` path for an async fiber that returns `POLL_READY`: bitmap scan (TZCNT) → load fiber fields (step_fn, task_frame) → `invoke_task` → check result → clear bitmap → decrement count → return. The 82cy gap between this and the bare `invoke_task` (29cy) is the scheduler bookkeeping cost.

**Spawn async (99 cy / ~25 ns)**: Create a new async fiber: bitmap scan for free slot → slab alloc (pop_stack) → initialize frame → set bitmap. At 99 cycles, this is **3-5× faster** than Go's goroutine creation (~300-500cy) and **100× faster** than Linux's `clone()` (~10,000+cy).

**Spawn preemptive (116 cy / ~29 ns)**: Same as async spawn plus IRETQ frame setup (`preemptive_stack_init`): write RIP, CS, RFLAGS, SS, RSP to the stack in the format expected by IRETQ. The 17cy delta (116 - 99) is the cost of setting up the hardware interrupt-return frame.

**Preemptive dispatch (430 cy / ~108 ns)**: The full IRETQ dispatch chain: `sched_yield()` → bitmap scan → `invoke_task(invoke_preemptive_thread, stack_ptr)` → push 6 callee-saved registers → save RSP to GS segment → pop 15 GPRs → IRETQ → thread executes → ret → `preemptive_exit_trampoline` → restore RSP → pop callee-saved → return POLL_READY. At 430 cycles, this is **12× faster** than Linux context switches (~5,200cy) and competitive with seL4's thread switch (~200-600cy). The 319cy gap between preemptive (430cy) and async (111cy) dispatch is dominated by the IRETQ hardware cost (~30-50cy) and the GPR save/restore (21 register operations).

**IRQ latency (33.9M cy / ~8.5 ms)**: Measures the cycle gap between consecutive PIT timer interrupts. The PIT is configured at 100 Hz (10 ms period). At 4.0 GHz, 10 ms = 40M cycles. The measured 33.9M average is consistent with PIT delivery jitter and the measurement window.

## TCG Results (February 25, 2026)

> [!NOTE]
> TCG emulates x86 instructions in software on the ARM host. Absolute cycle counts are inflated 20-40x. These numbers are useful for development but should not be cited as performance claims.

| Benchmark | Avg (cycles) | Min | Max | Samples |
|:----------|------------:|---------:|---------:|--------:|
| Arena alloc | 1,089 | 1,000 | 10,000 | 101 |
| Null syscall (SWAPGS) | 3,250 | 1,000 | 17,000 | 8 |
| PMM alloc/free | 1,079 | 1,000 | 11,000 | 126 |
| IPC ping-pong | 9,200 | 1,000 | 40,000 | 5 |
| UTP async yield | 1,214 | 1,000 | 7,000 | 28 |
| UTP async direct | 1,105 | 1,000 | 5,000 | 38 |
| UTP preempt dispatch | 1,135 | 1,000 | 6,000 | 37 |
| UTP spawn (preempt) | 1,073 | 1,000 | 4,000 | 41 |
| UTP spawn (async) | 1,125 | 1,000 | 5,000 | 32 |
| SMP PMM (per-core) | 1,018 | 1,000 | 5,000 | 218 |
| SMP Slab (per-core) | 1,029 | 1,000 | 8,000 | 238 |
| SIP IPC ring (4-SPSC) | 188 cy/pass | — | — | 1,000 |

TCG runs use a 100x divisor to reduce iteration counts (otherwise benchmarks take minutes under emulation).

> [!NOTE]
> **Ring 3 TDD Gates (February 25, 2026):** Three end-to-end Ring 3 isolation tests pass on every boot: Gate 1 (IRETQ frame: SS=0x23, CS=0x2B, RFLAGS=0x202 — 6/6), Gate 2 (KPTI: kernel_cr3 at GS:[64] — 3/3), Gate 3 (end-to-end: Ring 3 → SYSCALL(0xDEAD, 42) → exit_code=42 — 2/2). SWAPGS added to all syscall entry/exit paths.

## KVM Compatibility Fixes

Six bugs were discovered and fixed during KVM bring-up:

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
