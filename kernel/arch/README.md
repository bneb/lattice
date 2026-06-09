# Architecture Specifics

**The Mission:** The hardware-dependent layer that bridges the gap between the chaotic reality of silicon and the pristine abstractions of the `core` kernel.

## Supported Architectures

| Architecture | Role | Status |
|--------------|------|--------|
| [`x86/`](./x86) | **Boot & Initialization.** The 32-bit trampoline code that sets up Long Mode. | **Stable** |
| [`x86_64/`](./x86_64) | **64-bit Runtime.** The actual runtime architecture for the KeuOS microkernel. | **Stable** |

## Invariants

> [!CAUTION]
> **Performance Limits**
> The context switch mechanism defined here must compete with optimized C (switch_to.s) and Rust (context_switch.rs) implementations.

### The ABI Boundary
Code in this directory is the *only* place where the Salt ABI interacts with the raw hardware ABI (System V AMD64).
- **Alignment:** We explicitly enforce 16-byte stack alignment at function boundaries where Salt does not.
- **Registers:** We manually preserve Callee-Saved registers (RBX, RBP, R12-R15) during context switches.
