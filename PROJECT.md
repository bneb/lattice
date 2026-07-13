# Project: Salt Ecosystem Anti-Slop Audit

## Architecture
The Salt ecosystem consists of 5 disaggregated repositories:
1. `salt`: Salt compiler and toolchain.
2. `keuos`: KeuOS kernel and operating system.
3. `basalt`: Cryptography, models, and WASM runtime.
4. `lettuce`: Verified network service.
5. `facet`: Window compositor and UI framework.

Each repository has its own code structure, docs, and test suites.

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|---|---|---|---|
| 1 | M1_salt | Audit salt repository, eradicate slop, and pass compiler/contract tests | None | IN_PROGRESS |
| 2 | M2_keuos | Audit keuos repository, eradicate slop, and pass kernel/QEMU tests | None | IN_PROGRESS |
| 3 | M3_basalt | Audit basalt repository, eradicate slop, and pass basalt tests | None | IN_PROGRESS |
| 4 | M4_lettuce | Audit lettuce repository, eradicate slop, and pass lettuce tests | None | IN_PROGRESS |
| 5 | M5_facet | Audit facet repository, eradicate slop, and pass facet tests | None | IN_PROGRESS |

## Interface Contracts
- None. This is an audit and clean-up task; no functional interfaces are being modified.
