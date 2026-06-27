// Z3 translation for Real (exact rational) and BV (bitvector) types.
// Separate from memory.rs to keep file sizes under control.

use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use std::collections::HashMap;

/// Check if an expression has a float type (F32 or F64).
pub(crate) fn is_float_expr(expr: &syn::Expr, local_vars: &HashMap<String, (Type, LocalKind)>) -> bool {
    if matches!(expr, syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Float(_), .. })) {
        return true;
    }
    if let syn::Expr::Path(p) = expr {
        if let Some(ident) = p.path.get_ident() {
            if let Some((ty, _)) = local_vars.get(&ident.to_string()) {
                return ty.is_float();
            }
        }
    }
    false
}

/// Convert an f64 literal to exact Z3 rational (num/den strings).
fn float_to_rational(val: f64) -> (String, String) {
    let s = format!("{}", val);
    if let Some(dot) = s.find('.') {
        let int_part = &s[..dot];
        let frac_part = &s[dot + 1..];
        let frac_len = frac_part.len() as u32;
        let num: String = if int_part == "0" || int_part == "-0" {
            format!("{}{}", if s.starts_with('-') { "-" } else { "" }, frac_part.trim_start_matches('0'))
        } else {
            format!("{}{}", int_part, frac_part)
        };
        let den = format!("1{}", "0".repeat(frac_len as usize));
        let num = if num.is_empty() || num == "-" { "0".to_string() } else { num };
        (num, den)
    } else {
        (s, "1".to_string())
    }
}

/// Translate a Salt expression to a Z3 Real (exact rational) value.
#[allow(clippy::only_used_in_recursion)]
pub fn translate_real_to_z3<'a, 'ctx>(
    ctx: &mut LoweringContext<'a, 'ctx>,
    expr: &syn::Expr,
    local_vars: &HashMap<String, (Type, LocalKind)>,
) -> Result<crate::z3_shim::ast::Real<'a>, String> {
    match expr {
        syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Float(lf), .. }) => {
            let val = lf.base10_parse::<f64>().map_err(|e| e.to_string())?;
            let (num, den) = float_to_rational(val);
            crate::z3_shim::ast::Real::from_real_str(ctx.z3_ctx, &num, &den)
                .ok_or_else(|| format!("invalid real literal: {}/{}", num, den))
        }
        syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(li), .. }) => {
            let val = li.base10_parse::<i64>().map_err(|e| e.to_string())?;
            let int_val = crate::z3_shim::ast::Int::from_i64(ctx.z3_ctx, val);
            Ok(crate::z3_shim::ast::Real::from_int(&int_val))
        }
        syn::Expr::Path(p) => {
            let name = p.path.segments.last()
                .ok_or_else(|| "Empty path in real context".to_string())?
                .ident.to_string();
            Ok(crate::z3_shim::ast::Real::new_const(ctx.z3_ctx, name))
        }
        syn::Expr::Binary(b) => {
            let lhs = translate_real_to_z3(ctx, &b.left, local_vars)?;
            let rhs = translate_real_to_z3(ctx, &b.right, local_vars)?;
            match b.op {
                syn::BinOp::Add(_) => Ok(lhs + rhs),
                syn::BinOp::Sub(_) => Ok(lhs - rhs),
                syn::BinOp::Mul(_) => Ok(lhs * rhs),
                syn::BinOp::Div(_) => Ok(lhs / rhs),
                _ => Err(format!("unsupported real operator: {:?}", b.op)),
            }
        }
        syn::Expr::Paren(p) => translate_real_to_z3(ctx, &p.expr, local_vars),
        syn::Expr::Group(g) => translate_real_to_z3(ctx, &g.expr, local_vars),
        syn::Expr::Cast(c) => translate_real_to_z3(ctx, &c.expr, local_vars),
        syn::Expr::Unary(u) => {
            match u.op {
                syn::UnOp::Neg(_) => {
                    let inner = translate_real_to_z3(ctx, &u.expr, local_vars)?;
                    Ok(-inner)
                }
                _ => Err(format!("unsupported real unary op: {:?}", u.op)),
            }
        }
        _ => Err(format!("cannot translate {:?} to Z3 Real", expr)),
    }
}

/// Apply a bitwise binary op by converting Int operands to BV, operating, then back to Int.
pub(crate) fn translate_bitwise_op<'a, 'ctx>(
    _ctx: &mut LoweringContext<'a, 'ctx>,
    lhs: &crate::z3_shim::ast::Int<'a>,
    rhs: &crate::z3_shim::ast::Int<'a>,
    op: &syn::BinOp,
) -> Result<crate::z3_shim::ast::Int<'a>, String> {
    let w = 64;
    let l = crate::z3_shim::ast::BV::from_int(lhs, w);
    let r = crate::z3_shim::ast::BV::from_int(rhs, w);
    match op {
        syn::BinOp::BitAnd(_) => Ok(l.bvand(&r).to_int(true)),
        syn::BinOp::BitOr(_) => Ok(l.bvor(&r).to_int(true)),
        syn::BinOp::BitXor(_) => Ok(l.bvxor(&r).to_int(true)),
        syn::BinOp::Shl(_) => Ok(l.bvshl(&r).to_int(true)),
        syn::BinOp::Shr(_) => Ok(l.bvashr(&r).to_int(true)),
        _ => Err(format!("not a bitwise op: {:?}", op)),
    }
}
