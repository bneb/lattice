use crate::grammar::{Stmt, SaltElse, SaltIf};
use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use crate::codegen::type_bridge::resolve_type;
use std::collections::HashMap;

pub fn salt_block_always_returns(stmts: &[Stmt]) -> bool {
    for stmt in stmts {
        match stmt {
            Stmt::Return(_) => return true,
            Stmt::Expr(syn::Expr::Return(_), _) => return true,
            Stmt::Syn(syn::Stmt::Expr(syn::Expr::Return(_), _)) => return true,
            Stmt::If(f) => { if salt_if_always_returns(f) { return true; } }
            _ => {}
        }
    }
    false
}

fn salt_if_always_returns(f: &SaltIf) -> bool {
    let Some(else_branch) = &f.else_branch else { return false; };
    salt_block_always_returns(&f.then_branch.stmts) && salt_else_always_returns(else_branch.as_ref())
}

fn salt_else_always_returns(else_branch: &SaltElse) -> bool {
    match else_branch {
        SaltElse::Block(b) => salt_block_always_returns(&b.stmts),
        SaltElse::If(nested) => salt_block_always_returns(&nested.then_branch.stmts),
    }
}

pub fn hoist_allocas_in_block(ctx: &mut LoweringContext, stmts: &[Stmt], local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<(), String> {
    for stmt in stmts {
        match stmt {
            Stmt::Syn(syn::Stmt::Local(local)) => {
                let pat = match &local.pat {
                    syn::Pat::Type(pt) => &pt.pat,
                    p => p,
                };
                if let syn::Pat::Ident(id) = pat {
                    let name = id.ident.to_string();

                    if let std::collections::hash_map::Entry::Vacant(e) = local_vars.entry(name.clone()) {
                        let ty = if let syn::Pat::Type(pt) = &local.pat {
                            resolve_type(ctx, &crate::grammar::SynType::from_std(*pt.ty.clone()).map_err(|e| e.to_string())?)
                        } else if let Some(_init) = &local.init {
                            continue;
                        } else {
                            Type::I32
                        };

                        let alloca = format!("%local_{}_{}", name, ctx.next_id());
                        let mlir_ty = ty.to_mlir_storage_type(ctx)?;
                        ctx.emit_alloca(&mut String::new(), &alloca, &mlir_ty);
                        e.insert((ty, LocalKind::Ptr(alloca)));
                    }
                }
            }
            Stmt::While(w) => {
                let mut inner_vars = local_vars.clone();
                hoist_allocas_in_block(ctx, &w.body.stmts, &mut inner_vars)?;
            }
            Stmt::Loop(body) => {
                let mut inner_vars = local_vars.clone();
                hoist_allocas_in_block(ctx, &body.stmts, &mut inner_vars)?;
            }
            Stmt::If(f) => hoist_allocas_if_block(ctx, f, local_vars)?,
            Stmt::For(f) => {
                let mut inner_vars = local_vars.clone();
                hoist_allocas_in_block(ctx, &f.body.stmts, &mut inner_vars)?;
            }
            Stmt::Unsafe(b) => {
                let mut inner_vars = local_vars.clone();
                hoist_allocas_in_block(ctx, &b.stmts, &mut inner_vars)?;
            }
            Stmt::DynamicCheck(b) => {
                let mut inner_vars = local_vars.clone();
                hoist_allocas_in_block(ctx, &b.stmts, &mut inner_vars)?;
            }
            Stmt::WithRegion { region: _, body } => {
                let mut inner_vars = local_vars.clone();
                hoist_allocas_in_block(ctx, &body.stmts, &mut inner_vars)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn hoist_allocas_if_block(ctx: &mut LoweringContext, f: &SaltIf, local_vars: &HashMap<String, (Type, LocalKind)>) -> Result<(), String> {
    let mut then_vars = local_vars.clone();
    hoist_allocas_in_block(ctx, &f.then_branch.stmts, &mut then_vars)?;
    let Some(eb) = &f.else_branch else { return Ok(()); };
    let mut else_vars = local_vars.clone();
    hoist_allocas_in_else_branch(ctx, eb.as_ref(), &mut else_vars)
}

fn hoist_allocas_in_else_branch(
    ctx: &mut LoweringContext,
    eb: &SaltElse,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<(), String> {
    match eb {
        SaltElse::Block(b) => hoist_allocas_in_block(ctx, &b.stmts, local_vars),
        SaltElse::If(nested) => hoist_allocas_in_block(ctx, &nested.then_branch.stmts, local_vars),
    }
}
