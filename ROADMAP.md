# Lattice Roadmap

This roadmap outlines the progression of Lattice from a research kernel to a secure infrastructure for edge AI agents.

---

## Phase 1: The Sandbox *(Complete ✅)*

Userspace exploration and ABI fuzzing with a stable build process.

- **Onboarding** — Frictionless build process for Linux and macOS.
- **Verification** — Documentation on Z3-verified userspace programs.
- **Test Suites** — Robust set of Ring 3 test cases.

---

## Phase 2: The Sovereign ABI *(Current — v0.3.0-brutalism)*

Rip out legacy POSIX paradigms. The kernel becomes a pure **Control Plane** for resource multiplexing; the **Data Plane** bypasses the kernel entirely via Z3-verified zero-copy SPSC rings.

- **Hardware Abstraction Layer (HAL)** — Zero-cost compile-time dispatch (`kernel/arch/mod.salt`) ensuring `kernel/core/`, `kernel/mem/`, and `kernel/sched/` never import architecture-specific code. Supports x86_64 and aarch64.
- **Lock-Free Per-Core PMM** — Cacheline-padded Treiber stack with CAS work-stealing (`kernel/mem/pmm_sharded.salt`).
- **O(1) Scheduling** — Hierarchical 2-level bitmap with `ctz_u64` intrinsic, supporting 4096 tasks (`kernel/sched/bitmap_disp.salt`).
- **Fast-Path Register IPC** — Sub-microsecond signaling via CPU register injection (`kernel/ipc/fastpath.salt`).
- **Sovereign Reclaim** — 4-phase hardware-fenced teardown protocol (`kernel/core/teardown.salt`).
- **Codata Substrate** — Userspace `ReactiveStream` with functional `map_to` composition (`user/lib/codata.salt`).

> [!NOTE]
> ABI Status: **Level 0, Experimental.** System calls may change between commits.

---

## Phase 3: Service Orchestration *(Medium Term)*

Running non-trivial services in Ring 3 with a solidified core ABI.

- **IPC Formalization** — Finalize the SPSC ring buffer contract for userspace process communication.
- **Memory Allocation** — Implement basic allocation wrappers like `user.alloc` for `sys_brk`.
- **Service Porting** — Run a read-only version of the Lettuce state engine as a standalone userspace process.

---

## Phase 4: The AI Appliance *(Long Term)*

Realizing the end-to-end agent runtime vision.

- **Basalt Integration** — Port the Basalt reasoning engine into Ring 3.
- **Full Pipeline** — Run the complete NetD, Basalt, and Lettuce pipeline entirely as verified services.

---

## Phase 5: Open Ecosystem *(Future)*

General-purpose application development on a stable, formally verified foundation.

- **Standard Library** — Release a comprehensive `salt-std` for userspace.
- **Community Services** — Open the platform for a wider variety of community-submitted applications.
