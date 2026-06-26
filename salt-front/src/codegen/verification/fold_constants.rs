// Constant folder for Z3 contract expressions.
//
// Before a requires/ensures expression is translated to Z3, this pass
// evaluates any sub-expression that can be resolved at compile time:
//   - String literal .length() → integer
//   - Arithmetic on constants → literal result
//   - String literal comparisons → boolean
//
// This runs in the verification engine, not the Z3 translation layer.
// Z3 sees only the simplified expression with constants folded.

use std::collections::HashMap;

/// Fold compile-time-known values in a requires/ensures expression.
/// `known_lengths` maps parameter names to their known string lengths
/// (when the argument was a string literal).
pub fn fold_constants(
    expr: &syn::Expr,
    known_lengths: &HashMap<String, i64>,
) -> syn::Expr {
    match expr {
        // .length() or .len() on a known parameter → literal int
        syn::Expr::MethodCall(mc) => {
            let method = mc.method.to_string();
            if method == "length" || method == "len" {
                if let syn::Expr::Path(p) = &*mc.receiver {
                    if let Some(ident) = p.path.get_ident() {
                        if let Some(len) = known_lengths.get(&ident.to_string()) {
                            return make_int_literal(*len);
                        }
                    }
                }
                // Also handle .length() on a string literal directly
                if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &*mc.receiver {
                    return make_int_literal(s.value().len() as i64);
                }
            }
            // Recurse into receiver and args
            let folded_receiver = Box::new(fold_constants(&mc.receiver, known_lengths));
            let folded_args: Vec<syn::Expr> = mc.args.iter()
                .map(|a| fold_constants(a, known_lengths))
                .collect();
            syn::Expr::MethodCall(syn::ExprMethodCall {
                attrs: mc.attrs.clone(),
                receiver: folded_receiver,
                dot_token: mc.dot_token,
                method: mc.method.clone(),
                turbofish: mc.turbofish.clone(),
                paren_token: mc.paren_token,
                args: syn::punctuated::Punctuated::from_iter(folded_args),
            })
        }

        // Binary operations: fold operands, then evaluate if both are literals
        syn::Expr::Binary(b) => {
            let left = fold_constants(&b.left, known_lengths);
            let right = fold_constants(&b.right, known_lengths);
            // Try to constant-fold the operation
            if let (Some(lv), Some(rv)) = (int_literal_value(&left), int_literal_value(&right)) {
                if let Some(result) = eval_binary_i64(&b.op, lv, rv) {
                    return make_bool_literal(result);
                }
                if let Some(result) = eval_binary_comparison(&b.op, lv, rv) {
                    return make_bool_literal(result);
                }
            }
            syn::Expr::Binary(syn::ExprBinary {
                attrs: b.attrs.clone(),
                left: Box::new(left),
                op: b.op,
                right: Box::new(right),
            })
        }

        // Paren/Group: transparent
        syn::Expr::Paren(p) => syn::Expr::Paren(syn::ExprParen {
            attrs: p.attrs.clone(),
            paren_token: p.paren_token,
            expr: Box::new(fold_constants(&p.expr, known_lengths)),
        }),
        syn::Expr::Group(g) => syn::Expr::Group(syn::ExprGroup {
            attrs: g.attrs.clone(),
            group_token: g.group_token,
            expr: Box::new(fold_constants(&g.expr, known_lengths)),
        }),

        // Unary: fold inner
        syn::Expr::Unary(u) => syn::Expr::Unary(syn::ExprUnary {
            attrs: u.attrs.clone(),
            op: u.op,
            expr: Box::new(fold_constants(&u.expr, known_lengths)),
        }),

        // Block: fold inner expression
        syn::Expr::Block(block) => {
            if let Some(syn::Stmt::Expr(inner, _semi)) = block.block.stmts.first() {
                let folded = fold_constants(inner, known_lengths);
                let mut new_block = block.clone();
                if let Some(syn::Stmt::Expr(_, semi)) = new_block.block.stmts.first_mut() {
                    *new_block.block.stmts.first_mut().unwrap() = syn::Stmt::Expr(folded, *semi);
                }
                syn::Expr::Block(new_block)
            } else {
                expr.clone()
            }
        }

        // Everything else: clone as-is
        _ => expr.clone(),
    }
}

/// Create an integer literal expression
fn make_int_literal(val: i64) -> syn::Expr {
    use syn::Lit;
    syn::Expr::Lit(syn::ExprLit {
        attrs: vec![],
        lit: Lit::Int(syn::LitInt::new(&val.to_string(), proc_macro2::Span::call_site())),
    })
}

/// Create a boolean literal expression
fn make_bool_literal(val: bool) -> syn::Expr {
    use syn::Lit;
    syn::Expr::Lit(syn::ExprLit {
        attrs: vec![],
        lit: Lit::Bool(syn::LitBool::new(val, proc_macro2::Span::call_site())),
    })
}

/// Extract an i64 value from a literal expression
fn int_literal_value(expr: &syn::Expr) -> Option<i64> {
    if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(li), .. }) = expr {
        li.base10_parse::<i64>().ok()
    } else {
        None
    }
}

/// Evaluate a binary comparison on two integer literals, returning a boolean
fn eval_binary_comparison(op: &syn::BinOp, left: i64, right: i64) -> Option<bool> {
    match op {
        syn::BinOp::Eq(_) => Some(left == right),
        syn::BinOp::Ne(_) => Some(left != right),
        syn::BinOp::Lt(_) => Some(left < right),
        syn::BinOp::Le(_) => Some(left <= right),
        syn::BinOp::Gt(_) => Some(left > right),
        syn::BinOp::Ge(_) => Some(left >= right),
        _ => None,
    }
}

/// Evaluate a binary arithmetic/logical operation on two integers, returning a boolean
fn eval_binary_i64(op: &syn::BinOp, left: i64, right: i64) -> Option<bool> {
    match op {
        syn::BinOp::And(_) => Some((left != 0) && (right != 0)),
        _ => None,
    }
}
