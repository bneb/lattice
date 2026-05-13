# Code Health Report - High-Fidelity Audit

This report summarizes significant architectural and "code smell" issues identified during a deep manual audit of the codebase.

## 1. Architectural Leaks & Coupling
- **Tightly Coupled Syscalls & Hardware**: `kernel/core/syscall.salt` contains significant hardware-specific logic for the "MoE TX Bridge" (`moe_drain_bar`). This should be abstracted into a driver layer rather than living in the syscall dispatcher.
- **Leaky Abstractions in PMM**: The PMM (`kernel/core/pmm.salt`) exposes raw `u64` physical addresses throughout the kernel. Higher-level modules should interact with `PhysAddr` types or opaque handles to prevent accidental pointer arithmetic on physical addresses.
- **Global State Proliferation**: The kernel relies heavily on global variables (`CURRENT_PID`, `NEXT_PID`, `PMM_SHARDS`) without a unified "Kernel Context" or "Core Context" struct. This makes testing difficult and leads to the synchronization issues noted in `POSSIBLE_BUGS.md`.

## 2. Complexity & Length
- **Monolithic Files**: Several core files exceed 500 lines (`kernel/core/syscall.salt`, `kernel/core/scheduler.salt`, `kernel/core/main.salt`). These files handle too many responsibilities (e.g., `main.salt` handles memory, SMP, terminal logic, and process spawning).
- **Excessive Nesting**: Many compute kernels in `basalt/src/kernels.salt` and the scheduler logic use 4+ levels of indentation, making the control flow difficult to follow and increasing the risk of logic errors.

## 3. Resilience & Safety
- **Manual Memory Management**: The widespread use of `malloc`/`free` and raw pointer arithmetic in `basalt` and the kernel without RAII-like patterns (which Salt may not support) leads to the frequent memory leaks and OOM-handling bugs identified.
- **Lack of Error Propagation**: Many functions (e.g., `sys_write`, `sys_shm_grant`) return silently or with a simple `-1` on failure, losing critical diagnostic information. There is no structured error handling (Result types or similar) in the core logic.
- **Hardcoded Memory Layouts**: Stacks and page tables are often hardcoded to specific virtual addresses (e.g., `0xFFFFFFFF80126000`), making the system fragile to changes in the memory map.

## 4. Sub-optimal Algorithms
- **Linear Search in Hot Paths**: Tokenization and PID allocation use O(N) linear scans. For a system intended to scale (32k tokens, 16 processes), these should be replaced with HashMaps or bitsets.

---
*Note: This report is a high-level architectural summary. Specific line-by-line structural violations (indentation, scope length, etc.) are documented in the automated logs.*
