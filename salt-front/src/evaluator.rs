use syn::{Expr, BinOp, UnOp, Lit};
use std::collections::HashMap;
use crate::common::mangling::Mangler;

/// Represents a value computed during compilation.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstValue {
    Integer(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Array(Vec<ConstValue>),
    Complex, 
}

#[derive(Debug, Clone, PartialEq)]
pub enum EvalError {
    NonConstExpression(String),
    TypeMismatch(String),
    MathError(String),
    RecursionLimitExceeded,
    UnsupportedExpr(String),
}

pub struct Evaluator {
    /// Max depth to prevent infinite recursion during eval
    pub depth_limit: usize,
    /// Context for looking up other 'salt.constant' values
    pub constant_table: HashMap<String, ConstValue>,
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl Evaluator {
    pub fn new() -> Self {
        Self {
            depth_limit: 100,
            constant_table: HashMap::new(),
        }
    }

    pub fn eval_expr(&self, expr: &Expr) -> Result<ConstValue, EvalError> {
        self.eval_expr_depth(expr, 0)
    }

    fn eval_expr_depth(&self, expr: &Expr, depth: usize) -> Result<ConstValue, EvalError> {
        if depth > self.depth_limit {
            return Err(EvalError::RecursionLimitExceeded);
        }

        match expr {
            Expr::Struct(_) => Ok(ConstValue::Complex),
            Expr::Array(expr_array) => {
                let mut elements = Vec::new();
                for elem in &expr_array.elems {
                    elements.push(self.eval_expr_depth(elem, depth + 1)?);
                }
                Ok(ConstValue::Array(elements))
            }
            Expr::Repeat(expr_repeat) => {
                let elem = self.eval_expr_depth(&expr_repeat.expr, depth + 1)?;
                let len_val = self.eval_expr_depth(&expr_repeat.len, depth + 1)?;
                if let ConstValue::Integer(len) = len_val {
                    Ok(ConstValue::Array(vec![elem; len as usize]))
                } else {
                    Err(EvalError::TypeMismatch("Array length must be integer".to_string()))
                }
            }
            Expr::Lit(expr_lit) => self.eval_literal(&expr_lit.lit),
            Expr::Binary(expr_binary) => {
                let left = self.eval_expr_depth(&expr_binary.left, depth + 1)?;
                let right = self.eval_expr_depth(&expr_binary.right, depth + 1)?;
                self.compute_binary(&expr_binary.op, left, right)
            }
            Expr::Unary(expr_unary) => {
                let val = self.eval_expr_depth(&expr_unary.expr, depth + 1)?;
                self.compute_unary(&expr_unary.op, val)
            }
            Expr::Paren(expr_paren) => self.eval_expr_depth(&expr_paren.expr, depth + 1),
            Expr::Path(expr_path) => {
                let segments: Vec<String> = expr_path.path.segments.iter().map(|s| s.ident.to_string()).collect();
                let name = Mangler::mangle(&segments);
                if let Some(val) = self.constant_table.get(&name) {
                    Ok(val.clone())
                } else if segments.len() == 1 {
                     Err(EvalError::NonConstExpression(format!("'{}' is not a known constant", name)))
                } else {
                    // Try to resolve namespaced constant. 
                    // Note: This logic assumes constants are already mangled in the table.
                    // This matches the new emit_mlir logic.
                    Err(EvalError::NonConstExpression(format!("Namespaced constant '{}' not found", segments.join("."))))
                }
            }
            Expr::Let(expr_let) => self.eval_expr_depth(&expr_let.expr, depth + 1),
            _ => Err(EvalError::UnsupportedExpr("Expression type not supported in const eval".to_string())),
        }
    }

    fn eval_literal(&self, lit: &Lit) -> Result<ConstValue, EvalError> {
        match lit {
            Lit::Int(lit_int) => {
                let s = lit_int.to_string();
                let val = if s.trim_start().starts_with("0x") || s.trim_start().starts_with("0X") {
                     let clean = s.trim_start().split_at(2).1;
                     let hex_part: String = clean.chars()
                         .take_while(|c| c.is_ascii_hexdigit() || *c == '_')
                         .filter(|c| *c != '_')
                         .collect();
                     
                     u64::from_str_radix(&hex_part, 16)
                        .map(|u| u as i64)
                        .map_err(|e| EvalError::UnsupportedExpr(format!("Invalid hex literal: {} ({})", s, e)))?
                } else {
                     lit_int.base10_parse::<u64>()
                        .map(|u| u as i64)
                        .map_err(|_| EvalError::UnsupportedExpr("Invalid int literal".to_string()))?
                };
                Ok(ConstValue::Integer(val))
            },
            Lit::Float(lit_float) => Ok(ConstValue::Float(
                lit_float.base10_parse::<f64>().map_err(|_| EvalError::UnsupportedExpr("Invalid float literal".to_string()))?
            )),
            Lit::Bool(lit_bool) => Ok(ConstValue::Bool(lit_bool.value)),
            Lit::Str(lit_str) => Ok(ConstValue::String(lit_str.value())),
            _ => Err(EvalError::UnsupportedExpr("Literal type not supported".to_string())),
        }
    }

    fn compute_unary(&self, op: &UnOp, val: ConstValue) -> Result<ConstValue, EvalError> {
        match (op, val) {
            (UnOp::Neg(_), ConstValue::Integer(i)) => Ok(ConstValue::Integer(-i)),
            (UnOp::Neg(_), ConstValue::Float(f)) => Ok(ConstValue::Float(-f)),
            (UnOp::Not(_), ConstValue::Bool(b)) => Ok(ConstValue::Bool(!b)),
            (UnOp::Not(_), ConstValue::Integer(i)) => Ok(ConstValue::Integer(!i)),
            _ => Err(EvalError::TypeMismatch("Invalid unary operation".to_string())),
        }
    }

    fn compute_binary(&self, op: &BinOp, left: ConstValue, right: ConstValue) -> Result<ConstValue, EvalError> {
        match (left, right) {
            // Integer
            (ConstValue::Integer(l), ConstValue::Integer(r)) => match op {
                BinOp::Add(_) => Ok(ConstValue::Integer(l + r)),
                BinOp::Sub(_) => Ok(ConstValue::Integer(l - r)),
                BinOp::Mul(_) => Ok(ConstValue::Integer(l * r)),
                BinOp::Div(_) => Ok(ConstValue::Integer(l.checked_div(r).ok_or(EvalError::MathError("Division by zero".into()))?)),
                BinOp::Rem(_) => Ok(ConstValue::Integer(l.checked_rem(r).ok_or(EvalError::MathError("Division by zero".into()))?)),
                
                // Comparisons
                BinOp::Eq(_) => Ok(ConstValue::Bool(l == r)),
                BinOp::Ne(_) => Ok(ConstValue::Bool(l != r)),
                BinOp::Lt(_) => Ok(ConstValue::Bool(l < r)),
                BinOp::Le(_) => Ok(ConstValue::Bool(l <= r)),
                BinOp::Gt(_) => Ok(ConstValue::Bool(l > r)),
                BinOp::Ge(_) => Ok(ConstValue::Bool(l >= r)),
                
                // Bitwise
                BinOp::BitAnd(_) => Ok(ConstValue::Integer(l & r)),
                BinOp::BitOr(_) => Ok(ConstValue::Integer(l | r)),
                BinOp::BitXor(_) => Ok(ConstValue::Integer(l ^ r)),
                BinOp::Shl(_) => Ok(ConstValue::Integer(l << r)),
                BinOp::Shr(_) => Ok(ConstValue::Integer(l >> r)),
                
                _ => Err(EvalError::UnsupportedExpr("Operator not supported for Int".into())),
            },
            
            // Float
            (ConstValue::Float(l), ConstValue::Float(r)) => match op {
                BinOp::Add(_) => Ok(ConstValue::Float(l + r)),
                BinOp::Sub(_) => Ok(ConstValue::Float(l - r)),
                BinOp::Mul(_) => Ok(ConstValue::Float(l * r)),
                BinOp::Div(_) => Ok(ConstValue::Float(l / r)),
                _ => Err(EvalError::UnsupportedExpr("Operator not supported for Float".into())),
            },

            // Bool
            (ConstValue::Bool(l), ConstValue::Bool(r)) => match op {
                BinOp::And(_) => Ok(ConstValue::Bool(l && r)),
                BinOp::Or(_) => Ok(ConstValue::Bool(l || r)),
                BinOp::Eq(_) => Ok(ConstValue::Bool(l == r)),
                BinOp::Ne(_) => Ok(ConstValue::Bool(l != r)),
                _ => Err(EvalError::UnsupportedExpr("Operator not supported for Bool".into())),
            },

            _ => Err(EvalError::TypeMismatch("Binary operation type mismatch".to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(s: &str) -> Result<ConstValue, EvalError> {
        let expr: Expr = syn::parse_str(s).expect("valid expr");
        Evaluator::new().eval_expr(&expr)
    }

    #[test] fn test_int_literal() { assert_eq!(eval("42"), Ok(ConstValue::Integer(42))); }
    #[test] fn test_hex_literal() { assert_eq!(eval("0xFF"), Ok(ConstValue::Integer(255))); }
    #[test] fn test_float_literal() { assert_eq!(eval("3.14"), Ok(ConstValue::Float(3.14))); }
    #[test] fn test_bool_true() { assert_eq!(eval("true"), Ok(ConstValue::Bool(true))); }
    #[test] fn test_bool_false() { assert_eq!(eval("false"), Ok(ConstValue::Bool(false))); }
    #[test] fn test_neg_int() { assert_eq!(eval("-5"), Ok(ConstValue::Integer(-5))); }
    #[test] fn test_neg_float() { assert_eq!(eval("-2.5"), Ok(ConstValue::Float(-2.5))); }
    #[test] fn test_not_bool() { assert_eq!(eval("!true"), Ok(ConstValue::Bool(false))); }
    #[test] fn test_not_int() { assert_eq!(eval("!0"), Ok(ConstValue::Integer(-1))); }

    #[test] fn test_add() { assert_eq!(eval("2 + 3"), Ok(ConstValue::Integer(5))); }
    #[test] fn test_sub() { assert_eq!(eval("10 - 3"), Ok(ConstValue::Integer(7))); }
    #[test] fn test_mul() { assert_eq!(eval("4 * 7"), Ok(ConstValue::Integer(28))); }
    #[test] fn test_div() { assert_eq!(eval("15 / 3"), Ok(ConstValue::Integer(5))); }
    #[test] fn test_rem() { assert_eq!(eval("17 % 5"), Ok(ConstValue::Integer(2))); }
    #[test] fn test_div_by_zero() { assert!(eval("5 / 0").is_err()); }
    #[test] fn test_rem_by_zero() { assert!(eval("5 % 0").is_err()); }
    #[test] fn test_eq_int() { assert_eq!(eval("5 == 5"), Ok(ConstValue::Bool(true))); }
    #[test] fn test_lt() { assert_eq!(eval("3 < 5"), Ok(ConstValue::Bool(true))); }
    #[test] fn test_gt() { assert_eq!(eval("7 > 5"), Ok(ConstValue::Bool(true))); }
    #[test] fn test_bit_and() { assert_eq!(eval("0xFF & 0x0F"), Ok(ConstValue::Integer(15))); }
    #[test] fn test_shl() { assert_eq!(eval("1 << 4"), Ok(ConstValue::Integer(16))); }
    #[test] fn test_shr() { assert_eq!(eval("16 >> 2"), Ok(ConstValue::Integer(4))); }
    #[test] fn test_float_add() { assert_eq!(eval("2.0 + 3.0"), Ok(ConstValue::Float(5.0))); }
    #[test] fn test_bool_or() { assert_eq!(eval("true || false"), Ok(ConstValue::Bool(true))); }
    #[test] fn test_paren() { assert_eq!(eval("(2 + 3) * 4"), Ok(ConstValue::Integer(20))); }

    #[test] fn test_constant_table() {
        let mut e = Evaluator::new();
        e.constant_table.insert("PI".into(), ConstValue::Float(3.14));
        let expr: Expr = syn::parse_str("PI").expect("valid");
        assert_eq!(e.eval_expr(&expr), Ok(ConstValue::Float(3.14)));
    }

    #[test] fn test_unknown_constant() {
        let expr: Expr = syn::parse_str("UNDEFINED").expect("valid");
        assert!(Evaluator::new().eval_expr(&expr).is_err());
    }

    #[test] fn test_recursion_limit() {
        let mut e = Evaluator::new();
        e.depth_limit = 0;
        let expr: Expr = syn::parse_str("1 + 1").expect("valid");
        assert!(matches!(e.eval_expr(&expr), Err(EvalError::RecursionLimitExceeded)));
    }
}
