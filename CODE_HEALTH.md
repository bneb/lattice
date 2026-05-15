# Code Health and Architecture Report

This report summarizes significant architectural and "code smell" issues identified during a deep manual audit of the codebase across all major subsystems.

## 1. Compiler Soundness & Debt (salt-front)
- **Disconnected Verification Logic**: `salt-front/src/codegen/ptr_bounds_verifier.rs`. Verification logic for pointer bounds is fully implemented but entirely unreferenced in the codegen path. This indicates a "shadow" safety system that provides no real-world protection.
- **Inconsistent Unification**: The generic resolver skips subsequent encounters of the same generic parameter, bypassing structural equivalence checks. This is a major architectural flaw that breaks the language's type-safety guarantee.

## 2. Kernel Architecture (Ring 0)
- **Missing Cross-Core TLB Shootdown**: `kernel/core/vmm.salt`. Guard pages only flush the local TLB, leading to severe incoherence across SMP cores.
- **GS-Base Initialization Race**: The interaction between `SYSCALL` (hardware segment swap) and the kernel's software `SWAPGS` is vulnerable to NMI interrupts, a classic "unfixable" architectural race if not handled with extreme care in the entry stubs.
- **Tightly Coupled Syscalls & Hardware**: `kernel/core/syscall.salt` contains hardware-specific logic for the "MoE TX Bridge".

## 3. Resilience & Safety
- **Manual Memory Management**: widespread use of `malloc`/`free` without RAII or lifecycle tracking leads to frequent leaks.
- **Lack of Error Propagation**: Many critical kernel functions return silently or with uninformative integers, losing diagnostic context.
- **Hardcoded Memory Layouts**: The kernel relies on specific virtual address constants (e.g., `0xFFFFFFFF80126000`), making the memory map extremely fragile.

## 4. Sub-optimal Algorithms & Performance Anti-Patterns
- **Linear Search in Hot Paths**: PID allocation and tokenization use $O(N)$ scans.
- **$O(N^2)$ Tokenizer Prompt Pre-scan**: Prompt encoding speed degrades exponentially with context length.
- **Excessive Mallocs in Tokenizer**: Initialization performs `malloc(1)` for every single byte, fragmenting the heap.
- **Hardcoded Engine Limits**: Llama 2 engine is artificially limited to 1GB models due to hardcoded mmap lengths.

## 5. UI/UX Bugs in System Processes
- **Debug Artifact in Production**: `user/browser/compositor.salt` unconditionally overwrites the first primitive's X-coordinate.
- **Busy-Waiting in Main Thread**: The browser UI freezes while waiting for shared-memory IPC.
