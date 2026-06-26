// Constant folder for Z3 contract expressions.
//
// Before a requires/ensures expression is translated to Z3, this pass
// evaluates sub-expressions using the compiler's built-in Evaluator.
// The Evaluator handles integer/float/bool literals, binary ops,
// comparisons, and path lookups via a constant table.
//
// This pass adds MethodCall resolution (.length() → int) which the
// Evaluator does not handle, then delegates everything else.

use std::collections::HashMap;
use crate::evaluator::{ConstValue, Evaluator};

/// Resolve an expression to a concrete ConstValue, or return None
/// if it depends on symbolic (runtime) values.
pub fn try_eval(
    expr: &syn::Expr,
    known_lengths: &HashMap<String, i64>,
) -> Option<ConstValue> {
    // Build constant table from known argument values
    let mut constant_table: HashMap<String, ConstValue> = HashMap::new();
    for (param, &len) in known_lengths {
        // Insert the parameter as an integer (its known length)
        constant_table.insert(param.clone(), ConstValue::Integer(len));
    }

    let evaluator = Evaluator {
        depth_limit: 100,
        constant_table,
    };

    // First pass: resolve .length() method calls to integers
    let resolved = resolve_methods(expr, known_lengths);

    // Second pass: use the Evaluator for everything else
    evaluator.eval_expr(&resolved).ok()
}

/// Resolve .length() and .len() method calls to integer literals.
/// This is the one operation the Evaluator doesn't handle.
fn resolve_methods(expr: &syn::Expr, known_lengths: &HashMap<String, i64>) -> syn::Expr {
    match expr {
        syn::Expr::MethodCall(mc) => {
            let method = mc.method.to_string();
            if method == "length" || method == "len" {
                // String literal receiver
                if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &*mc.receiver {
                    return make_int_literal(s.value().len() as i64);
                }
                // Known parameter
                if let syn::Expr::Path(p) = &*mc.receiver {
                    if let Some(ident) = p.path.get_ident() {
                        if let Some(&len) = known_lengths.get(&ident.to_string()) {
                            return make_int_literal(len);
                        }
                    }
                }
            }
            // Recurse
            let folded_receiver = Box::new(resolve_methods(&mc.receiver, known_lengths));
            let folded_args: Vec<syn::Expr> = mc.args.iter()
                .map(|a| resolve_methods(a, known_lengths))
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
        syn::Expr::Binary(b) => syn::Expr::Binary(syn::ExprBinary {
            attrs: b.attrs.clone(),
            left: Box::new(resolve_methods(&b.left, known_lengths)),
            op: b.op,
            right: Box::new(resolve_methods(&b.right, known_lengths)),
        }),
        syn::Expr::Paren(p) => syn::Expr::Paren(syn::ExprParen {
            attrs: p.attrs.clone(), paren_token: p.paren_token,
            expr: Box::new(resolve_methods(&p.expr, known_lengths)),
        }),
        syn::Expr::Group(g) => syn::Expr::Group(syn::ExprGroup {
            attrs: g.attrs.clone(), group_token: g.group_token,
            expr: Box::new(resolve_methods(&g.expr, known_lengths)),
        }),
        syn::Expr::Unary(u) => syn::Expr::Unary(syn::ExprUnary {
            attrs: u.attrs.clone(), op: u.op,
            expr: Box::new(resolve_methods(&u.expr, known_lengths)),
        }),
        syn::Expr::Block(block) => {
            if let Some(syn::Stmt::Expr(inner, _semi)) = block.block.stmts.first() {
                let folded = resolve_methods(inner, known_lengths);
                let mut new_block = block.clone();
                if let Some(syn::Stmt::Expr(_, semi)) = new_block.block.stmts.first_mut() {
                    *new_block.block.stmts.first_mut().unwrap() = syn::Stmt::Expr(folded, *semi);
                }
                syn::Expr::Block(new_block)
            } else { expr.clone() }
        }
        _ => expr.clone(),
    }
}

fn make_int_literal(val: i64) -> syn::Expr {
    syn::Expr::Lit(syn::ExprLit {
        attrs: vec![],
        lit: syn::Lit::Int(syn::LitInt::new(&val.to_string(), proc_macro2::Span::call_site())),
    })
}
