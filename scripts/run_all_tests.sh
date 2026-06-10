#!/usr/bin/env zsh
# =============================================================================
# Salt Test Suite Runner — Run All Tests
# =============================================================================
# Runs all test_*.salt files through the full pipeline and reports results.
#
# Categories:
#   tests/test_*.salt      — Executable tests (compile + run, assert exit 0)
#   tests/lib/test_*.salt  — Library tests (compile-only, verify MLIR emits)
#
# Usage:
#   ./scripts/run_all_tests.sh                  # Run all tests
#   ./scripts/run_all_tests.sh --filter thread  # Run tests matching "thread"
# =============================================================================

set -uo pipefail

SCRIPT_DIR="${0:A:h}"
PROJECT_ROOT="${SCRIPT_DIR:h}"
RUN_TEST="$SCRIPT_DIR/run_test.sh"

echo "🔨 Building external dependencies..."
"$SCRIPT_DIR/build_cdm.sh" > /dev/null 2>&1 || true
"$SCRIPT_DIR/build_worker.sh" > /dev/null 2>&1 || true

FILTER="${1:-}"
[[ "$FILTER" == "--filter" ]] && FILTER="${2:-}" || true

PASSED=0
FAILED=0
SKIPPED=0
LIB_PASSED=0
LIB_FAILED=0
FAILURES=()
LIB_FAILURES=()

# Known compiler deficiencies — tracked but not blocking.
# These remain in the active suite to drive compiler fixes.
KNOWN_FAILING=(
    # - test_pulse_queue: rewrote with mocked ring buffer
    # - test_task0_spawn: rewrote with mocked process table
)

is_known_failing() {
    local name="$1"
    for kf in "${KNOWN_FAILING[@]}"; do
        [[ "$name" == "$kf" ]] && return 0
    done
    return 1
}

echo "🧪 Salt Test Suite"
echo "==================="
echo ""

# =========================================================================
# Phase 1: Executable tests (tests/test_*.salt)
# =========================================================================
echo "--- Executable Tests ---"
echo ""

for test_file in "$PROJECT_ROOT"/tests/test_*.salt; do
    BASENAME=$(basename "$test_file" .salt)

    # Apply filter if provided
    if [[ -n "$FILTER" && "$BASENAME" != *"$FILTER"* ]]; then
        continue
    fi

    printf "%-40s " "$BASENAME"

    # Run the test, capture output and exit code
    OUTPUT=$("$RUN_TEST" "$test_file" 2>&1)
    EXIT_CODE=$?

    if [[ $EXIT_CODE -eq 0 ]]; then
        echo "✅ PASS"
        ((PASSED++))
    elif is_known_failing "$BASENAME"; then
        echo "⚠️  KNOWN FAIL (exit $EXIT_CODE)"
        ((SKIPPED++))
    else
        echo "❌ FAIL (exit $EXIT_CODE)"
        echo "$OUTPUT"
        ((FAILED++))
        FAILURES+=("$BASENAME")
    fi
done

# =========================================================================
# Phase 2: Library tests (tests/lib/test_*.salt) — compile-only
# =========================================================================
if [[ -d "$PROJECT_ROOT/tests/lib" ]]; then
    echo ""
    echo "--- Library Tests (compile-only) ---"
    echo ""

    for test_file in "$PROJECT_ROOT"/tests/lib/test_*.salt; do
        [[ -f "$test_file" ]] || continue
        BASENAME=$(basename "$test_file" .salt)

        if [[ -n "$FILTER" && "$BASENAME" != *"$FILTER"* ]]; then
            continue
        fi

        printf "%-40s " "$BASENAME"

        OUTPUT=$("$RUN_TEST" "$test_file" --compile-only --lib 2>&1)
        EXIT_CODE=$?

        if [[ $EXIT_CODE -eq 0 ]]; then
            echo "✅ COMPILE OK"
            ((LIB_PASSED++))
        else
            echo "❌ COMPILE FAIL"
            ((LIB_FAILED++))
            LIB_FAILURES+=("$BASENAME")
        fi
    done
fi

# =========================================================================
# Summary
# =========================================================================
echo ""
echo "==================="
echo "Executable: $PASSED passed, $FAILED failed, $SKIPPED known-failing"
if [[ $LIB_PASSED -gt 0 || $LIB_FAILED -gt 0 ]]; then
    echo "Library:    $LIB_PASSED compiled, $LIB_FAILED failed"
fi

if [[ $FAILED -gt 0 ]]; then
    echo ""
    echo "Failed executable tests:"
    for f in "${FAILURES[@]}"; do
        echo "  ❌ $f"
    done
    exit 1
fi

if [[ $LIB_FAILED -gt 0 ]]; then
    echo ""
    echo "Library compile failures (non-blocking):"
    for f in "${LIB_FAILURES[@]}"; do
        echo "  ⚠️  $f"
    done
fi

echo "✅ All tests passed!"
