//! Expression utility functions for Salt-specific syntax extensions.
//!
//! Handles `forall` quantifier expansion: `forall i in lo..hi: body`
//! expands to a conjunction when bounds are compile-time integer constants.

use syn::{
    parse::ParseStream,
    Expr, ExprLit, Ident, Token, LitInt, Lit,
    punctuated::Punctuated,
};

/// Parse an exists expression: `exists ident in expr..expr => expr`
/// Expands to a chain of || disjunctions when bounds are integer literals.
pub(crate) fn parse_exists_expr(input: ParseStream) -> syn::Result<Expr> {
    input.parse::<crate::keywords::exists>()?;
    let var: Ident = input.parse()?;
    input.parse::<Token![in]>()?;
    let range: Expr = input.parse()?;
    let (lo, hi) = match &range {
        Expr::Range(r) => (r.start.as_deref(), r.end.as_deref()),
        _ => return Err(syn::Error::new_spanned(&range, "expected range like 0..3 or 0..n")),
    };
    let lo = lo.ok_or_else(|| input.error("exists range must have a lower bound"))?;
    let hi = hi.ok_or_else(|| input.error("exists range must have an upper bound"))?;
    input.parse::<Token![=>]>()?;
    let body: Expr = input.parse()?;
    if let (Some(lo_val), Some(hi_val)) = (extract_int_literal(lo), extract_int_literal(hi)) {
        if hi_val <= lo_val { return Ok(syn::parse_quote! { false }); }
        let mut disjuncts: Vec<Expr> = Vec::new();
        for val in lo_val..hi_val {
            let replacement = Expr::Lit(ExprLit { attrs: vec![], lit: Lit::Int(LitInt::new(&val.to_string(), proc_macro2::Span::call_site())) });
            disjuncts.push(substitute_ident(&body, &var, &replacement));
        }
        let mut result = disjuncts.pop().unwrap();
        while let Some(next) = disjuncts.pop() {
            result = Expr::Binary(syn::ExprBinary { attrs: vec![], left: Box::new(next), op: syn::BinOp::Or(syn::token::OrOr::default()), right: Box::new(result) });
        }
        return Ok(result);
    }
    let var_name = Expr::Lit(ExprLit { attrs: vec![], lit: Lit::Str(syn::LitStr::new(&var.to_string(), proc_macro2::Span::call_site())) });
    let args: Punctuated<Expr, Token![,]> = vec![var_name, lo.clone(), hi.clone(), body].into_iter().collect();
    Ok(Expr::Call(syn::ExprCall { attrs: vec![], func: Box::new(syn::parse_quote! { __z3_exists }), paren_token: syn::token::Paren::default(), args }))
}

/// Parse a forall expression: `forall ident in expr..expr => expr`
/// Expands to a chain of && conjunctions when bounds are integer literals.
pub(crate) fn parse_forall_expr(input: ParseStream) -> syn::Result<Expr> {
    input.parse::<crate::keywords::forall>()?;

    let var: Ident = input.parse()?;
    input.parse::<Token![in]>()?;

    // Parse `lo..hi` as a Range expression, then extract bounds
    let range: Expr = input.parse()?;
    let (lo, hi) = match &range {
        Expr::Range(r) => (r.start.as_deref(), r.end.as_deref()),
        _ => return Err(syn::Error::new_spanned(&range, "expected range like `0..3` or `0..n`")),
    };
    let lo = lo.ok_or_else(|| input.error("forall range must have a lower bound"))?;
    let hi = hi.ok_or_else(|| input.error("forall range must have an upper bound"))?;

    input.parse::<Token![=>]>()?;

    let body: Expr = input.parse()?;

    if let (Some(lo_val), Some(hi_val)) = (extract_int_literal(lo), extract_int_literal(hi)) {
        if hi_val <= lo_val {
            return Ok(Expr::Lit(ExprLit {
                attrs: vec![],
                lit: Lit::Bool(syn::LitBool::new(true, input.span())),
            }));
        }
        let mut conjuncts: Vec<Expr> = Vec::new();
        for val in lo_val..hi_val {
            let replacement = Expr::Lit(ExprLit {
                attrs: vec![],
                lit: Lit::Int(LitInt::new(&val.to_string(), proc_macro2::Span::call_site())),
            });
            conjuncts.push(substitute_ident(&body, &var, &replacement));
        }
        let mut result = conjuncts.pop().unwrap();
        while let Some(next) = conjuncts.pop() {
            result = Expr::Binary(syn::ExprBinary {
                attrs: vec![],
                left: Box::new(next),
                op: syn::BinOp::And(syn::token::AndAnd::default()),
                right: Box::new(result),
            });
        }
        return Ok(result);
    }

    // Symbolic bounds: encode as __z3_forall(var, lo, hi, body) for Z3 ForAll
    let var_name = Expr::Lit(ExprLit {
        attrs: vec![],
        lit: Lit::Str(syn::LitStr::new(&var.to_string(), proc_macro2::Span::call_site())),
    });
    let args: Punctuated<Expr, Token![,]> = vec![var_name, lo.clone(), hi.clone(), body]
        .into_iter().collect();
    Ok(Expr::Call(syn::ExprCall {
        attrs: vec![],
        func: Box::new(syn::parse_quote! { __z3_forall }),
        paren_token: syn::token::Paren::default(),
        args,
    }))
}

/// Extract an i64 value from an integer literal expression.
fn extract_int_literal(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Lit(lit) => match &lit.lit {
            Lit::Int(li) => li.base10_parse::<i64>().ok(),
            _ => None,
        },
        Expr::Unary(unary) => {
            if let syn::UnOp::Neg(_) = unary.op {
                extract_int_literal(&unary.expr).map(|v| -v)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Substitute all occurrences of `var` in `expr` with `replacement`.
pub(crate) fn substitute_ident(expr: &Expr, var: &Ident, replacement: &Expr) -> Expr {
    match expr {
        Expr::Path(path) => {
            if path.path.is_ident(var) {
                replacement.clone()
            } else {
                expr.clone()
            }
        }
        Expr::Index(idx) => {
            Expr::Index(syn::ExprIndex {
                attrs: idx.attrs.clone(),
                expr: Box::new(substitute_ident(&idx.expr, var, replacement)),
                bracket_token: idx.bracket_token,
                index: Box::new(substitute_ident(&idx.index, var, replacement)),
            })
        }
        Expr::Binary(bin) => {
            Expr::Binary(syn::ExprBinary {
                attrs: bin.attrs.clone(),
                left: Box::new(substitute_ident(&bin.left, var, replacement)),
                op: bin.op,
                right: Box::new(substitute_ident(&bin.right, var, replacement)),
            })
        }
        Expr::Paren(p) => {
            Expr::Paren(syn::ExprParen {
                attrs: p.attrs.clone(),
                paren_token: p.paren_token,
                expr: Box::new(substitute_ident(&p.expr, var, replacement)),
            })
        }
        Expr::Unary(u) => {
            Expr::Unary(syn::ExprUnary {
                attrs: u.attrs.clone(),
                op: u.op,
                expr: Box::new(substitute_ident(&u.expr, var, replacement)),
            })
        }
        Expr::Group(g) => {
            Expr::Group(syn::ExprGroup {
                attrs: g.attrs.clone(),
                group_token: g.group_token,
                expr: Box::new(substitute_ident(&g.expr, var, replacement)),
            })
        }
        Expr::Call(c) => {
            let func = substitute_ident(&c.func, var, replacement);
            let args: Punctuated<Expr, Token![,]> = c.args.iter()
                .map(|a| substitute_ident(a, var, replacement))
                .collect();
            Expr::Call(syn::ExprCall {
                attrs: c.attrs.clone(),
                func: Box::new(func),
                paren_token: c.paren_token,
                args,
            })
        }
        // Default: return unchanged for expressions we don't decompose
        _ => expr.clone(),
    }
}
