# Contributing to KeuOS

Welcome to the KeuOS project. We are currently in an **active research phase**, which means the internal structures and the system call ABI change frequently. We value contributions that help stabilize the platform and expand our userspace capabilities.

## Build Requirements

The project is currently standardized on **LLVM 21**. To build the kernel and the Salt compiler, you will need:

| Dependency | Purpose |
|:-----------|:--------|
| **LLVM 21** | `mlir-opt`, `mlir-translate`, and `clang` |
| **Rust 1.75+** | Builds the Salt compiler (`salt-front`) |
| **Z3 4.12+** | Formal verification of memory safety contracts |
| **Zsh + Python 3** | Build scripts and tooling |
| **clang + libclang-dev** | Required on Linux (Debian/Ubuntu) |

> [!TIP]
> We recommend using the provided Docker environment in the `tools/` directory to ensure a deterministic build.

## How to Contribute

We are specifically looking for help in these areas:

- **Userspace Tests** — Adding self-contained Ring 3 test programs to the `user/` directory.
- **Documentation** — Clarifying architecture docs or fixing inaccuracies in the README.
- **Bug Reports** — Reproducible reports for kernel panics or compiler crashes.

> [!IMPORTANT]
> We are **not** currently accepting major changes to the kernel internals or the compiler's core verification passes without prior discussion in the [GitHub Discussions](https://github.com/bneb/keuos/discussions) area.

## Submission Process

1. Fork the repository and create a feature branch.
2. Ensure your code passes the Z3 memory safety verifier.
3. All changes to the kernel or standard library **must** pass the TDD gates. Do not submit a PR unless `tools/runner_qemu.py` reports GREEN for your gates.
4. If your change affects cross-component compatibility (e.g., changing the IPC contract between the Kernel and NetD), you must update both components in the same atomic PR.
5. If you introduce a new system service or major application, propose adding it to `manifest.salt` in your PR description.
6. Submit a pull request. For small fixes, no associated issue is required.

## Versioning Policy: The KeuOS Distribution Model

KeuOS is built as a cohesive, keuos platform. We use a **Unified Versioning Strategy** for all repository-wide Git releases (e.g., `v0.9.0`).

- **Unified Git Tag** — Every major architectural milestone (e.g., moving the networking stack to Ring 3) gets a unified repository tag. When a user pulls a KeuOS release, that tag guarantees that a specific version of the Salt compiler is verified to build a specific version of the Kernel, NetD, and the Socket API ecosystem.
- **Internal Component Versions** — Individual sub-systems (Kernel, standard library, Basalt) track their own internal maturity versions in `manifest.salt` at the repository root. Build tools use this manifest to verify component synchronization.
- **Kernel Version Identification** — The kernel binary maintains its own version constant for the boot screen (e.g., `KEUOS BOOT [OK] v0.9.0`), decoupled from the compiler version.

## The HAL Mandate

> [!CAUTION]
> `kernel/core/`, `kernel/mem/`, and `kernel/sched/` are **strictly forbidden** from importing `kernel::arch::x86_64` or any architecture-specific module. All hardware operations must go through the compile-time HAL router (`kernel/arch/mod.salt`) or compiler intrinsics (e.g., `ctz_u64()`).

This ensures the System ABI remains portable across x86_64, aarch64 (Apple Silicon / AWS Graviton), and future RISC-V targets.

## Project Status

KeuOS is an experimental keuos systems language. **APIs and ABIs are subject to change without deprecation notices, but we will try to be as polite as possible.**
