// Smoke tests for the Z3 verification module
use saltc::z3_shim;

#[test]
fn test_z3_context_creation() {
    let cfg = z3_shim::Config::new();
    let ctx = z3_shim::Context::new(&cfg);
    let solver = z3_shim::Solver::new(&ctx);
    let x = z3_shim::ast::Int::from_i64(&ctx, 42);
    let zero = z3_shim::ast::Int::from_i64(&ctx, 0);
    solver.assert(&x.gt(&zero));
    assert_eq!(solver.check(), z3_shim::SatResult::Sat);
}

#[test]
fn test_unsat_contradiction() {
    let cfg = z3_shim::Config::new();
    let ctx = z3_shim::Context::new(&cfg);
    let solver = z3_shim::Solver::new(&ctx);
    let x = z3_shim::ast::Int::from_i64(&ctx, 1);
    let zero = z3_shim::ast::Int::from_i64(&ctx, 0);
    solver.assert(&x.lt(&zero));
    solver.assert(&x.gt(&zero));
    assert_eq!(solver.check(), z3_shim::SatResult::Unsat);
}
