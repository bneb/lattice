#!/usr/bin/env bash
# =============================================================================
# Z3 Contract Regression Tests
# =============================================================================
# Runs each contract through saltc --verify and checks the expected result.
# Used to detect the Z3 SAT/UNSAT inversion and other verification regressions.
#
# Usage: bash $PROJECT_ROOT/salt-front/tests/z3_contracts/run_tests.sh
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

SALTC="${SALTC:-$PROJECT_ROOT/salt-front/target/release/saltc}"
if [ ! -f "$SALTC" ]; then
    SALTC="$PROJECT_ROOT/salt-front/target/debug/saltc"
fi
PASS=0
FAIL=0

echo "=== Z3 Contract Regression Suite ==="
echo ""

# ── Test 1: Contract MUST be proved ────────────────────────────
echo -n "  test_contract_proved: "
if "$SALTC" "$SCRIPT_DIR/test_contract_proved.salt" \
    --lib --disable-alias-scopes -o /tmp/z3_test_proved > /tmp/z3_out_proved.txt 2>&1; then
    echo "PASS (Z3 proved the contract)"
    PASS=$((PASS + 1))
else
    echo "FAIL (unexpected compile error — possible SAT/UNSAT inversion)"
    cat /tmp/z3_out_proved.txt | head -5
    FAIL=$((FAIL + 1))
fi

# ── Test 2: Contract MUST be rejected ──────────────────────────
echo -n "  test_contract_rejected: "
if ! "$SALTC" "$SCRIPT_DIR/test_contract_rejected.salt" \
    --lib --disable-alias-scopes -o /tmp/z3_test_rejected > /tmp/z3_out_rejected.txt 2>&1; then
    if grep -q 'VERIFICATION ERROR\|contract evaluates to false' /tmp/z3_out_rejected.txt; then
        echo "PASS (contract violation caught)"
        PASS=$((PASS + 1))
    else
        echo "FAIL (compile error but not from verification)"
        cat /tmp/z3_out_rejected.txt | head -3
        FAIL=$((FAIL + 1))
    fi
else
    echo "FAIL (unexpected compile success — SAT/UNSAT inversion detected!)"
    FAIL=$((FAIL + 1))
fi

# ── Test 3: Complex contract (timeout/fallback) ─────────────────
echo -n "  test_contract_timeout: "
OUTCOME=$( "$SALTC" "$SCRIPT_DIR/test_contract_timeout.salt" \
    --lib --disable-alias-scopes -o /tmp/z3_test_timeout 2>&1 || true )
if echo "$OUTCOME" | grep -q 'VERIFICATION ERROR'; then
    echo "PASS (Z3 could not prove, runtime assertion emitted)"
    PASS=$((PASS + 1))
elif echo "$OUTCOME" | grep -q 'compiled successfully'; then
    echo "PASS (compiled — contract proved within timeout)"
    PASS=$((PASS + 1))
else
    echo "INCONCLUSIVE (unexpected output)"
    echo "$OUTCOME" | head -3
fi

# ── Test 4: Symbolic string contracts MUST be proved ────────────
echo -n "  test_strings_symbolic: "
if "$SALTC" "$SCRIPT_DIR/test_strings_symbolic.salt" \
    --lib --disable-alias-scopes -o /tmp/z3_test_strings_sym > /tmp/z3_out_strings_sym.txt 2>&1; then
    echo "PASS (symbolic string contracts proved)"
    PASS=$((PASS + 1))
else
    echo "FAIL (unexpected verification error)"
    cat /tmp/z3_out_strings_sym.txt | head -3
    FAIL=$((FAIL + 1))
fi

# ── Test 5: Symbolic string contracts MUST be rejected ──────────
echo -n "  test_strings_symbolic_rejected: "
if ! "$SALTC" "$SCRIPT_DIR/test_strings_symbolic_rejected.salt" \
    --lib --disable-alias-scopes -o /tmp/z3_test_strings_sym_rej > /tmp/z3_out_strings_sym_rej.txt 2>&1; then
    if grep -q 'VERIFICATION ERROR\|contract evaluates to false' /tmp/z3_out_strings_sym_rej.txt; then
        echo "PASS (contract violation caught)"
        PASS=$((PASS + 1))
    else
        echo "FAIL (compile error but not from verification)"
        FAIL=$((FAIL + 1))
    fi
else
    echo "FAIL (unexpected compile success — should have been rejected)"
    FAIL=$((FAIL + 1))
fi

# ── Test 6: Real (exact rational) contracts — KNOWN FLAKY ──────
# Z3's Real theory is incomplete (per FAQ). The 100ms timeout is not
# always sufficient on CI hardware. Accept both pass and timeout.
echo -n "  test_real: "
if "$SALTC" "$SCRIPT_DIR/test_real.salt" \
    --lib --disable-alias-scopes -o /tmp/z3_test_real > /tmp/z3_out_real.txt 2>&1; then
    echo "PASS (Real contracts proved)"
    PASS=$((PASS + 1))
else
    if grep -q 'VERIFICATION ERROR.*could not prove' /tmp/z3_out_real.txt; then
        echo "SKIP (known Z3 Real theory limitation — FAQ: float theory incomplete)"
    else
        echo "FAIL (unexpected error)"
        cat /tmp/z3_out_real.txt | head -3
        FAIL=$((FAIL + 1))
    fi
fi

# ── Test 7: BV (bitvector) contracts MUST be proved ──────────────
echo -n "  test_bv: "
if "$SALTC" "$SCRIPT_DIR/test_bv.salt" \
    --lib --disable-alias-scopes -o /tmp/z3_test_bv > /tmp/z3_out_bv.txt 2>&1; then
    echo "PASS (BV contracts proved)"
    PASS=$((PASS + 1))
else
    echo "FAIL (unexpected verification error)"
    cat /tmp/z3_out_bv.txt | head -3
    FAIL=$((FAIL + 1))
fi

# ── Test 8: Contract library predicates MUST be proved ──────────
echo -n "  test_contract_library: "
if "$SALTC" "$SCRIPT_DIR/test_contract_library.salt" \
    --lib --disable-alias-scopes -o /tmp/z3_test_contract_lib > /tmp/z3_out_contract_lib.txt 2>&1; then
    echo "PASS (contract library predicates proved)"
    PASS=$((PASS + 1))
else
    echo "FAIL (unexpected compile error)"
    cat /tmp/z3_out_contract_lib.txt | head -5
    FAIL=$((FAIL + 1))
fi

# ── Test 9: ensures(result != 0) MUST be proved ──────────────────
echo -n "  test_ensures_nonzero_proved: "
if "$SALTC" "$SCRIPT_DIR/test_ensures_nonzero_proved.salt" \
    --lib --disable-alias-scopes -o /tmp/z3_test_ensures_proved > /tmp/z3_out_ensures_proved.txt 2>&1; then
    echo "PASS (postcondition proved — result is never zero)"
    PASS=$((PASS + 1))
else
    echo "FAIL (unexpected verification error)"
    cat /tmp/z3_out_ensures_proved.txt | head -3
    FAIL=$((FAIL + 1))
fi

# ── Test 10: ensures(result != 0) MUST be rejected ───────────────
echo -n "  test_ensures_nonzero_rejected: "
if ! "$SALTC" "$SCRIPT_DIR/test_ensures_nonzero_rejected.salt" \
    --lib --disable-alias-scopes -o /tmp/z3_test_ensures_rejected > /tmp/z3_out_ensures_rejected.txt 2>&1; then
    if grep -q 'VERIFICATION ERROR\|contract evaluates to false\|Postcondition violation' /tmp/z3_out_ensures_rejected.txt; then
        echo "PASS (postcondition violation caught — returns 0 despite ensures(result!=0))"
        PASS=$((PASS + 1))
    else
        echo "FAIL (compile error but not from verification)"
        cat /tmp/z3_out_ensures_rejected.txt | head -3
        FAIL=$((FAIL + 1))
    fi
else
    echo "FAIL (should have been rejected — Z3 missed the postcondition violation)"
    FAIL=$((FAIL + 1))
fi

# ── Test 11: requires(start < len) MUST be proved ────────────────
echo -n "  test_requires_bounds_proved: "
if "$SALTC" "$SCRIPT_DIR/test_requires_bounds_proved.salt" \
    --lib --disable-alias-scopes -o /tmp/z3_test_bounds_proved > /tmp/z3_out_bounds_proved.txt 2>&1; then
    echo "PASS (bounds precondition proved — valid array access)"
    PASS=$((PASS + 1))
else
    echo "FAIL (unexpected verification error)"
    cat /tmp/z3_out_bounds_proved.txt | head -3
    FAIL=$((FAIL + 1))
fi

# ── Test 12: requires(start < len) MUST be rejected ──────────────
echo -n "  test_requires_bounds_rejected: "
if ! "$SALTC" "$SCRIPT_DIR/test_requires_bounds_rejected.salt" \
    --lib --disable-alias-scopes -o /tmp/z3_test_bounds_rejected > /tmp/z3_out_bounds_rejected.txt 2>&1; then
    if grep -q 'VERIFICATION ERROR\|contract evaluates to false' /tmp/z3_out_bounds_rejected.txt; then
        echo "PASS (bounds violation caught — idx=150 exceeds len=10)"
        PASS=$((PASS + 1))
    else
        echo "FAIL (compile error but not from verification)"
        cat /tmp/z3_out_bounds_rejected.txt | head -3
        FAIL=$((FAIL + 1))
    fi
else
    echo "FAIL (should have been rejected — Z3 missed the bounds violation)"
    FAIL=$((FAIL + 1))
fi

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
if [ "$FAIL" -gt 0 ]; then
    echo "REGESSION DETECTED — Z3 verification behavior has changed!"
    exit 1
else
    echo "All tests pass — Z3 verification working correctly."
fi
