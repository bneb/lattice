# Lattice OS Driver Model specification

> [!NOTE]
> This document describes a **design target**, not the current implementation. The `Window<T>` and `map_window` primitives have not been implemented yet. Current Lattice drivers use raw port I/O (`io.outb`/`io.inb`) via assembly FFI and direct memory-mapped addresses. See [`kernel/drivers/`](../kernel/drivers/) for the current driver code.

Lattice replaces the traditional kernel-mode driver model (Linux/Windows) with a compiler-verified, zero-overhead Safety Model.

## 1. The Core Thesis: "Compiler IS the OS"
In traditional systems, the kernel protects hardware via MMU page tables and privilege rings (Ring 0 vs Ring 3). Lattice eliminates this runtime overhead by statically verifying memory safety and hardware access policies at compile time.

### The "Zero-Context-Switch" Driver
Because drivers are proven safe, they run in the same address space as the kernel *without* protection boundaries.
- **Latency**: No Ring transition (syscall) overhead.
- **Throughput**: Zero-copy data paths from NIC to Application.

## 2. The `map_window` Primitive
Drivers do not access arbitrary physical memory. Instead, they declare a `Window<T>` which maps a specific MMIO region.

```salt
// Mapping a VGA Buffer (0xB8000)
// The compiler treats strict layout requirements for 'struct VGA'
let vga: Window<VGA> = map_window(0xB8000, 4096, "VGA");
```

### Safety Properties
1.  **Exclusivity**: Only one active `Window` can exist for a physical range (Linear Type System).
2.  **Layout**: The mapped type `T` must be `#[repr(C)]` or equivalent (verified at compile time by the type checker).
3.  **Bounds**: All accesses are bounds-checked or statically proven safe.

## 3. Atomic Hardware Access
Hardware is inherently concurrent. Salt enforces that all MMIO fields must be accessed via `Atomic<T>` wrappers to prevent data races and undefined behavior from compiler optimizations.

```salt
struct NetworkCard {
    command_reg: Atomic<u32>,
    status_reg: Atomic<u32>,
}

fn send_packet(nic: &mut Window<NetworkCard>) {
    // 1. Check Status (Atomic Load with Acquire ordering)
    while (nic.status_reg.load() & BUSY) != 0 {}
    
    // 2. Write Command (Atomic Store with Release ordering)
    nic.command_reg.store(SEND_CMD);
}
```

## 4. Comparison with Linux
| Feature | Linux Kernel Module | Lattice Driver |
|---|---|---|
| **Language** | C (Manual Safety) | Salt (Verified Safety) |
| **Isolation** | Runtime (MMU/Ring) | Compile-time (Type System) |
| **Bugs** | Segfaults Panic Kernel | Compiler Error |
| **Performance** | Context Switch Overhead | Native Function Call |

## 5. Verification Strategy
The Lattice Compiler (`salt-front`) and Verifier (`salt-opt`) work together:
1.  **Frontend**: Ensures distinct ownership of Windows (`map_window` checks).
2.  **Backend (Z3)**: Proves that indices into MMIO windows are within bounds, even for variable-length descriptors.
