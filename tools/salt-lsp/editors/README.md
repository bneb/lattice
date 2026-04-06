# Salt — Editor & AI Integration

Salt ships a TextMate grammar and an optional LSP server for syntax highlighting, completions, and diagnostics.

---

## Quick Start (VS Code / Antigravity / Any VS Code Fork)

### Option A: Install from VSIX (Recommended)

```bash
cd tools/salt-lsp/editors/vscode
npm install
npx -y @vscode/vsce package     # produces salt-language-0.1.0.vsix
```

Then inside your editor:

**Cmd+Shift+P** → **"Extensions: Install from VSIX..."** → select `salt-language-0.1.0.vsix`

This works in **VS Code, Antigravity, Cursor, Windsurf**, and any other VS Code fork — the VSIX install path handles extension registration automatically.

### Option B: CLI Install

```bash
# VS Code
code --install-extension salt-language-0.1.0.vsix

# If 'code' is not in PATH, use the full path:
# macOS VS Code:
"/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code" \
    --install-extension salt-language-0.1.0.vsix
```

### Option C: Development Mode (no packaging)

Launch your editor with the extension loaded from source:

```bash
code --extensionDevelopmentPath=tools/salt-lsp/editors/vscode .
```

> **Note**: This only applies to the launched window and does not persist.

---

## AI Coding Assistants

### Codex (OpenAI)

Codex uses a sandboxed environment. To enable Salt awareness:

1. **Agent instructions** — Place a `AGENTS.md` file in your project root (or `codex.md`). Codex reads this at startup. Reference the Salt skill file:

```markdown
## Salt Language

Salt (.salt) is a systems language with MLIR codegen and Z3 verification.
See `.agent/skills/salt-language/SKILL.md` for full syntax reference.

Build: `./scripts/build.sh`
Test:  `./scripts/run_test.sh tests/<test>.salt`
```

2. **Syntax highlighting** — Codex does not run VS Code extensions. Salt syntax highlighting is not available in Codex's terminal-based UI. However, Codex will correctly read and write `.salt` files using the skill file for conventions.

### Claude Code (Anthropic)

Claude Code is a terminal-based agent. To enable Salt awareness:

1. **Project instructions** — Place a `CLAUDE.md` file in your project root:

```markdown
## Salt Language

Salt (.salt) is a systems language. Key conventions:
- Explicit `return` always — no implicit returns
- Error handling: `Result<T>` with `Status` (8 bytes, 16 canonical codes)
- Imports: `use std.core.result.Result` (dot-separated, never `import`)
- Build: `./scripts/build.sh`
- Test: `./scripts/run_test.sh tests/<test>.salt`

See `.agent/skills/salt-language/SKILL.md` for full syntax and stdlib reference.
```

2. **Syntax highlighting** — Claude Code runs in a terminal and does not support TextMate grammars. It will correctly read, write, and reason about `.salt` files using the project instructions.

### Antigravity (Google DeepMind)

Antigravity is a VS Code fork. Two integration paths:

1. **Syntax highlighting** — Install the VSIX via Command Palette:
   - **Cmd+Shift+P** → **"Extensions: Install from VSIX..."** → select `salt-language-0.1.0.vsix`

2. **Agent skill** — Already configured at `.agent/skills/salt-language/SKILL.md`. Antigravity reads this automatically when working on `.salt` files.

3. **Workflows** — Build, test, and benchmark workflows are at `.agent/workflows/salt-build.md` and `.agent/workflows/salt-benchmarks.md`.

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
| **Attributes**  | `@derive`, `@yielding`, `@pulse`, `@inline`, `@pure`, `@trusted` |
| **Types**       | `i32`, `f64`, `bool`, `Ptr<T>`, `Result<T>`, `String`, `Vec` |
| **F-strings**   | `f"Hello {name}"` with embedded expressions |
| **Operators**   | `->`, `=>`, `::`, `..`, `?`, `|>`, `|?>`, `@` |
| **Constants**   | `true`, `false`, `self`                     |

---

## Teaching AI Agents to Write Salt

Salt is a new language — **no LLM has it in its training data**. For any AI coding agent to write correct Salt, your project needs a language reference file that the agent reads at startup.

### Key Resources

| Resource | What it covers |
|----------|---------------|
| [SYNTAX.md](../../SYNTAX.md) | Full syntax reference — types, control flow, traits, verification, sugar |
| [SKILL.md](../../.agent/skills/salt-language/SKILL.md) | Agent-ready cheat sheet — conventions, abolished patterns, build commands |
| [std/ README](../../std/README.md) | Standard library module map (79 modules) |
| [Salt repo](https://github.com/salt-lang/keuos) | Source, examples, benchmarks |

### Per-Platform Instruction Files

Each AI platform reads a different file at project root:

| Platform | File | Notes |
|----------|------|-------|
| **Antigravity** | `.agent/skills/salt-language/SKILL.md` | Auto-discovered by the agent |
| **Codex** | `AGENTS.md` or `codex.md` | Read at sandbox startup |
| **Claude Code** | `CLAUDE.md` | Read at session start |
| **Gemini CLI** | `GEMINI.md` | Read at session start |
| **GitHub Copilot** | `.github/copilot-instructions.md` | Workspace-level instructions |
| **Cursor** | `.cursorrules` | Project-level rules file |

### What to Include

At minimum, the instruction file should cover Salt's **three permanent bets**:

```markdown
## Salt Language Conventions

1. **Explicit `return`** — every function with a return type MUST use `return`. No implicit returns.
2. **`Result<T>` with `Status`** — all errors use `Status` (8 bytes, 16 canonical gRPC codes). Never `Result<T, E>`.
3. **`use` not `import`** — imports are dot-separated: `use std.core.result.Result`

Full reference: see SYNTAX.md and .agent/skills/salt-language/SKILL.md
Build: ./scripts/build.sh
Test:  ./scripts/run_test.sh tests/<test>.salt
```

