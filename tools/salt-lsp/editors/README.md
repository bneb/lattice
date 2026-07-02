# Salt — Editor Integration

Salt ships a TextMate grammar and an LSP server (v0.3.0) for semantic highlighting, go-to-definition, find-references, document symbols, code actions, and in-memory diagnostics.

---

## Quick Start (VS Code and VS Code-compatible editors)

### Option A: Install from VSIX (Recommended)

```bash
cd tools/salt-lsp/editors/vscode
npm install
npx -y @vscode/vsce package     # produces salt-language-0.3.0.vsix
```

Then inside your editor:

**Cmd+Shift+P** → **"Extensions: Install from VSIX..."** → select `salt-language-0.3.0.vsix`

### Option B: CLI Install

```bash
code --install-extension salt-language-0.3.0.vsix
```

### Option C: Development Mode (no packaging)

```bash
code --extensionDevelopmentPath=tools/salt-lsp/editors/vscode .
```

---

## AI Coding Assistants

Salt is a new language — no LLM has it in its training data. For any AI coding agent to write correct Salt, place a language reference file in your project root that the agent reads at startup. The exact filename varies by platform.

### What to Include

At minimum, the instruction file should cover:

```markdown
## Salt Language Conventions

1. **Explicit `return`** — every function with a return type MUST use `return`. No implicit returns.
2. **`Result<T>` with `Status`** — all errors use `Status` (8 bytes, 16 canonical codes). Never `Result<T, E>`.
3. **`import` syntax** — imports use dot-separated paths: `import std.core.result.Result`

Full reference: see SYNTAX.md and .agent/skills/salt-language/SKILL.md
Build: ./scripts/build.sh
Test:  ./scripts/run_test.sh tests/<test>.salt
```

### Key Resources

| Resource | What it covers |
|----------|---------------|
| [SYNTAX.md](../../SYNTAX.md) | Full syntax reference |
| [SKILL.md](../../.agent/skills/salt-language/SKILL.md) | Agent-ready cheat sheet |
| [std/ README](../../std/README.md) | Standard library module map |

---

## LSP Server (Optional)

The LSP provides completions and diagnostics beyond syntax highlighting.

```bash
cd tools/salt-lsp
cargo build
```

The VS Code extension auto-detects the LSP binary at `tools/salt-lsp/target/debug/salt-lsp`. If the binary is not found, the extension gracefully falls back to syntax-highlighting-only mode.

Override the binary path:
```bash
export SALT_LSP_PATH=/path/to/salt-lsp
```

### Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| No syntax highlighting after install | Extension not activated | Reload window: **Cmd+Shift+P** → **"Developer: Reload Window"** |
| LSP features missing (completions, diagnostics) | LSP binary not built | `cd tools/salt-lsp && cargo build` |
| `cargo build` fails for LSP | Missing Rust toolchain | `rustup update` |
| VSIX packaging fails | Missing node dependencies | `cd tools/salt-lsp/editors/vscode && npm install` |

---

## What the Grammar Highlights

| Element         | Examples                                    |
|-----------------|---------------------------------------------|
| **Keywords**    | `fn`, `let`, `mut`, `struct`, `enum`, `impl`, `match`, `return` |
| **Verification**| `requires`, `ensures`, `invariant`          |
| **Attributes**  | `@derive`, `@yielding`, `@pulse`, `@inline`, `@trusted` |
| **Types**       | `i32`, `f64`, `bool`, `Ptr<T>`, `Result<T>`, `String`, `Vec` |
| **F-strings**   | `f"Hello {name}"` with embedded expressions |
| **Operators**   | `->`, `=>`, `::`, `..`, `?`, `|>`, `|?>`, `@` |
| **Constants**   | `true`, `false`, `self`                     |
