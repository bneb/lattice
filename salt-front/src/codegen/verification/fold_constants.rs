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
    params: &[String],
    arg_exprs: &[syn::Expr],
) -> Option<ConstValue> {
    // Build constant table from known argument values
    let mut constant_table: HashMap<String, ConstValue> = HashMap::new();
    for (param, &len) in known_lengths {
        constant_table.insert(param.clone(), ConstValue::Integer(len));
    }

    // Substitute parameters with compile-time-known argument values
    let mut substituted = expr.clone();
    for (i, param) in params.iter().enumerate() {
        if i < arg_exprs.len() {
            if let Some(value) = arg_to_const(&arg_exprs[i]) {
                substituted = substitute_param(&substituted, param, &arg_exprs[i]);
                // Also insert into constant table for Evaluator path lookups
                if let ConstValue::Integer(n) = value {
                    constant_table.insert(param.clone(), ConstValue::Integer(n));
                }
            }
        }
    }

    let evaluator = Evaluator {
        depth_limit: 100,
        constant_table,
    };

    // Resolve .length() and string content methods to literals
    let resolved = resolve_methods(&substituted, known_lengths);

    // Use the Evaluator for everything else
    evaluator.eval_expr(&resolved).ok()
}

/// Convert an argument expression to a ConstValue if it's a literal.
fn arg_to_const(expr: &syn::Expr) -> Option<ConstValue> {
    match expr {
        syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(li), .. }) => {
            Some(ConstValue::Integer(li.base10_parse::<i64>().ok()?))
        }
        syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) => {
            Some(ConstValue::String(s.value()))
        }
        syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Bool(b), .. }) => {
            Some(ConstValue::Bool(b.value))
        }
        syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Float(lf), .. }) => {
            Some(ConstValue::Float(lf.base10_parse::<f64>().ok()?))
        }
        _ => None,
    }
}

/// Substitute a parameter reference with its argument expression in the AST.
fn substitute_param(expr: &syn::Expr, param: &str, arg: &syn::Expr) -> syn::Expr {
    match expr {
        syn::Expr::Path(p) => {
            if p.path.get_ident().is_some_and(|i| i == param) {
                return arg.clone();
            }
            expr.clone()
        }
        syn::Expr::Binary(b) => syn::Expr::Binary(syn::ExprBinary {
            attrs: b.attrs.clone(),
            left: Box::new(substitute_param(&b.left, param, arg)),
            op: b.op,
            right: Box::new(substitute_param(&b.right, param, arg)),
        }),
        syn::Expr::MethodCall(mc) => syn::Expr::MethodCall(syn::ExprMethodCall {
            attrs: mc.attrs.clone(),
            receiver: Box::new(substitute_param(&mc.receiver, param, arg)),
            dot_token: mc.dot_token,
            method: mc.method.clone(),
            turbofish: mc.turbofish.clone(),
            paren_token: mc.paren_token,
            args: mc.args.iter().map(|a| substitute_param(a, param, arg)).collect(),
        }),
        syn::Expr::Paren(p) => syn::Expr::Paren(syn::ExprParen {
            attrs: p.attrs.clone(), paren_token: p.paren_token,
            expr: Box::new(substitute_param(&p.expr, param, arg)),
        }),
        syn::Expr::Unary(u) => syn::Expr::Unary(syn::ExprUnary {
            attrs: u.attrs.clone(), op: u.op,
            expr: Box::new(substitute_param(&u.expr, param, arg)),
        }),
        syn::Expr::Block(block) => {
            if let Some(syn::Stmt::Expr(inner, _semi)) = block.block.stmts.first() {
                let folded = substitute_param(inner, param, arg);
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

/// Extract a string literal value from an expression, if possible.
fn string_literal_value(expr: &syn::Expr) -> Option<String> {
    if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = expr {
        Some(s.value())
    } else {
        None
    }
}

/// Resolve .length(), .len(), .contains(), .starts_with(), .ends_with()
/// method calls to integer or boolean literals.
fn resolve_methods(expr: &syn::Expr, known_lengths: &HashMap<String, i64>) -> syn::Expr {
    match expr {
        syn::Expr::MethodCall(mc) => {
            let method = mc.method.to_string();

            // String length — resolve to integer literal
            if method == "length" || method == "len" {
                if let Some(s) = string_literal_value(&mc.receiver) {
                    return make_int_literal(s.len() as i64);
                }
                if let syn::Expr::Path(p) = &*mc.receiver {
                    if let Some(ident) = p.path.get_ident() {
                        if let Some(&len) = known_lengths.get(&ident.to_string()) {
                            return make_int_literal(len);
                        }
                    }
                }
            }

            // String content — resolve to boolean literal
            if (method == "contains" || method == "starts_with" || method == "ends_with")
                && mc.args.len() == 1
            {
                if let (Some(receiver), Some(arg)) = (
                    string_literal_value(&mc.receiver),
                    string_literal_value(&mc.args[0]),
                ) {
                    let result = match method.as_str() {
                        "contains" => receiver.contains(&arg),
                        "starts_with" => receiver.starts_with(&arg),
                        "ends_with" => receiver.ends_with(&arg),
                        _ => unreachable!(),
                    };
                    return make_bool_literal(result);
                }
            }

            // Regex match — resolve to boolean literal via regex crate
            if method == "matches" && mc.args.len() == 1 {
                if let (Some(text), Some(pattern)) = (
                    string_literal_value(&mc.receiver),
                    string_literal_value(&mc.args[0]),
                ) {
                    let result = regex::Regex::new(&pattern)
                        .map(|re| re.is_match(&text))
                        .unwrap_or(false);
                    return make_bool_literal(result);
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

fn make_bool_literal(val: bool) -> syn::Expr {
    syn::Expr::Lit(syn::ExprLit {
        attrs: vec![],
        lit: syn::Lit::Bool(syn::LitBool::new(val, proc_macro2::Span::call_site())),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn empty_lengths() -> HashMap<String, i64> { HashMap::new() }
    fn empty_params() -> Vec<String> { vec![] }
    fn empty_args() -> Vec<syn::Expr> { vec![] }

    fn parse_expr(s: &str) -> syn::Expr {
        syn::parse_str(s).expect("failed to parse test expression")
    }

    #[test]
    fn test_int_literal_folds() {
        let expr = parse_expr("42");
        let result = try_eval(&expr, &empty_lengths(), &empty_params(), &empty_args());
        assert_eq!(result, Some(crate::evaluator::ConstValue::Integer(42)));
    }

    #[test]
    fn test_string_length_literal() {
        let expr = parse_expr("\"hello\".length()");
        let result = try_eval(&expr, &empty_lengths(), &empty_params(), &empty_args());
        assert_eq!(result, Some(crate::evaluator::ConstValue::Integer(5)));
    }

    #[test]
    fn test_string_length_known_param() {
        let expr = parse_expr("key.length()");
        let mut lengths = HashMap::new();
        lengths.insert("key".to_string(), 5);
        let result = try_eval(&expr, &lengths, &empty_params(), &empty_args());
        assert_eq!(result, Some(crate::evaluator::ConstValue::Integer(5)));
    }

    #[test]
    fn test_starts_with_true() {
        let expr = parse_expr("\"hello\".starts_with(\"hel\")");
        let result = try_eval(&expr, &empty_lengths(), &empty_params(), &empty_args());
        assert_eq!(result, Some(crate::evaluator::ConstValue::Bool(true)));
    }

    #[test]
    fn test_starts_with_false() {
        let expr = parse_expr("\"hello\".starts_with(\"xyz\")");
        let result = try_eval(&expr, &empty_lengths(), &empty_params(), &empty_args());
        assert_eq!(result, Some(crate::evaluator::ConstValue::Bool(false)));
    }

    #[test]
    fn test_ends_with_true() {
        let expr = parse_expr("\"program.salt\".ends_with(\".salt\")");
        let result = try_eval(&expr, &empty_lengths(), &empty_params(), &empty_args());
        assert_eq!(result, Some(crate::evaluator::ConstValue::Bool(true)));
    }

    #[test]
    fn test_contains_true() {
        let expr = parse_expr("\"hello world\".contains(\"lo w\")");
        let result = try_eval(&expr, &empty_lengths(), &empty_params(), &empty_args());
        assert_eq!(result, Some(crate::evaluator::ConstValue::Bool(true)));
    }

    #[test]
    fn test_param_substitution_starts_with() {
        let requires_expr = parse_expr("key.starts_with(\"salt-\")");
        let params = vec!["key".to_string()];
        let args = vec![parse_expr("\"salt-lang\"")];
        let result = try_eval(&requires_expr, &empty_lengths(), &params, &args);
        assert_eq!(result, Some(crate::evaluator::ConstValue::Bool(true)));
    }

    #[test]
    fn test_param_substitution_false_case() {
        let requires_expr = parse_expr("key.starts_with(\"salt-\")");
        let params = vec!["key".to_string()];
        let args = vec![parse_expr("\"wrong-key\"")];
        let result = try_eval(&requires_expr, &empty_lengths(), &params, &args);
        assert_eq!(result, Some(crate::evaluator::ConstValue::Bool(false)));
    }

    #[test]
    fn test_compound_comparison_substitution() {
        let requires_expr = parse_expr("key.length() > 0");
        let params = vec!["key".to_string()];
        let args = vec![parse_expr("\"hello\"")];
        let result = try_eval(&requires_expr, &empty_lengths(), &params, &args);
        assert_eq!(result, Some(crate::evaluator::ConstValue::Bool(true)));
    }

    #[test]
    fn test_matches_true() {
        let expr = parse_expr("\"deadbeef\".matches(\"^[0-9a-f]+$\")");
        let result = try_eval(&expr, &empty_lengths(), &empty_params(), &empty_args());
        assert_eq!(result, Some(crate::evaluator::ConstValue::Bool(true)));
    }

    #[test]
    fn test_matches_false() {
        let expr = parse_expr("\"hello\".matches(\"^[0-9]+$\")");
        let result = try_eval(&expr, &empty_lengths(), &empty_params(), &empty_args());
        assert_eq!(result, Some(crate::evaluator::ConstValue::Bool(false)));
    }

    #[test]
    fn test_matches_invalid_regex() {
        let expr = parse_expr("\"abc\".matches(\"[invalid\")");
        let result = try_eval(&expr, &empty_lengths(), &empty_params(), &empty_args());
        assert_eq!(result, Some(crate::evaluator::ConstValue::Bool(false)));
    }

    #[test]
    fn test_symbolic_returns_none() {
        let requires_expr = parse_expr("x > 0");
        let params = vec!["x".to_string()];
        let args = vec![parse_expr("x")]; // variable, not literal
        let result = try_eval(&requires_expr, &empty_lengths(), &params, &args);
        assert_eq!(result, None); // symbolic — can't evaluate
    }
}
