# Tools

Developer tooling for the Salt ecosystem.

## Components

| Tool | Description | Status |
|------|-------------|--------|
| [`sp`](sp/) | **Salt Package Manager** — builds projects from `salt.toml`, content-addressed caching, dependency resolution | 🚧 Early |
| [`salt-lsp`](salt-lsp/) | **Language Server v0.2.0** — zero-I/O in-memory compilation via `salt-front` crate linkage, Z3 semantic hover, cross-file Go-to-Definition, SIR-powered completions, real-time diagnostics (<5ms) | ✅ Published |
| [`salt-build`](salt-build/) | **Build Scripts** — shell and Python helpers for the MLIR → LLVM compilation pipeline | ✅ Functional |

## Salt LSP

The Salt language server provides full IDE support for VS Code:

- **Diagnostics:** Two-tier system — instant pattern lints + in-memory `salt-front` compilation
- **Semantic Hover:** Function signatures, Z3 contract status (✅ Verified / ⚠️ Runtime Assertion), attributes
- **Go-to-Definition:** Cmd+Click navigation across files via `SirIndex` (handles syn 1-indexed → LSP 0-indexed conversion)
- **Completions:** SIR-powered function and struct names from compiled modules

```bash
# Build the LSP server (requires Z3)
cd salt-lsp
BINDGEN_EXTRA_CLANG_ARGS="-I/opt/homebrew/include" Z3_SYS_Z3_HEADER="/opt/homebrew/include/z3.h" LIBRARY_PATH="/opt/homebrew/lib" cargo build --release

# Install the VS Code extension
cd editors/vscode && npm install && npm run compile
```

**Architecture:** `salt-front` is linked as a library crate — no subprocess spawning, no temp files, no stderr parsing. The entire compilation pipeline runs in-memory.

See [`salt-lsp/`](salt-lsp/) for details.

## Package Manager (`sp`)

`sp` reads a `salt.toml` manifest and compiles Salt projects:

```bash
cd tools/sp && cargo build --release

# In a Salt project directory:
sp build
sp build --release
```

See [`sp/`](sp/) for details.
