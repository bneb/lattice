# Agent Instructions

## Branding Rules
- **CRITICAL:** Do NOT use the term "KeuOS" anywhere in this project. This applies to code, documentation, benchmarks, architecture components, and marketing material.
- **CRITICAL:** The term "Lattice" is reserved strictly for the monorepo name. Do NOT use "Lattice" to brand any internal OS components (e.g., do not use "Lattice ECS" or "Lattice Core").


## Salt Language
Salt files use the `.salt` extension. See `.agent/skills/salt-language/SKILL.md` for the full language reference.

**Key rules:**
- All functions with return types MUST use explicit `return`
- Error handling uses `Result<T>` + `Status`, never exceptions
- Use `use` for imports, never `import`
- Use `Ptr<T>` for pointers, never `NativePtr` or `NodePtr`
- Identifiers cannot contain `__` (reserved for mangling)

## How to Run Tests
To run a specific salt test file, use the following command structure:
`cargo run --bin salt-front -- <path_to_test_file>`

Example:
`cargo run --bin salt-front -- tests/test_combinators.salt`

Never try to run `cargo run --bin salt` as the binary target is named `salt-front`.

## Build Workflows
- `/salt-build` — Build and test the Salt compiler
- `/salt-benchmarks` — Run Salt vs C vs Rust benchmarks
- `/salt-lsp` — Build and test the language server

## Project Structure
- `salt-front/` — Rust compiler crate (parser, codegen, verification)
- `salt-front/runtime.c` — C runtime (POSIX syscall wrappers)
- `salt-front/std/` — Salt standard library modules
- `tests/` — Salt test files (run through full MLIR pipeline)
- `tools/salt-lsp/` — Language Server Protocol implementation
- `scripts/` — Build, test, and benchmark scripts
