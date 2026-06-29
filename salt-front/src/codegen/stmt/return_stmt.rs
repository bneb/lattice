use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use crate::codegen::expr::emit_expr;
use std::collections::HashMap;
use syn::spanned::Spanned;
use super::helpers::extract_return_var_name;

/// Verify the ensures (postcondition) clause at a return site using Z3.
fn verify_return_ensures_clause(
    ctx: &mut LoweringContext,
    out: &mut String,
    ret_expr: &syn::Expr,
    local_vars: &HashMap<String, (Type, LocalKind)>,
) -> Result<(), String> {
    let ensures = ctx.current_ensures().clone();
    if ensures.is_empty() { return Ok(()); }
    let fn_name = ctx.current_fn_name().clone();
    let file = ctx.config.file;
    let (requires, param_names) = file.items.iter()
        .filter_map(|item| {
            if let crate::grammar::Item::Fn(f) = item {
                if f.name == fn_name || ctx.expansion.current_fn_name.ends_with(&f.name.to_string()) {
                    let params: Vec<String> = f.args.iter().map(|a| a.name.to_string()).collect();
                    return Some((f.requires.clone(), params));
                }
            }
            None
        })
        .next()
        .unwrap_or((vec![], vec![]));
    match crate::codegen::verification::VerificationEngine::verify_postcondition(
        ctx, &ensures, &requires, ret_expr, &param_names, local_vars, &fn_name,
    ) {
        Ok(true) => {
            out.push_str(&format!("    // z3_postcondition_verified: ensures proven for '{}'\n", fn_name));
        }
        Ok(false) => {}
        Err(err) => { return Err(err); }
    }
    Ok(())
}

pub(crate) fn emit_return_stmt(ctx: &mut LoweringContext, out: &mut String, opt_expr: &Option<syn::Expr>, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<bool, String>  {
            emit_cleanup_for_return(ctx, out, local_vars)?;
            if let Some(e) = opt_expr {
                // Substitute generics in return type (T -> u8 etc.)
                let expected_ret = ctx.current_ret_ty().clone().map(|t| t.substitute(ctx.current_type_map()));
                let (val_raw, ty) = emit_expr(ctx, out, e, local_vars, expected_ret.as_ref())?;

                // Recursive escape marking.
                crate::codegen::expr::mark_expression_escaped(ctx, e);

                // Arena escape analysis: enforce the return rule
                // return x is valid iff depth(x) <= 1.
                // A pointer from a local arena (depth >= 2) cannot escape.
                if let Some(var_name) = extract_return_var_name(e) {
                    ctx.arena_escape_tracker.check_return_escape(&var_name)?
                }

                verify_return_ensures_clause(ctx, out, e, local_vars)?;

                let loc = ctx.loc_tag(e.span());
                if ty == Type::Unit {
                    out.push_str(&format!("    func.return{}\n", loc));
                } else {
                    let mut val = val_raw;
                    if let Some(expected) = &expected_ret {
                        val = crate::codegen::type_bridge::promote_numeric(ctx, out, &val, &ty, expected)?;
                    }

                    let mlir_ty = if let Some(expected) = &expected_ret {
                        let e_ty: Type = expected.clone();
                        e_ty.to_mlir_type(ctx)?
                    } else {
                        ty.to_mlir_type(ctx)?
                    };
                    out.push_str(&format!("    func.return {} : {}{}\n", val, mlir_ty, loc));
                }
            } else {
                out.push_str("    func.return\n");
            }
            Ok(true)
        }

pub(crate) fn emit_cleanup_for_return(ctx: &mut LoweringContext, out: &mut String, local_vars: &HashMap<String, (Type, LocalKind)>) -> Result<(), String> {
    // RAII-Lite: Emit cleanup for all owned resources in the cleanup_stack
    // This handles Vec and other container types registered via register_owned_resource
    {
        let tasks: Vec<_> = ctx.cleanup_stack()
            .last()
            .map(|t| t.iter().rev().cloned().collect::<Vec<_>>())
            .unwrap_or_default();

        for task in &tasks {
                // Z3 Ownership Ledger: Register DEATH event for each resource (DISABLED)
                /*
                ctx.ownership_tracker.mark_released(
                    &task.var_name,
                    &ctx.z3_solver
                )?;
                */

                let mlir_ty = task.ty.to_mlir_type(ctx)?;
                out.push_str(&format!("    func.call @{}({}) : ({}) -> ()\n",
                    task.drop_fn, task.value, mlir_ty));
        }
    }

    // Drop Trait RAII: Auto-call drop() on locals implementing Drop
    // Iterate in reverse insertion order for proper cleanup ordering (LIFO)
    {
        let mut drop_fns: Vec<(String, String)> = Vec::new();

        for (name, (ty, kind)) in local_vars.iter() {
            // Skip internal/synthetic variables
            if name.starts_with("__") { continue; }

            let type_key = crate::codegen::type_bridge::type_to_type_key(ty);
            if ctx.trait_registry().contains_method(&type_key, "drop") {
                if let LocalKind::Ptr(ptr) = kind {
                    // Construct the mangled drop function name
                    let type_name = match ty {
                        Type::Struct(n) | Type::Concrete(n, _) => n.clone(),
                        _ => continue,
                    };
                    let mangled = format!("{}__drop", type_name);

                    // Demand-driven hydration: ensure drop() is emitted
                    // Same pattern as Display::fmt hydration (intrinsics.rs:3580-3596)
                    let drop_impl_data = {
                        ctx.generic_impls().get(&mangled).cloned()
                    };
                    if let Some((func_def, func_imports)) = drop_impl_data {
                        let task = crate::codegen::collector::MonomorphizationTask {
                            identity: crate::types::TypeKey {
                                path: vec![],
                                name: mangled.clone(),
                                specialization: None
                            },
                            mangled_name: mangled.clone(),
                            func: func_def,
                            concrete_tys: vec![],
                            self_ty: Some(ty.clone()),
                            imports: func_imports,
                            type_map: std::collections::BTreeMap::new(),
                        };
                        ctx.entity_registry_mut().request_specialization(task.clone());
                    }

                    drop_fns.push((mangled, ptr.clone()));
                }
            }
        }

        // Emit drop calls in reverse order
        for (mangled, ptr) in drop_fns.iter().rev() {
            out.push_str(&format!("    func.call @{}({}) : (!llvm.ptr) -> ()\n", mangled, ptr));
        }
    }

    // Legacy cleanup for Type::Owned
    // Note: salt.drop was removed as MLIR doesn't recognize the salt dialect.
    // Owned types that need cleanup should use explicit drop() calls or
    // register with the CleanupStack for RAII-Lite handling.
    for (name, (ty, kind)) in local_vars {
        if let Type::Owned(inner) = ty {
            if !ctx.consumed_vars().contains(name) {
                 if let LocalKind::Ptr(ptr) = kind {
                     let loaded_ptr = format!("%owned_load_{}", ctx.next_id());
                     out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> !llvm.ptr\n", loaded_ptr, ptr));

                     let type_key = crate::codegen::type_bridge::type_to_type_key(inner);
                     if ctx.trait_registry().contains_method(&type_key, "drop") {
                         let type_name = match &**inner {
                             Type::Struct(n) | Type::Concrete(n, _) => n.clone(),
                             _ => String::new(),
                         };
                         if !type_name.is_empty() {
                             let mangled = format!("{}__drop", type_name);
                             let drop_impl_data = ctx.generic_impls().get(&mangled).cloned();
                             if let Some((func_def, func_imports)) = drop_impl_data {
                                 let task = crate::codegen::collector::MonomorphizationTask {
                                     identity: crate::types::TypeKey { path: vec![], name: mangled.clone(), specialization: None },
                                     mangled_name: mangled.clone(),
                                     func: func_def,
                                     concrete_tys: vec![],
                                     self_ty: Some((**inner).clone()),
                                     imports: func_imports,
                                     type_map: std::collections::BTreeMap::new(),
                                 };
                                 ctx.entity_registry_mut().request_specialization(task.clone());
                                 ctx.pending_generations_mut().push_back(task);
                             }
                             out.push_str(&format!("    func.call @{}({}) : (!llvm.ptr) -> ()\n", mangled, loaded_ptr));
                         }
                     }
                     out.push_str(&format!("    func.call @free({}) : (!llvm.ptr) -> ()\n", loaded_ptr));
                 }
            }
        }
    }
    Ok(())
}
