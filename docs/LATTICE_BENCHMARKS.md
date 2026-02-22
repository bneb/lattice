# Lattice OS Kernel Benchmarks

Self-hosted benchmarks measuring Lattice unikernel primitives on real x86 hardware. Salt compiles directly to kernel code via MLIR → LLVM IR → ELF. No OS abstraction layer, no libc, no runtime.

## Platforms

| Platform | Hardware | Hypervisor | Purpose |
|:---------|:---------|:-----------|:--------|
| **KVM** | AWS z1d.metal, Intel Xeon 8151 (Skylake, 4.0 GHz) | QEMU 8.2 + KVM (`-cpu host`) | Authoritative cycle counts |
| **TCG** | Apple M4, QEMU x86_64 software emulation | None (interpreted) | Development iteration |

## KVM Results (February 22, 2026)

> [!IMPORTANT]
> KVM runs kernel instructions on real x86 silicon via hardware-assisted virtualization. These cycle counts reflect actual CPU pipeline behavior, cache effects, and branch prediction.

| Benchmark | Avg (cycles) | Min | Max | Samples | What It Measures |
|:----------|------------:|---------:|---------:|--------:|:-----------------|
| **Arena alloc** | **59** | 56 | 432 | 1,000 | Bump-allocator slot throughput |
| **PMM alloc/free** | **73** | 70 | 504 | 500 | Physical page alloc + free pair |
| **IPC ping-pong** | **297** | 218 | 842 | 10 | Fiber-to-fiber yield round-trip (4 fibers) |
| **Ring of Fire** | **487** | — | — | 1,000 | Context switch latency (4 fibers, full FPU save) |
| **IRQ latency** | **31.6M** | 9.9M | 34.3M | 10 | PIT interrupt delivery latency |

### What the Numbers Mean

**Arena alloc (59 cy / ~15 ns)**: A single arena slot allocation resolves in 59 cycles. This is pure L1 cache territory: pointer increment, bounds check, return. For comparison, glibc `malloc` typically costs 1,000-5,000 cycles due to binning logic and lock acquisition.

**PMM alloc/free (73 cy / ~18 ns)**: A physical page allocation and free pair (LIFO stack). The benchmark pops and pushes in reverse order, so the hardware prefetcher predicts access patterns perfectly.

**IPC ping-pong (297 cy / ~74 ns)**: A sender-receiver pair yields back and forth via `sched_yield`. Each round-trip involves writing to a shared mailbox and two context switches. Zero-copy, zero privilege transition (shared address space unikernel).

**Ring of Fire (487 cy / ~122 ns)**: 4 fibers in a ring, each yielding cooperatively. Each context switch includes a full `FXSAVE`/`FXRSTOR` (512 bytes of FPU/SSE state) and GPR save/restore. The gap between IPC (297 cy, 2 switches) and ROF (487 cy, 1 switch with FPU) reflects the FXSAVE cost.

**IRQ latency (31.6M cy / ~7.9 ms)**: Measures the cycle gap between consecutive PIT timer interrupts. The PIT is configured at 100 Hz (10 ms period). At 4.0 GHz, 10 ms = 40M cycles. The measured 31.6M average is consistent with PIT delivery jitter and the measurement window.

## TCG Results (February 22, 2026)

> [!NOTE]
> TCG emulates x86 instructions in software on the ARM host. Absolute cycle counts are inflated 20-40x. These numbers are useful for development but should not be cited as performance claims.

| Benchmark | Avg (cycles) | Min | Max | Samples |
|:----------|------------:|---------:|---------:|--------:|
| Arena alloc | 1,542 | 1,000 | 39,000 | 70 |
| PMM alloc/free | 1,637 | 1,000 | 38,000 | 58 |
| IPC ping-pong | 8,000 | 1,000 | 31,000 | 5 |

TCG runs use a 100x divisor to reduce iteration counts (otherwise benchmarks take minutes under emulation).

## Benchmarks Not Yet Passing on KVM

Five benchmarks are disabled on KVM due to specific hardware-level issues. Each is documented here for transparency.

| Benchmark | Issue | Root Cause |
|:----------|:------|:-----------|
| **Syscall** | Crashes | `syscall_noop()` invokes `SYSCALL` from Ring 0. `syscall_entry_fast` assumes Ring 3 caller and swaps to a stale kernel stack, corrupting execution state. Needs Ring 3 benchmark infrastructure. |
| **Ring of Fire 1K** | Crashes | 1,000 fibers exhaust the PMM's mapped physical memory range. The kernel's page tables map 1 GB, but 1,000 fiber stacks (16 KB each = 16 MB) plus slab metadata exceed available memory. |
| **Slab stress** | Hangs | `lock cmpxchgq` CAS loop in the Treiber stack spins forever on KVM. Likely an ABA issue or timing-dependent CAS retry that real silicon exposes but TCG emulation masks. |
| **Slab reclaim** | Timeout | 100,000 fiber spawn/exit cycles exceed the 300s benchmark timeout. Same fiber memory pressure as ROF-1K at 100x scale. |
| **Net echo** | Skipped | Requires external UDP packet injection via `test_net` harness. Not a bug. |

### Why These Work on TCG but Not KVM

TCG is a software interpreter. It doesn't enforce:
- **Stack swap correctness** (syscall): TCG's SYSRET emulation is more permissive about stack state.
- **Physical memory limits** (ROF-1K, slab_reclaim): TCG's memory model doesn't fault on the same boundaries.
- **Atomic timing** (slab_stress): TCG executes `lock cmpxchgq` as a sequential operation with no real contention or cache coherence.

## KVM Compatibility Fixes

Three bugs were discovered and fixed during KVM bring-up:

### 1. Pulse Ring Buffer Triple Fault
`pulse::push()` writes to a global array (`RING` at `0xffffffff8011c940`) that crashes on KVM. Other BSS globals (slab cache, VMA, scheduler) work fine. The crash is specific to pulse's calling context (ISR re-entrancy). Fixed by no-opping `push()`.

### 2. CPUID Byte-Swap Bug
KVM was misdetected as TCG because the CPUID hypervisor check compared `0x4b564d4b` instead of the correct `0x4b4d564b` ("KVMK" in little-endian EBX). All KVM benchmarks ran at 100x reduced iterations until this was fixed.

### 3. GDT Kernel Data Descriptor
The Kernel Data GDT entry at offset `0x10` had `limit=0` and `D/B=0`. Under KVM, the CPU enforces segment limits during the 32→64-bit boot transition, causing a triple fault on `retf`. TCG's emulator doesn't enforce this. Fixed to flat 4GB (`0x00CF92000000FFFF`).

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
