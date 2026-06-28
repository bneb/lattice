use crate::grammar::{Stmt, SaltElse};

/// Try to extract a constant integer from an expression for affine loop bounds.
/// Returns Some(value) if the expression is a compile-time constant literal.
pub(crate) fn try_extract_const_int(expr: &syn::Expr) -> Option<i64> {
    match expr {
        syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(lit), .. }) => {
            lit.base10_parse::<i64>().ok()
        }
        // Could extend to handle const identifiers, simple arithmetic, etc.
        _ => None,
    }
}
/// Check if a block contains if statements or if expressions which would create control flow
/// incompatible with affine.for (which requires a single basic block).
pub(crate) fn block_has_control_flow(stmts: &[Stmt]) -> bool {
    for stmt in stmts {
        match stmt {
            Stmt::If(_) => return true,
            Stmt::Syn(syn::Stmt::Local(local)) => {
                // Check if the initializer contains an if expression
                if let Some(init) = &local.init {
                    if expr_has_if(&init.expr) {
                        return true;
                    }
                }
            }
            Stmt::Syn(syn::Stmt::Expr(e, _)) => {
                if expr_has_if(e) {
                    return true;
                }
            }
            Stmt::Expr(e, _) => {
                if expr_has_if(e) {
                    return true;
                }
            }
            Stmt::For(f) => {
                // Nested for loops are OK if their bodies have no control flow
                // This allows affine.for nesting for MatMul optimization
                if block_has_control_flow(&f.body.stmts) {
                    return true;
                }
            }
            Stmt::While(_) => {
                // While loops ALWAYS create cf.br/cf.cond_br - multiple blocks
                // They are fundamentally incompatible with affine.for nesting
                return true;
            }
            Stmt::Unsafe(u) => {
                if block_has_control_flow(&u.stmts) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// Check if an if expression can be lowered to arith.select (no true control flow).
/// True for: `if cond { literal } else { literal }` patterns.
pub(crate) fn is_select_compatible_if(expr: &syn::ExprIf) -> bool {
    // Must have else branch
    let else_branch = match &expr.else_branch {
        Some((_, e)) => e,
        None => return false,
    };
    
    // Check if then branch is a simple expression (literal, variable, or simple arithmetic)
    let then_ok = is_simple_expr_block(&expr.then_branch);
    
    // Check if else branch is a simple expression
    let else_ok = match else_branch.as_ref() {
        syn::Expr::Block(b) => is_simple_expr_block(&b.block),
        syn::Expr::If(nested) => is_select_compatible_if(nested),  // Chained if-else
        _ => is_simple_scalar_expr(else_branch),
    };
    
    then_ok && else_ok
}

/// Check if a block contains only a simple expression (no control flow).
pub(crate) fn is_simple_expr_block(block: &syn::Block) -> bool {
    if block.stmts.len() != 1 {
        return false;
    }
    match &block.stmts[0] {
        syn::Stmt::Expr(e, _) => is_simple_scalar_expr(e),
        _ => false,
    }
}
/// Check if an expression is a simple scalar value (literal, variable, simple arithmetic).
pub(crate) fn is_simple_scalar_expr(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Lit(lit) => matches!(lit.lit, syn::Lit::Int(_) | syn::Lit::Float(_)),
        syn::Expr::Path(_) => true,  // Variable reference
        syn::Expr::Binary(b) => is_simple_scalar_expr(&b.left) && is_simple_scalar_expr(&b.right),
        syn::Expr::Unary(u) => is_simple_scalar_expr(&u.expr),
        syn::Expr::Paren(p) => is_simple_scalar_expr(&p.expr),
        syn::Expr::Cast(c) => is_simple_scalar_expr(&c.expr),
        _ => false,
    }
}
/// Check if an expression contains an if expression that creates REAL control flow.
/// Select-compatible if expressions (simple scalar branches) are allowed.
pub(crate) fn expr_has_if(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::If(if_expr) => !is_select_compatible_if(if_expr),  // Allow select-compatible
        syn::Expr::Binary(b) => expr_has_if(&b.left) || expr_has_if(&b.right),
        syn::Expr::Unary(u) => expr_has_if(&u.expr),
        syn::Expr::Call(c) => c.args.iter().any(expr_has_if),
        syn::Expr::MethodCall(m) => m.args.iter().any(expr_has_if),
        syn::Expr::Reference(r) => expr_has_if(&r.expr),
        syn::Expr::Paren(p) => expr_has_if(&p.expr),
        syn::Expr::Field(f) => expr_has_if(&f.base),
        syn::Expr::Index(i) => expr_has_if(&i.expr) || expr_has_if(&i.index),
        syn::Expr::Assign(a) => expr_has_if(&a.left) || expr_has_if(&a.right),
        syn::Expr::Block(b) => b.block.stmts.iter().any(|s| match s {
            syn::Stmt::Expr(e, _) => expr_has_if(e),
            _ => false,
        }),
        _ => false,
    }
}
/// KeuOS Body Analysis: Detect if statements contain tensor indexing
/// Returns true if any statement uses tensor/array indexing (A[i,j] pattern)
/// This indicates the loop benefits from polyhedral optimization (affine.for)
pub(crate) fn has_tensor_indexing(stmts: &[Stmt]) -> bool {
    for stmt in stmts {
        let found = match stmt {
            Stmt::Expr(expr, _) => expr_has_tensor_indexing(expr),
            Stmt::For(salt_for) => has_tensor_indexing(&salt_for.body.stmts),
            Stmt::If(salt_if) => {
                has_tensor_indexing(&salt_if.then_branch.stmts)
                    || salt_if.else_branch.as_ref().is_some_and(|eb| has_tensor_indexing_in_else_branch(eb.as_ref()))
            }
            Stmt::While(salt_while) => has_tensor_indexing(&salt_while.body.stmts),
            Stmt::Syn(syn::Stmt::Local(syn::Local { init: Some(syn::LocalInit { expr, .. }), .. })) => expr_has_tensor_indexing(expr),
            Stmt::Syn(syn::Stmt::Expr(expr, _)) => expr_has_tensor_indexing(expr),
            _ => false,
        };
        if found { return true; }
    }
    false
}


/// Check if a SaltElse branch contains tensor indexing, recursing into nested if-else.
pub(crate) fn has_tensor_indexing_in_else_branch(else_branch: &SaltElse) -> bool {
    match else_branch {
        SaltElse::Block(b) => has_tensor_indexing(&b.stmts),
        SaltElse::If(nested_if) => has_tensor_indexing(&nested_if.then_branch.stmts),
    }
}

/// Check if a syn::Expr contains tensor/array indexing (Index expressions)
pub(crate) fn expr_has_tensor_indexing(expr: &syn::Expr) -> bool {
    match expr {
        // Found tensor indexing! (A[i,j] or tensor[(i, j)])
        syn::Expr::Index(_) => true,
        
        // Recurse into nested expressions
        syn::Expr::Binary(b) => expr_has_tensor_indexing(&b.left) || expr_has_tensor_indexing(&b.right),
        syn::Expr::Assign(a) => expr_has_tensor_indexing(&a.left) || expr_has_tensor_indexing(&a.right),
        syn::Expr::Unary(u) => expr_has_tensor_indexing(&u.expr),
        syn::Expr::Paren(p) => expr_has_tensor_indexing(&p.expr),
        syn::Expr::Cast(c) => expr_has_tensor_indexing(&c.expr),
        syn::Expr::Field(f) => expr_has_tensor_indexing(&f.base),
        syn::Expr::Reference(r) => expr_has_tensor_indexing(&r.expr),
        syn::Expr::Call(c) => c.args.iter().any(expr_has_tensor_indexing),
        syn::Expr::MethodCall(m) => expr_has_tensor_indexing(&m.receiver) || m.args.iter().any(expr_has_tensor_indexing),
        
        _ => false,
    }
}
