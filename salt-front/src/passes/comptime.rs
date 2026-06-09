use crate::grammar::{SaltFile, Item, SaltBlock, Stmt};
use crate::evaluator::{Evaluator, ConstValue, EvalError};
use syn::{Expr, Lit, LitInt, LitBool, LitFloat};

pub fn run(file: &mut SaltFile) -> Result<(), EvalError> {
    let mut evaluator = Evaluator::new();
    
    // Process functions. In a real pass we'd handle global scope first.
    for item in &mut file.items {
        if let Item::Fn(func) = item {
            process_block(&mut func.body, &mut evaluator)?;
        }
    }
    Ok(())
}

fn process_block(block: &mut SaltBlock, evaluator: &mut Evaluator) -> Result<(), EvalError> {
    for stmt in &mut block.stmts {
        match stmt {
            Stmt::Syn(syn_stmt) => process_syn_stmt(syn_stmt, evaluator)?,
            Stmt::While(w) => {
                // Optimization: If condition is constant false, we could kill the block.
                // For now, just optimize inside
                process_block(&mut w.body, evaluator)?;
            }
            Stmt::For(f) => {
                process_block(&mut f.body, evaluator)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn process_syn_stmt(stmt: &mut syn::Stmt, evaluator: &mut Evaluator) -> Result<(), EvalError> {
        match stmt {
            syn::Stmt::Local(local) => {
                // Heuristic: If it's a `let` binding, we try to optimize it (Case B).
                // If we had `const` or `salt.constant` we would treat it as Case A (Mandatory).
                
                // For now, scan assignment: let x = <expr>;
                // If <expr> evaluates to a constant, register x in the table and replace <expr>.
                
                if let Some(init) = &mut local.init {
                    // Opportunity: Try to eval
                    match evaluator.eval_expr(&init.expr) {
                        Ok(val) => {
                            // It evaluated!
                            if matches!(val, ConstValue::Complex | ConstValue::Array(_)) {
                                // Skip optimization for complex types as we can't convert them back to AST easily
                            } else {
                                // 1. Register so future uses can see it
                                if let syn::Pat::Ident(pat_ident) = &local.pat {
                                    if pat_ident.mutability.is_none() {
                                        evaluator.constant_table.insert(pat_ident.ident.to_string(), val.clone());
                                    }
                                }
                                
                                // 2. Optimization: Replace the expression with the literal
                                *init.expr = value_to_expr(val);
                            }
                        },
                        Err(_) => {
                            // Fallback: It's runtime code. Do nothing.
                        }
                    }
                }
            }
            syn::Stmt::Expr(expr, _) => {
                 // Try to fold expressions too
                 if let Ok(val) = evaluator.eval_expr(expr) {
                     if !matches!(val, ConstValue::Complex | ConstValue::Array(_)) {
                         *expr = value_to_expr(val);
                     }
                 }
            }
            _ => {}
        }
    Ok(())
}

fn value_to_expr(val: ConstValue) -> Expr {
    match val {
        ConstValue::Integer(i) => Expr::Lit(syn::ExprLit {
            attrs: vec![],
            lit: Lit::Int(LitInt::new(&i.to_string(), proc_macro2::Span::call_site())),
        }),
        ConstValue::Float(f) => Expr::Lit(syn::ExprLit {
            attrs: vec![],
            lit: Lit::Float(LitFloat::new(&f.to_string(), proc_macro2::Span::call_site())),
        }),
        ConstValue::Bool(b) => Expr::Lit(syn::ExprLit {
            attrs: vec![],
            lit: Lit::Bool(LitBool::new(b, proc_macro2::Span::call_site())),
        }),
        ConstValue::String(s) => Expr::Lit(syn::ExprLit {
            attrs: vec![],
            lit: Lit::Str(syn::LitStr::new(&s, proc_macro2::Span::call_site())),
        }),
        ConstValue::Complex => panic!("Cannot convert complex constant back to expression"),
        ConstValue::Array(_) => panic!("Cannot convert array constant back to expression"),
    }
}
