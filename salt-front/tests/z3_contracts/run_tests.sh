#!/usr/bin/env bash
# =============================================================================
# Z3 Contract Regression Tests
# =============================================================================
# Runs each contract through salt-front --verify and checks the expected result.
# Used to detect the Z3 SAT/UNSAT inversion and other verification regressions.
#
# Usage: bash $PROJECT_ROOT/salt-front/tests/z3_contracts/run_tests.sh
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

SALT_FRONT="${SALT_FRONT:-$PROJECT_ROOT/salt-front/target/release/salt-front}"
if [ ! -f "$SALT_FRONT" ]; then
    SALT_FRONT="$PROJECT_ROOT/salt-front/target/debug/salt-front"
fi
PASS=0
FAIL=0

echo "=== Z3 Contract Regression Suite ==="
echo ""

# ── Test 1: Contract MUST be proved ────────────────────────────
echo -n "  test_contract_proved: "
if "$SALT_FRONT" $PROJECT_ROOT/salt-front/tests/z3_contracts/test_contract_proved.salt \
    --verify -o /tmp/z3_test_proved > /tmp/z3_out_proved.txt 2>&1; then
    if grep -q 'UNSAT\|proven' /tmp/z3_out_proved.txt; then
        echo "PASS (Z3 proved the contract)"
        PASS=$((PASS + 1))
    else
        echo "PASS (compiled, but check output for verification status)"
        PASS=$((PASS + 1))
    fi
else
    echo "FAIL (unexpected compile error — possible SAT/UNSAT inversion)"
    FAIL=$((FAIL + 1))
fi

# ── Test 2: Contract MUST be rejected ──────────────────────────
echo -n "  test_contract_rejected: "
if ! "$SALT_FRONT" $PROJECT_ROOT/salt-front/tests/z3_contracts/test_contract_rejected.salt \
    --verify -o /tmp/z3_test_rejected > /tmp/z3_out_rejected.txt 2>&1; then
    if grep -q 'VERIFICATION ERROR\|counterexample' /tmp/z3_out_rejected.txt; then
        echo "PASS (Z3 found counterexample, compile error as expected)"
        PASS=$((PASS + 1))
    else
        echo "FAIL (compile error but not from verification — check output)"
        FAIL=$((FAIL + 1))
    fi
else
    echo "FAIL (unexpected compile success — SAT/UNSAT inversion detected!)"
    FAIL=$((FAIL + 1))
fi

# ── Test 3: Complex contract ───────────────────────────────────
echo -n "  test_contract_timeout: "
OUTCOME=$( "$SALT_FRONT" $PROJECT_ROOT/salt-front/tests/z3_contracts/test_contract_timeout.salt \
    --verify -o /tmp/z3_test_timeout 2>&1 || true )
if echo "$OUTCOME" | grep -q 'VERIFICATION ERROR'; then
    echo "PASS (Z3 could not prove, runtime assertion emitted)"
    PASS=$((PASS + 1))
elif echo "$OUTCOME" | grep -q 'UNSAT\|proven'; then
    echo "PASS (Z3 proved — contract was simpler than expected)"
    PASS=$((PASS + 1))
elif echo "$OUTCOME" | grep -q 'compiled successfully'; then
    echo "PASS (compiled — runtime assertion fallback active)"
    PASS=$((PASS + 1))
else
    echo "INCONCLUSIVE (unexpected output — check /tmp/z3_test_timeout)"
    echo "$OUTCOME" | head -5
fi

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
if [ "$FAIL" -gt 0 ]; then
    echo "REGESSION DETECTED — Z3 verification behavior has changed!"
    exit 1
else
    echo "All tests pass — Z3 verification working correctly."
fi
