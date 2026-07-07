# Lattice

**The Lattice project has been split into standalone repositories.** This
monorepo is preserved as a historical reference and index.

## Projects

| Repository | Description |
|------------|-------------|
| **[salt](https://github.com/bneb/salt)** | The Salt programming language — systems programming with Z3-powered compile-time verification |
| **[keuos](https://github.com/bneb/keuos)** | KeuOS microkernel — SMP, SPSC IPC, TCP stack, arena memory, Ring 3 userspace |
| **[basalt](https://github.com/bneb/basalt)** | Llama 2 inference in 1,600 lines of Salt — 920 tok/s, Z3-verified kernels, WASM |
| **[lettuce](https://github.com/bneb/lettuce)** | Redis-compatible server in 314 lines of Salt with Z3-proven buffer bounds |
| **[facet](https://github.com/bneb/facet)** | GPU 2D compositor in Salt — Metal backend, matches C performance |
| **[lattice-ecs](https://github.com/bneb/lattice-ecs)** | Entity Component System for Rust — no_std, used by KeuOS |

## Why the split?

The monorepo served its purpose during initial development, but each project
deserves its own identity, issue tracker, and release cadence. Salt is a
language. KeuOS is an OS. Basalt is an ML inference engine. They're different
things with different audiences.

## History

This monorepo contains the full commit history from January–July 2026, including
the compiler, kernel, and all applications. It is archived and read-only.

## License

MIT — applies to all split projects.
