#!/usr/bin/env bash
# project_stats.sh — Idempotent project meta-analysis for the KeuOS repo.
# Outputs stats tied to the current HEAD commit.
# Usage:
#   ./scripts/project_stats.sh          # human-readable
#   ./scripts/project_stats.sh --json   # JSON output
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# ─── Helpers ──────────────────────────────────────────────────────────────────
# find_src: find source files, excluding all build/vendored dirs globally
find_src() {
  find . "$@" \
    -not -path './.git/*' \
    -not -path '*/target/*' \
    -not -path './bazel-*' \
    -not -path './coverage_report/*' \
    -not -path '*/.venv/*' \
    -not -path '*/node_modules/*' \
    -not -path './qemu_build/*' \
    2>/dev/null
}

count_loc() {
  local files
  files=$(find_src "$@")
  if [[ -z "$files" ]]; then
    echo 0
  else
    echo "$files" | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}'
  fi
}

count_files() {
  find_src "$@" | wc -l | tr -d ' '
}

# Safely grep-count a pattern across files in a directory
# Usage: gcount <pattern> <include_glob> <dir>
gcount() {
  local pattern="$1" include="$2" dir="${3:-.}"
  grep -rc "$pattern" "$dir" --include="$include" 2>/dev/null \
    | awk -F: '{s+=$2}END{print s+0}'
}

# ─── Git metadata ────────────────────────────────────────────────────────────
COMMIT_HASH=$(git rev-parse HEAD 2>/dev/null || echo "unknown")
COMMIT_SHORT=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
COMMIT_DATE=$(git log -1 --format='%ci' 2>/dev/null || echo "unknown")
COMMIT_MSG=$(git log -1 --format='%s' 2>/dev/null || echo "unknown")
SNAPSHOT_DATE=$(date -u '+%Y-%m-%dT%H:%M:%SZ')

# ─── LOC by language ─────────────────────────────────────────────────────────
LOC_RUST=$(count_loc -name '*.rs')
LOC_SALT=$(count_loc -name '*.salt')
LOC_C=$(count_loc \( -name '*.c' -o -name '*.h' \))
LOC_ASM=$(count_loc -name '*.S')
LOC_HTML=$(count_loc -name '*.html')
LOC_CSS=$(count_loc -name '*.css')
LOC_JS=$(count_loc -name '*.js')
LOC_MD=$(count_loc -name '*.md')
LOC_PYTHON=$(count_loc -name '*.py')
LOC_SHELL=$(count_loc -name '*.sh')
LOC_TOML=$(count_loc -name '*.toml')
LOC_LD=$(count_loc -name '*.ld')

FILES_RUST=$(count_files -name '*.rs')
FILES_SALT=$(count_files -name '*.salt')

LOC_TOTAL=$((LOC_RUST + LOC_SALT + LOC_C + LOC_ASM + LOC_HTML + LOC_CSS + LOC_JS + LOC_MD + LOC_PYTHON + LOC_SHELL + LOC_TOML + LOC_LD))

# ─── Testing ──────────────────────────────────────────────────────────────────
TEST_FNS=$(find_src -name '*.rs' | xargs grep -c '#\[test\]' 2>/dev/null | awk -F: '{s+=$2}END{print s+0}')
TEST_MODULES=$(find_src -name '*.rs' | xargs grep -c '#\[cfg(test)\]' 2>/dev/null | awk -F: '{s+=$2}END{print s+0}')

TEST_RUST_LOC=0
rust_test_files=$(find . -path '*/tests/*' -name '*.rs' -not -path '*/target/*' -not -path './.git/*' -not -path './bazel-*' -not -path './coverage_report/*' 2>/dev/null)
if [[ -n "$rust_test_files" ]]; then
  TEST_RUST_LOC=$(echo "$rust_test_files" | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}')
fi

TEST_SALT_FILES=$(find ./tests -name '*.salt' 2>/dev/null | wc -l | tr -d ' ')

TEST_SALT_LOC=0
salt_test_files=$(find ./tests -name '*.salt' 2>/dev/null)
if [[ -n "$salt_test_files" ]]; then
  TEST_SALT_LOC=$(echo "$salt_test_files" | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}')
fi

TEST_TOTAL_LOC=$((TEST_RUST_LOC + TEST_SALT_LOC))

# ─── Compiler internals (salt-front) ─────────────────────────────────────────
compiler_files=$(find ./salt-front/src -name '*.rs' 2>/dev/null)
COMPILER_LOC=0
[[ -n "$compiler_files" ]] && COMPILER_LOC=$(echo "$compiler_files" | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}')

codegen_files=$(find ./salt-front/src/codegen -name '*.rs' 2>/dev/null)
CODEGEN_LOC=0
CODEGEN_FILES=0
if [[ -n "$codegen_files" ]]; then
  CODEGEN_LOC=$(echo "$codegen_files" | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}')
  CODEGEN_FILES=$(echo "$codegen_files" | wc -l | tr -d ' ')
fi

type_files=$(find ./salt-front/src -name '*type*' -name '*.rs' 2>/dev/null)
TYPE_LOC=0
[[ -n "$type_files" ]] && TYPE_LOC=$(echo "$type_files" | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}')

COMPILER_STRUCTS=$(gcount 'struct ' '*.rs' salt-front/src/)
COMPILER_ENUMS=$(gcount 'enum ' '*.rs' salt-front/src/)
MLIR_OPS=$(grep -roh 'llvm\.[a-z_]*\|arith\.[a-z_]*\|memref\.[a-z_]*\|func\.[a-z_]*\|scf\.[a-z_]*\|affine\.[a-z_]*\|cf\.[a-z_]*' salt-front/src/ --include='*.rs' 2>/dev/null | sort -u | wc -l | tr -d ' ')
Z3_REFS=$(gcount 'z3\|Z3\|solver\|Solver' '*.rs' salt-front/src/)
UNSAFE_COUNT=$(gcount 'unsafe' '*.rs' salt-front/src/)
COMPILER_TEST_FNS=$(gcount '#\[test\]' '*.rs' salt-front/src/)

# ─── Salt ecosystem ──────────────────────────────────────────────────────────
SALT_FNS=$(gcount '^fn \|^pub fn ' '*.salt' .)
SALT_STRUCTS=$(gcount '^struct \|^pub struct ' '*.salt' .)
SALT_CONTRACTS=$(gcount 'requires\|ensures' '*.salt' .)
SALT_ATTRS=$(grep -roh '@[a-z_]*' --include='*.salt' . 2>/dev/null | sort -u | wc -l | tr -d ' ')

SALT_STDLIB_MODULES=$(find ./salt -name '*.salt' -type f 2>/dev/null | wc -l | tr -d ' ')

stdlib_files=$(find ./salt -name '*.salt' 2>/dev/null)
SALT_STDLIB_LOC=0
[[ -n "$stdlib_files" ]] && SALT_STDLIB_LOC=$(echo "$stdlib_files" | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}')

SALT_BENCH_FILES=$(find ./benchmarks -name '*.salt' 2>/dev/null | wc -l | tr -d ' ')
bench_files=$(find ./benchmarks -name '*.salt' 2>/dev/null)
SALT_BENCH_LOC=0
[[ -n "$bench_files" ]] && SALT_BENCH_LOC=$(echo "$bench_files" | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}')

SALT_EXAMPLE_FILES=$(find ./examples -name '*.salt' 2>/dev/null | wc -l | tr -d ' ')

kernel_files=$(find ./kernel -name '*.salt' 2>/dev/null)
SALT_KERNEL_LOC=0
[[ -n "$kernel_files" ]] && SALT_KERNEL_LOC=$(echo "$kernel_files" | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}')

# ─── Components ───────────────────────────────────────────────────────────────
tools_files=$(find_src -path './tools/*' -name '*.rs')
TOOLS_LOC=0
[[ -n "$tools_files" ]] && TOOLS_LOC=$(echo "$tools_files" | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}')

lettuce_files=$(find ./lettuce \( -name '*.rs' -o -name '*.salt' \) 2>/dev/null)
LETTUCE_LOC=0
[[ -n "$lettuce_files" ]] && LETTUCE_LOC=$(echo "$lettuce_files" | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}')

# ─── Fun facts ────────────────────────────────────────────────────────────────
COMMENT_RUST=$(find_src -name '*.rs' | xargs grep -c '^\s*//' 2>/dev/null | awk -F: '{s+=$2}END{print s+0}')
COMMENT_SALT=$(find . -name '*.salt' | xargs grep -c '^\s*//' 2>/dev/null | awk -F: '{s+=$2}END{print s+0}')
TODO_COUNT=$(find_src -name '*.rs' | xargs grep -ci 'TODO\|FIXME\|HACK\|XXX' 2>/dev/null | awk -F: '{s+=$2}END{print s+0}')
CARGO_DEPS=$(find_src -name 'Cargo.toml' | xargs grep '^\w.*=' 2>/dev/null | grep -v '\[' | grep -v '^#' | awk -F= '{print $1}' | sed 's/ //g' | sort -u | wc -l | tr -d ' ')

LARGEST_RUST=$(find_src -name '*.rs' | xargs wc -l 2>/dev/null | sort -rn | sed -n '2p' | awk '{print $2 " (" $1 " lines)"}')
LARGEST_SALT=$(find . -name '*.salt' | xargs wc -l 2>/dev/null | sort -rn | sed -n '2p' | awk '{print $2 " (" $1 " lines)"}')

# ─── Output ───────────────────────────────────────────────────────────────────
if [[ "${1:-}" == "--json" ]]; then
  # Escape any double quotes in commit message
  COMMIT_MSG_ESCAPED=$(echo "$COMMIT_MSG" | sed 's/"/\\"/g')
  cat <<EOF
{
  "snapshot": {
    "commit": "$COMMIT_HASH",
    "commit_short": "$COMMIT_SHORT",
    "commit_date": "$COMMIT_DATE",
    "commit_message": "$COMMIT_MSG_ESCAPED",
    "generated_at": "$SNAPSHOT_DATE"
  },
  "loc_by_language": {
    "rust": $LOC_RUST,
    "salt": $LOC_SALT,
    "c": $LOC_C,
    "assembly": $LOC_ASM,
    "html": $LOC_HTML,
    "css": $LOC_CSS,
    "javascript": $LOC_JS,
    "markdown": $LOC_MD,
    "python": $LOC_PYTHON,
    "shell": $LOC_SHELL,
    "toml": $LOC_TOML,
    "linker_scripts": $LOC_LD,
    "total": $LOC_TOTAL
  },
  "file_counts": {
    "rust": $FILES_RUST,
    "salt": $FILES_SALT
  },
  "testing": {
    "rust_test_functions": $TEST_FNS,
    "rust_test_modules": $TEST_MODULES,
    "rust_test_loc": $TEST_RUST_LOC,
    "salt_test_files": $TEST_SALT_FILES,
    "salt_test_loc": $TEST_SALT_LOC,
    "total_test_loc": $TEST_TOTAL_LOC
  },
  "compiler": {
    "total_loc": $COMPILER_LOC,
    "codegen_loc": $CODEGEN_LOC,
    "codegen_files": $CODEGEN_FILES,
    "type_system_loc": $TYPE_LOC,
    "structs": $COMPILER_STRUCTS,
    "enums": $COMPILER_ENUMS,
    "mlir_ops": $MLIR_OPS,
    "z3_references": $Z3_REFS,
    "unsafe_usages": $UNSAFE_COUNT,
    "test_functions": $COMPILER_TEST_FNS
  },
  "salt_ecosystem": {
    "functions": $SALT_FNS,
    "structs": $SALT_STRUCTS,
    "contracts": $SALT_CONTRACTS,
    "distinct_attributes": $SALT_ATTRS,
    "stdlib_modules": $SALT_STDLIB_MODULES,
    "stdlib_loc": $SALT_STDLIB_LOC,
    "benchmark_files": $SALT_BENCH_FILES,
    "benchmark_loc": $SALT_BENCH_LOC,
    "example_files": $SALT_EXAMPLE_FILES,
    "kernel_loc": $SALT_KERNEL_LOC
  },
  "components": {
    "compiler_loc": $COMPILER_LOC,
    "tools_loc": $TOOLS_LOC,
    "salt_total_loc": $LOC_SALT,
    "kernel_loc": $SALT_KERNEL_LOC,
    "lettuce_loc": $LETTUCE_LOC
  },
  "fun_facts": {
    "comment_lines_rust": $COMMENT_RUST,
    "comment_lines_salt": $COMMENT_SALT,
    "todo_fixme_count": $TODO_COUNT,
    "cargo_dependencies": $CARGO_DEPS,
    "largest_rust_file": "$LARGEST_RUST",
    "largest_salt_file": "$LARGEST_SALT"
  }
}
EOF
  exit 0
fi

# ─── Human-readable output ───────────────────────────────────────────────────
cat <<EOF

╔══════════════════════════════════════════════════════════════════╗
║                    KEUOS PROJECT STATS                        ║
╠══════════════════════════════════════════════════════════════════╣
║  Commit:  $COMMIT_SHORT ($COMMIT_DATE)
║  Message: $COMMIT_MSG
║  Snapshot: $SNAPSHOT_DATE
╚══════════════════════════════════════════════════════════════════╝

━━━ Lines of Code by Language ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Rust .............. $(printf '%6s' "$LOC_RUST")   ($FILES_RUST files)
  Salt .............. $(printf '%6s' "$LOC_SALT")   ($FILES_SALT files)
  C / Headers ....... $(printf '%6s' "$LOC_C")
  Markdown .......... $(printf '%6s' "$LOC_MD")
  Python ............ $(printf '%6s' "$LOC_PYTHON")
  Shell ............. $(printf '%6s' "$LOC_SHELL")
  HTML .............. $(printf '%6s' "$LOC_HTML")
  Assembly (x86) .... $(printf '%6s' "$LOC_ASM")
  CSS ............... $(printf '%6s' "$LOC_CSS")
  JavaScript ........ $(printf '%6s' "$LOC_JS")
  TOML .............. $(printf '%6s' "$LOC_TOML")
  Linker scripts .... $(printf '%6s' "$LOC_LD")
  ─────────────────────────────
  TOTAL              $(printf '%6s' "$LOC_TOTAL")

━━━ Testing ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Rust #[test] fns .... $TEST_FNS
  Rust test modules ... $TEST_MODULES
  Rust test LOC ....... $TEST_RUST_LOC
  Salt test files ..... $TEST_SALT_FILES
  Salt test LOC ....... $TEST_SALT_LOC
  Total test LOC ...... $TEST_TOTAL_LOC

━━━ Compiler (salt-front) ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Total LOC ........... $COMPILER_LOC
  Codegen LOC ......... $CODEGEN_LOC  ($CODEGEN_FILES files)
  Type system LOC ..... $TYPE_LOC
  Structs ............. $COMPILER_STRUCTS
  Enums ............... $COMPILER_ENUMS
  MLIR ops emitted .... $MLIR_OPS
  Z3 references ....... $Z3_REFS
  unsafe usages ....... $UNSAFE_COUNT
  Compiler tests ...... $COMPILER_TEST_FNS

━━━ Salt Ecosystem ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Functions (fn) ...... $SALT_FNS
  Structs ............. $SALT_STRUCTS
  Contracts ........... $SALT_CONTRACTS
  Attributes .......... $SALT_ATTRS
  Stdlib modules ...... $SALT_STDLIB_MODULES  ($SALT_STDLIB_LOC LOC)
  Benchmarks .......... $SALT_BENCH_FILES files  ($SALT_BENCH_LOC LOC)
  Examples ............ $SALT_EXAMPLE_FILES files
  Kernel LOC .......... $SALT_KERNEL_LOC

━━━ Components ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Compiler ............ $COMPILER_LOC
  Tools (LSP, sp) ..... $TOOLS_LOC
  Salt programs ....... $LOC_SALT
  Kernel .............. $SALT_KERNEL_LOC
  LETTUCE (Redis) ..... $LETTUCE_LOC

━━━ Fun Facts ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Comment lines ....... $((COMMENT_RUST + COMMENT_SALT))  (Rust: $COMMENT_RUST, Salt: $COMMENT_SALT)
  TODO/FIXME/HACK ..... $TODO_COUNT
  Cargo dependencies .. $CARGO_DEPS
  Largest Rust file ... $LARGEST_RUST
  Largest Salt file ... $LARGEST_SALT

EOF
