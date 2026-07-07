// Z3 Quantifier Tests — proves forall/exists work through the Z3 bridge.

#[cfg(test)]
mod quantifier_tests {
    use saltc::z3_shim;
    use saltc::z3_shim::ast::Ast;

    fn p<'a>() -> &'a [&'a z3_shim::Pattern<'a>] { &[] }

    #[test]
    fn test_forall_trivial() {
        let cfg = z3_shim::Config::new();
        let ctx = z3_shim::Context::new(&cfg);
        let x = z3_shim::ast::Int::new_const(&ctx, "x");
        let body = x._eq(&x);
        let forall = z3_shim::ast::forall_const(&ctx, &[&x], p(), &body);
        let solver = z3_shim::Solver::new(&ctx);
        solver.assert(&forall);
        assert_eq!(solver.check(), z3_shim::SatResult::Sat);
    }

    #[test]
    fn test_exists_five() {
        let cfg = z3_shim::Config::new();
        let ctx = z3_shim::Context::new(&cfg);
        let x = z3_shim::ast::Int::new_const(&ctx, "x");
        let five = z3_shim::ast::Int::from_i64(&ctx, 5);
        let body = x._eq(&five);
        let exists = z3_shim::ast::exists_const(&ctx, &[&x], p(), &body);
        let solver = z3_shim::Solver::new(&ctx);
        solver.assert(&exists);
        assert_eq!(solver.check(), z3_shim::SatResult::Sat);
    }

    #[test]
    fn test_forall_positive_implies_ge_zero() {
        let cfg = z3_shim::Config::new();
        let ctx = z3_shim::Context::new(&cfg);
        let x = z3_shim::ast::Int::new_const(&ctx, "x");
        let zero = z3_shim::ast::Int::from_i64(&ctx, 0);
        let implies = x.gt(&zero).implies(&x.ge(&zero));
        let forall = z3_shim::ast::forall_const(&ctx, &[&x], p(), &implies);
        let solver = z3_shim::Solver::new(&ctx);
        solver.assert(&forall);
        assert_eq!(solver.check(), z3_shim::SatResult::Sat);
    }

    #[test]
    fn test_forall_commutative_add() {
        let cfg = z3_shim::Config::new();
        let ctx = z3_shim::Context::new(&cfg);
        let x = z3_shim::ast::Int::new_const(&ctx, "x");
        let y = z3_shim::ast::Int::new_const(&ctx, "y");
        let body = (&x + &y)._eq(&(&y + &x));
        let forall = z3_shim::ast::forall_const(&ctx, &[&x, &y], p(), &body);
        let solver = z3_shim::Solver::new(&ctx);
        solver.assert(&forall);
        assert_eq!(solver.check(), z3_shim::SatResult::Sat);
    }
}
