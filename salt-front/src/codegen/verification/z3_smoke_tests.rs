#[allow(unused_imports)]
use crate::z3_shim::ast::Ast;

/// Test that a contradiction (x > 0 AND x < 0) is UNSAT
#[test]
fn test_trivially_safe_contradiction() {
    let z3_cfg = crate::z3_shim::Config::new();
    let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);

    let x = crate::z3_shim::ast::Int::new_const(&z3_ctx, "x");
    let zero = crate::z3_shim::ast::Int::from_i64(&z3_ctx, 0);

    let gt_zero = x.gt(&zero);
    let lt_zero = x.lt(&zero);
    let contradiction = crate::z3_shim::ast::Bool::and(&z3_ctx, &[&gt_zero, &lt_zero]);

    let solver = crate::z3_shim::Solver::new(&z3_ctx);
    solver.assert(&contradiction);
    assert_eq!(solver.check(), crate::z3_shim::SatResult::Unsat,
        "Contradiction should be unsatisfiable");
}

/// Test that x > 5 is SAT
#[test]
fn test_satisfiable_violation_returns_false() {
    let z3_cfg = crate::z3_shim::Config::new();
    let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);

    let x = crate::z3_shim::ast::Int::new_const(&z3_ctx, "x");
    let five = crate::z3_shim::ast::Int::from_i64(&z3_ctx, 5);
    let gt_five = x.gt(&five);

    let solver = crate::z3_shim::Solver::new(&z3_ctx);
    solver.assert(&gt_five);
    assert_eq!(solver.check(), crate::z3_shim::SatResult::Sat,
        "x > 5 should be satisfiable");
}

/// Test that always-false is UNSAT
#[test]
fn test_always_false_is_unsat() {
    let z3_cfg = crate::z3_shim::Config::new();
    let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);

    let always_false = crate::z3_shim::ast::Bool::from_bool(&z3_ctx, false);

    let solver = crate::z3_shim::Solver::new(&z3_ctx);
    solver.assert(&always_false);
    assert_eq!(solver.check(), crate::z3_shim::SatResult::Unsat,
        "Always-false should be unsatisfiable");
}

/// Test that always-true is SAT
#[test]
fn test_always_true_is_sat() {
    let z3_cfg = crate::z3_shim::Config::new();
    let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);

    let always_true = crate::z3_shim::ast::Bool::from_bool(&z3_ctx, true);

    let solver = crate::z3_shim::Solver::new(&z3_ctx);
    solver.assert(&always_true);
    assert_eq!(solver.check(), crate::z3_shim::SatResult::Sat,
        "Always-true should be satisfiable");
}

/// Test bounds check: i < len where i in [0, 10) and len = 10
#[test]
fn test_bounds_check_provable() {
    let z3_cfg = crate::z3_shim::Config::new();
    let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);

    let i = crate::z3_shim::ast::Int::new_const(&z3_ctx, "i");
    let len = crate::z3_shim::ast::Int::from_i64(&z3_ctx, 10);
    let zero = crate::z3_shim::ast::Int::from_i64(&z3_ctx, 0);

    let i_ge_0 = i.ge(&zero);
    let i_lt_10 = i.lt(&len);
    let violation = i.ge(&len);

    let solver = crate::z3_shim::Solver::new(&z3_ctx);
    solver.assert(&i_ge_0);
    solver.assert(&i_lt_10);
    solver.assert(&violation);

    assert_eq!(solver.check(), crate::z3_shim::SatResult::Unsat,
        "With i in [0, 10), violation i >= 10 should be unsatisfiable");
}
