#!/bin/bash
# end-of-turn.sh — fires after each Claude turn completes.
# JSON output with "block" decision prevents stopping when quality gates fail.
# Non-JSON output + exit 0 = allow stop.
set -euo pipefail
cd /Users/kevin/projects/lattice || exit 0

# Only gate if Rust source files changed in this session
CHANGED=$(git diff --name-only HEAD 2>/dev/null | grep '\.rs$' || true)
if [ -z "$CHANGED" ]; then
  exit 0
fi

# ── Gate 1: Compilation check ─────────────────────────────────
CHECK_OUTPUT=$(cd salt-front && cargo check 2>&1) || true
if ! (cd salt-front && cargo check 2>&1) > /dev/null 2>&1; then
  echo "{\"decision\":\"block\",\"reason\":\"salt-front does not compile. Run cargo check and fix errors before stopping.\"}"
  exit 0
fi

# ── Gate 2: Clippy warnings (deny all) ────────────────────────
if ! (cd salt-front && cargo clippy -- -D warnings 2>&1) > /dev/null 2>&1; then
  echo "{\"decision\":\"block\",\"reason\":\"Clippy reports warnings. Fix all warnings (cargo clippy -- -D warnings) before stopping.\"}"
  exit 0
fi

exit 0
