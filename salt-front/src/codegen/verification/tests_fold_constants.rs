// Tests for fold_constants — compile-time contract evaluation

#[cfg(test)]
mod tests {
    use super::super::fold_constants;
    use std::collections::HashMap;

    fn empty_lengths() -> HashMap<String, i64> { HashMap::new() }
    fn empty_params() -> Vec<String> { vec![] }
    fn empty_args() -> Vec<syn::Expr> { vec![] }

    fn parse_expr(s: &str) -> syn::Expr {
        syn::parse_str(s).expect("failed to parse test expression")
    }

    #[test]
    fn test_int_literal_folds_to_self() {
        let expr = parse_expr("42");
        let result = fold_constants::try_eval(
            &expr, &empty_lengths(), &empty_params(), &empty_args(),
        );
        assert!(result.is_some());
        assert_eq!(result.unwrap(), crate::evaluator::ConstValue::Integer(42));
    }

    #[test]
    fn test_string_length_literal() {
        // "hello".length() → 5
        let expr = parse_expr("\"hello\".length()");
        let result = fold_constants::try_eval(
            &expr, &empty_lengths(), &empty_params(), &empty_args(),
        );
        assert!(result.is_some());
        assert_eq!(result.unwrap(), crate::evaluator::ConstValue::Integer(5));
    }

    #[test]
    fn test_string_length_known_param() {
        // key.length() where key has known length 5 (e.g., arg was "hello")
        let expr = parse_expr("key.length()");
        let mut lengths = HashMap::new();
        lengths.insert("key".to_string(), 5);
        let result = fold_constants::try_eval(
            &expr, &lengths, &empty_params(), &empty_args(),
        );
        assert!(result.is_some());
        assert_eq!(result.unwrap(), crate::evaluator::ConstValue::Integer(5));
    }

    #[test]
    fn test_string_starts_with_true() {
        // "hello".starts_with("hel") → true
        let expr = parse_expr("\"hello\".starts_with(\"hel\")");
        let result = fold_constants::try_eval(
            &expr, &empty_lengths(), &empty_params(), &empty_args(),
        );
        assert!(result.is_some());
        assert_eq!(result.unwrap(), crate::evaluator::ConstValue::Bool(true));
    }

    #[test]
    fn test_string_starts_with_false() {
        // "hello".starts_with("xyz") → false
        let expr = parse_expr("\"hello\".starts_with(\"xyz\")");
        let result = fold_constants::try_eval(
            &expr, &empty_lengths(), &empty_params(), &empty_args(),
        );
        assert!(result.is_some());
        assert_eq!(result.unwrap(), crate::evaluator::ConstValue::Bool(false));
    }

    #[test]
    fn test_string_ends_with_true() {
        let expr = parse_expr("\"program.salt\".ends_with(\".salt\")");
        let result = fold_constants::try_eval(
            &expr, &empty_lengths(), &empty_params(), &empty_args(),
        );
        assert!(result.is_some());
        assert_eq!(result.unwrap(), crate::evaluator::ConstValue::Bool(true));
    }

    #[test]
    fn test_string_contains_true() {
        let expr = parse_expr("\"hello world\".contains(\"lo w\")");
        let result = fold_constants::try_eval(
            &expr, &empty_lengths(), &empty_params(), &empty_args(),
        );
        assert!(result.is_some());
        assert_eq!(result.unwrap(), crate::evaluator::ConstValue::Bool(true));
    }

    #[test]
    fn test_string_contains_false() {
        let expr = parse_expr("\"hello\".contains(\"xyz\")");
        let result = fold_constants::try_eval(
            &expr, &empty_lengths(), &empty_params(), &empty_args(),
        );
        assert!(result.is_some());
        assert_eq!(result.unwrap(), crate::evaluator::ConstValue::Bool(false));
    }

    #[test]
    fn test_param_substitution_string_op() {
        // Simulates: fn f(key: StringView) requires(key.starts_with("salt-"))
        // Called as: f("salt-lang")
        let requires_expr = parse_expr("key.starts_with(\"salt-\")");
        let params = vec!["key".to_string()];
        let args = vec![parse_expr("\"salt-lang\"")];
        let result = fold_constants::try_eval(
            &requires_expr, &empty_lengths(), &params, &args,
        );
        assert!(result.is_some());
        assert_eq!(result.unwrap(), crate::evaluator::ConstValue::Bool(true));
    }

    #[test]
    fn test_param_substitution_false_case() {
        // f("wrong-key") where requires(key.starts_with("salt-"))
        let requires_expr = parse_expr("key.starts_with(\"salt-\")");
        let params = vec!["key".to_string()];
        let args = vec![parse_expr("\"wrong-key\"")];
        let result = fold_constants::try_eval(
            &requires_expr, &empty_lengths(), &params, &args,
        );
        assert!(result.is_some());
        assert_eq!(result.unwrap(), crate::evaluator::ConstValue::Bool(false));
    }

    #[test]
    fn test_param_substitution_int() {
        // f(42) where requires(x > 0) with known length for x
        let requires_expr = parse_expr("x > 0");
        let params = vec!["x".to_string()];
        let args = vec![parse_expr("42")];
        let mut lengths = HashMap::new();
        lengths.insert("x".to_string(), 5);
        let result = fold_constants::try_eval(
            &requires_expr, &lengths, &params, &args,
        );
        assert!(result.is_some());
        assert_eq!(result.unwrap(), crate::evaluator::ConstValue::Bool(true));
    }

    #[test]
    fn test_compound_comparison_with_substitution() {
        // requires(key.length() > 0) called with "hello"
        let requires_expr = parse_expr("key.length() > 0");
        let params = vec!["key".to_string()];
        let args = vec![parse_expr("\"hello\"")];
        let result = fold_constants::try_eval(
            &requires_expr, &empty_lengths(), &params, &args,
        );
        assert!(result.is_some());
        assert_eq!(result.unwrap(), crate::evaluator::ConstValue::Bool(true));
    }

    #[test]
    fn test_symbolic_param_returns_none() {
        // f(x) where x is a runtime variable, requires(x > 0)
        // No substitution possible — should return None (fall through to Z3)
        let requires_expr = parse_expr("x > 0");
        let params = vec!["x".to_string()];
        let args = vec![parse_expr("x")];  // variable, not literal
        let result = fold_constants::try_eval(
            &requires_expr, &empty_lengths(), &params, &args,
        );
        // Should be None — symbolic, can't evaluate at compile time
        assert!(result.is_none());
    }
}
