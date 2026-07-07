// Additional evaluator tests to boost coverage
#![allow(clippy::approx_constant)]
use saltc::evaluator::{Evaluator, ConstValue};

fn parse_expr(s: &str) -> syn::Expr {
    syn::parse_str(s).unwrap()
}

// =============================================================================
// Literal parsing coverage
// =============================================================================

#[test]
fn test_eval_float_literal() {
    let eval = Evaluator::new();
    assert_eq!(eval.eval_expr(&parse_expr("3.14")).unwrap(), ConstValue::Float(3.14));
}

#[test]
fn test_eval_string_literal() {
    let eval = Evaluator::new();
    assert_eq!(eval.eval_expr(&parse_expr("\"hello\"")).unwrap(), ConstValue::String("hello".to_string()));
}

#[test]
fn test_eval_bool_true() {
    let eval = Evaluator::new();
    assert_eq!(eval.eval_expr(&parse_expr("true")).unwrap(), ConstValue::Bool(true));
}

#[test]
fn test_eval_bool_false() {
    let eval = Evaluator::new();
    assert_eq!(eval.eval_expr(&parse_expr("false")).unwrap(), ConstValue::Bool(false));
}

// =============================================================================
// Binary operations coverage
// =============================================================================

#[test]
fn test_eval_float_ops() {
    let eval = Evaluator::new();
    assert_eq!(eval.eval_expr(&parse_expr("1.5 + 2.5")).unwrap(), ConstValue::Float(4.0));
    assert_eq!(eval.eval_expr(&parse_expr("5.0 - 2.0")).unwrap(), ConstValue::Float(3.0));
    assert_eq!(eval.eval_expr(&parse_expr("3.0 * 2.0")).unwrap(), ConstValue::Float(6.0));
    assert_eq!(eval.eval_expr(&parse_expr("10.0 / 4.0")).unwrap(), ConstValue::Float(2.5));
}

#[test]
fn test_eval_int_comparisons() {
    let eval = Evaluator::new();
    assert_eq!(eval.eval_expr(&parse_expr("5 == 5")).unwrap(), ConstValue::Bool(true));
    assert_eq!(eval.eval_expr(&parse_expr("5 != 3")).unwrap(), ConstValue::Bool(true));
    assert_eq!(eval.eval_expr(&parse_expr("5 < 10")).unwrap(), ConstValue::Bool(true));
    assert_eq!(eval.eval_expr(&parse_expr("5 <= 5")).unwrap(), ConstValue::Bool(true));
    assert_eq!(eval.eval_expr(&parse_expr("10 > 5")).unwrap(), ConstValue::Bool(true));
    assert_eq!(eval.eval_expr(&parse_expr("10 >= 10")).unwrap(), ConstValue::Bool(true));
}

#[test]
fn test_eval_bitwise_ops() {
    let eval = Evaluator::new();
    // BitOr
    assert_eq!(eval.eval_expr(&parse_expr("0xFF | 0x0F")).unwrap(), ConstValue::Integer(0xFF));
    // BitXor
    assert_eq!(eval.eval_expr(&parse_expr("0xFF ^ 0x0F")).unwrap(), ConstValue::Integer(0xF0));
    // Shl
    assert_eq!(eval.eval_expr(&parse_expr("1 << 4")).unwrap(), ConstValue::Integer(16));
    // Shr
    assert_eq!(eval.eval_expr(&parse_expr("16 >> 2")).unwrap(), ConstValue::Integer(4));
}

#[test]
fn test_eval_bool_ops() {
    let eval = Evaluator::new();
    assert_eq!(eval.eval_expr(&parse_expr("true && true")).unwrap(), ConstValue::Bool(true));
    assert_eq!(eval.eval_expr(&parse_expr("true && false")).unwrap(), ConstValue::Bool(false));
    assert_eq!(eval.eval_expr(&parse_expr("true || false")).unwrap(), ConstValue::Bool(true));
    assert_eq!(eval.eval_expr(&parse_expr("false || false")).unwrap(), ConstValue::Bool(false));
    assert_eq!(eval.eval_expr(&parse_expr("true == true")).unwrap(), ConstValue::Bool(true));
    assert_eq!(eval.eval_expr(&parse_expr("true != false")).unwrap(), ConstValue::Bool(true));
}

// =============================================================================
// Unary operations coverage
// =============================================================================

#[test]
fn test_eval_unary_neg_int() {
    let eval = Evaluator::new();
    assert_eq!(eval.eval_expr(&parse_expr("-5")).unwrap(), ConstValue::Integer(-5));
    assert_eq!(eval.eval_expr(&parse_expr("-100")).unwrap(), ConstValue::Integer(-100));
}

#[test]
fn test_eval_unary_neg_float() {
    let eval = Evaluator::new();
    assert_eq!(eval.eval_expr(&parse_expr("-3.14")).unwrap(), ConstValue::Float(-3.14));
}

#[test]
fn test_eval_unary_not_bool() {
    let eval = Evaluator::new();
    assert_eq!(eval.eval_expr(&parse_expr("!true")).unwrap(), ConstValue::Bool(false));
    assert_eq!(eval.eval_expr(&parse_expr("!false")).unwrap(), ConstValue::Bool(true));
}

// =============================================================================
// Parenthesized expressions
// =============================================================================

#[test]
fn test_eval_paren() {
    let eval = Evaluator::new();
    assert_eq!(eval.eval_expr(&parse_expr("(5 + 3) * 2")).unwrap(), ConstValue::Integer(16));
    assert_eq!(eval.eval_expr(&parse_expr("(((10)))")).unwrap(), ConstValue::Integer(10));
}

// =============================================================================
// Constant table lookup
// =============================================================================

#[test]
fn test_eval_const_lookup() {
    let mut eval = Evaluator::new();
    eval.constant_table.insert("MY_CONST".to_string(), ConstValue::Integer(42));
    
    assert_eq!(eval.eval_expr(&parse_expr("MY_CONST")).unwrap(), ConstValue::Integer(42));
    assert_eq!(eval.eval_expr(&parse_expr("MY_CONST + 8")).unwrap(), ConstValue::Integer(50));
}

#[test]
fn test_eval_unknown_const() {
    let eval = Evaluator::new();
    let result = eval.eval_expr(&parse_expr("UNKNOWN_CONST"));
    assert!(result.is_err());
}

// =============================================================================
// Error cases
// =============================================================================

#[test]
fn test_eval_unsupported_expr() {
    let eval = Evaluator::new();
    // Complex path - should fail
    let result = eval.eval_expr(&parse_expr("foo::bar"));
    assert!(result.is_err());
}

#[test]
fn test_eval_type_mismatch_binary() {
    let eval = Evaluator::new();
    // int + bool should fail
    assert!(eval.eval_expr(&parse_expr("5 + true")).is_err());
}

#[test]
fn test_eval_type_mismatch_unary() {
    let eval = Evaluator::new();
    // -true should fail
    assert!(eval.eval_expr(&parse_expr("-true")).is_err());
}

#[test]
fn test_eval_unsupported_bool_op() {
    let eval = Evaluator::new();
    // bool > bool not supported
    assert!(eval.eval_expr(&parse_expr("true > false")).is_err());
}

#[test]
fn test_eval_unsupported_float_op() {
    let eval = Evaluator::new();
    // float % float not supported
    assert!(eval.eval_expr(&parse_expr("5.0 % 2.0")).is_err());
}
