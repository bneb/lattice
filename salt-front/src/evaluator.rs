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

#[derive(Debug, Clone)]
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
                BinOp::Div(_) => {
                    if r == 0 { Err(EvalError::MathError("Division by zero".into())) }
                    else { Ok(ConstValue::Integer(l / r)) }
                },
                BinOp::Rem(_) => {
                    if r == 0 { Err(EvalError::MathError("Division by zero".into())) }
                     else { Ok(ConstValue::Integer(l % r)) }
                },
                
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
