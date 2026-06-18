#!/bin/bash
# check-constraints.sh — fires after every Write/Edit.
# Exit 2 = block the change, feed stderr to Claude as feedback.
# Exit 0 = allow.
set -euo pipefail

INPUT=$(cat)
FILE=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')

# Only check Rust and Salt source files
if [[ "$FILE" != *.rs && "$FILE" != *.salt ]]; then
  exit 0
fi

# Skip vendor/ and generated files
if [[ "$FILE" == vendor/* ]] || [[ "$FILE" == isodir/* ]]; then
  exit 0
fi

FAIL=0

# ── Constraint 1: File max 500 lines ──────────────────────────
LINES=$(wc -l < "$FILE" 2>/dev/null || echo 0)
if [ "$LINES" -gt 500 ]; then
  echo "CONSTRAINT VIOLATION [max-500-lines]: $FILE is $LINES lines (limit: 500)." >&2
  echo "  Action: Split this file into smaller modules before proceeding." >&2
  FAIL=1
fi

# ── Constraint 2: Functions max 32 non-blank lines ────────────
# Uses awk to track fn start lines and detect >32-line functions.
# Pattern matches Rust `fn` and Salt `fn` declarations.
awk '
  /^\s*(pub(\s*\(\s*(crate|super|self)\s*\))?\s+)?(unsafe\s+)?(extern\s+)?fn\s/ {
    if (start && end_seen) {
      body = NR - start
      if (body > 32) {
        printf "LONG_FN %d %s:%d\n", body, FILENAME, start
      }
    }
    start = NR; end_seen = 0; brace_depth = 0
  }
  /{/ { if (start) brace_depth += gsub(/{/, "&") }
  /}/ {
    if (start) {
      brace_depth -= gsub(/}/, "&")
      if (brace_depth <= 0) { end_seen = 1 }
    }
  }
  END {
    if (start && !end_seen && NR - start > 32) {
      printf "LONG_FN_EOF %d %s:%d\n", NR - start, FILENAME, start
    }
  }
' "$FILE" 2>/dev/null | while read -r kind body_len loc; do
  echo "CONSTRAINT VIOLATION [max-32-lines-fn]: Function at $loc is ~$body_len lines (limit: 32)." >&2
  echo "  Action: Extract helper functions or split logic." >&2
done

# Check if any LONG_FN was found (pipeline in subshell, check via temp)
LONG_FNS=$(awk '
  /^\s*(pub(\s*\(\s*(crate|super|self)\s*\))?\s+)?(unsafe\s+)?(extern\s+)?fn\s/ {
    if (start && end_seen && NR - start > 32) { count++ }
    start = NR; end_seen = 0; brace_depth = 0
  }
  /{/ { if (start) brace_depth += gsub(/{/, "&") }
  /}/ {
    if (start) {
      brace_depth -= gsub(/}/, "&")
      if (brace_depth <= 0) { end_seen = 1 }
    }
  }
  END {
    if (start && !end_seen && NR - start > 32) { count++ }
    print count + 0
  }
' "$FILE" 2>/dev/null)
if [ "${LONG_FNS:-0}" -gt 0 ]; then
  FAIL=1
fi

# ── Constraint 3: Max 3 levels of nesting ─────────────────────
# 16 spaces = 4 indents = 4 levels deep (0-indexed: level 0,1,2,3,4 = 5 levels)
# We flag 16+ spaces of indent before if/match/while/for/loop
DEEP_BLOCKS=$(grep -cP '^\s{16,}(if|match|while|for|loop)\b' "$FILE" 2>/dev/null || echo 0)
if [ "${DEEP_BLOCKS:-0}" -gt 0 ]; then
  echo "CONSTRAINT VIOLATION [max-3-nesting]: $DEEP_BLOCKS block(s) at nesting level 4+ in $FILE." >&2
  echo "  Action: Extract deeply nested logic into a helper function." >&2
  FAIL=1
fi

# ── Constraint 4: No mutants (TODO/FIXME/HACK/XXX/temp/workaround) ──
if [[ "$FILE" != *test* && "$FILE" != *spec* && "$FILE" != *tests_* ]]; then
  MUTANTS=$(grep -nP '(TODO|FIXME|HACK|XXX|workaround|temp_)' "$FILE" 2>/dev/null | grep -v '//.*CONSTRAINT' || true)
  if [ -n "$MUTANTS" ]; then
    echo "CONSTRAINT VIOLATION [no-mutants]: Markers found in $FILE:" >&2
    echo "$MUTANTS" | while read -r line; do
      echo "  $line" >&2
    done
    echo "  Action: Resolve the issue now or open a GitHub issue — don't leave markers in code." >&2
    FAIL=1
  fi
fi

# ── Result ────────────────────────────────────────────────────
if [ "$FAIL" -eq 1 ]; then
  exit 2
fi
exit 0
