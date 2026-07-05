use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use std::collections::HashMap;

/// Assert callee postconditions into the caller's Z3 solver.
///
/// After `y = f(x)`, if f has `#ensures { result > 0 }`, this asserts
/// `y > 0` into the caller's solver so subsequent verification can
/// rely on the postcondition.
pub(crate) fn apply_ensures_to_solver(
    ctx: &mut LoweringContext,
    ensures: &[syn::Expr],
    param_names: &[String],
    args_vec: &[syn::Expr],
    result_ssa: &str,
) {
    if ensures.is_empty() || ctx.config.no_verify {
        return;
    }
    let sym_ctx = crate::codegen::verification::SymbolicContext::new(ctx.z3_ctx);
    use crate::z3_shim::ast::Ast;

    // Build local_vars: map param names to SSA-friendly entries,
    // plus "result" → the call's return SSA value.
    let mut locals: HashMap<String, (Type, LocalKind)> = HashMap::new();
    for p_name in param_names.iter() {
        locals.insert(p_name.clone(), (Type::I32, LocalKind::SSA(p_name.clone())));
    }
    locals.insert("result".to_string(), (Type::I32, LocalKind::SSA(result_ssa.to_string())));

    // Create param symbols and substitution pairs.
    // Insert each param symbol into symbolic_tracker so translate_to_z3
    // resolves parameter references to the same Z3 constant used in the
    // substitution. Save old bindings and restore after translation.
    let mut from_vec: Vec<crate::z3_shim::ast::Int> = Vec::new();
    let mut to_vec: Vec<crate::z3_shim::ast::Int> = Vec::new();
    let mut saved: Vec<(String, Option<crate::z3_shim::ast::Int>)> = Vec::new();
    for (i, p_name) in param_names.iter().enumerate() {
        let p_sym = crate::z3_shim::ast::Int::new_const(ctx.z3_ctx, p_name.clone());
        let for_tracker = p_sym.clone();
        let for_fallback = p_sym.clone();
        from_vec.push(p_sym);
        let old = ctx.symbolic_tracker.insert(p_name.clone(), for_tracker);
        saved.push((p_name.clone(), old));
        if i < args_vec.len() {
            if let Ok(arg_z3) = crate::codegen::expr::translate_to_z3(ctx, &args_vec[i], &locals) {
                to_vec.push(arg_z3);
            } else {
                to_vec.push(for_fallback);
            }
        } else {
            to_vec.push(for_fallback);
        }
    }

    let subs: Vec<(&crate::z3_shim::ast::Int, &crate::z3_shim::ast::Int)> =
        from_vec.iter().zip(to_vec.iter()).collect();

    for ens in ensures {
        let actual_ens = if let syn::Expr::Block(block) = ens {
            if let Some(syn::Stmt::Expr(inner, _)) = block.block.stmts.first() {
                inner
            } else {
                continue;
            }
        } else {
            ens
        };

        if let Ok(z3_ens) = crate::codegen::expr::translate_bool_to_z3(
            ctx, actual_ens, &locals, &sym_ctx,
        ) {
            let z3_subst = z3_ens.substitute(&subs);
            ctx.z3_solver.assert(&z3_subst);
        }
    }

    // Restore symbolic_tracker bindings saved before translation
    for (name, old) in saved {
        if let Some(old_val) = old {
            ctx.symbolic_tracker.insert(name, old_val);
        } else {
            ctx.symbolic_tracker.remove(&name);
        }
    }
}

#[allow(clippy::too_many_arguments)] // REASON: all 9 params independently meaningful; bundling would obscure intent
pub(crate) fn emit_low_level_call(
    ctx: &mut LoweringContext,
    out: &mut String,
    mangled_name: &str,
    args_vec: &[syn::Expr],
    args_vals: &[String],
    final_arg_tys: &[Type],
    ret_ty: &Type,
    ensures: &[syn::Expr],
    param_names: &[String],
) -> Result<(String, Type), String> {
    ctx.ensure_func_declared(mangled_name, final_arg_tys, ret_ty)?;

    let mut args_tys_code = Vec::new();
    let args_str = args_vals.join(", ");
    for t in final_arg_tys {
        args_tys_code.push(t.to_mlir_type(ctx)?);
    }
    let args_tys_str = args_tys_code.join(", ");
    
    let res_val = if *ret_ty != Type::Unit {
        format!("%call_{}_{}", mangled_name, ctx.next_id())
    } else {
        "".to_string() 
    };

    if mangled_name == "memcpy" && args_vals.len() == 3 {
        let is_ptr = |t: &Type| {
            match t {
                Type::Struct(name) => name.contains("Ptr"),
                _ => false,
            }
        };
        let dest_ptr = if is_ptr(&final_arg_tys[0]) {
            args_vals[0].clone()
        } else {
            let p = format!("%memcpy_dest_{}", ctx.next_id());
            out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", p, args_vals[0]));
            p
        };
        
        let src_ptr = if is_ptr(&final_arg_tys[1]) {
            args_vals[1].clone()
        } else {
            let p = format!("%memcpy_src_{}", ctx.next_id());
            out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", p, args_vals[1]));
            p
        };
        
        let size_val = if is_ptr(&final_arg_tys[2]) {
            let s = format!("%memcpy_size_{}", ctx.next_id());
            out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", s, args_vals[2]));
            s
        } else {
            args_vals[2].clone()
        };

        out.push_str(&format!("    \"llvm.intr.memcpy\"({}, {}, {}) <{{isVolatile = false}}> : (!llvm.ptr, !llvm.ptr, i64) -> ()\n", 
            dest_ptr, src_ptr, size_val));
        
        let ret_val = if *ret_ty != Type::Unit {
            crate::codegen::type_bridge::cast_numeric(ctx, out, &args_vals[0], &final_arg_tys[0], ret_ty)?
        } else {
            "".to_string()
        };
        
        return Ok((ret_val, ret_ty.clone()));
    } else if mangled_name == "free" && !args_vals.is_empty() {
        if let Some(first_arg) = args_vec.first() {
            if let Some(var_name) = super::extract_ident_name(first_arg) {
                let alloc_id = format!("malloc:{}", var_name);
                ctx.ownership_tracker.mark_released(&alloc_id, ctx.z3_solver)?;
                ctx.malloc_tracker.free(&alloc_id);
                ctx.pointer_tracker.mark_freed(&var_name);
            }
        }
        out.push_str(&format!("    func.call @{}({}) : ({}) -> ()\n", mangled_name, args_str, args_tys_str));
    } else if res_val.is_empty() {
        out.push_str(&format!("    func.call @{}({}) : ({}) -> ()\n", mangled_name, args_str, args_tys_str));
    } else {
        out.push_str(&format!("    {} = func.call @{}({}) : ({}) -> {}\n", res_val, mangled_name, args_str, args_tys_str, ret_ty.to_mlir_type(ctx)?));
    }
    
    ctx.emission.global_lvn.clear();

    if mangled_name == "malloc" && !res_val.is_empty() {
        *ctx.pending_malloc_result = Some(res_val.clone());
    }

    for (i, arg_expr) in args_vec.iter().enumerate() {
        super::mark_expression_escaped(ctx, arg_expr);
        
        // Only mark Optional for truly external/FFI, not user-defined functions
        let is_user_fn = ctx.config.file.items.iter().any(|item|
            matches!(item, crate::grammar::Item::Fn(f) if mangled_name.ends_with(&f.name.to_string())));
        let is_extern = ctx.external_decls().contains(mangled_name) && !is_user_fn && !ctx.defined_functions().contains(mangled_name);
        if mangled_name != "free" && mangled_name != "drop"
            && (is_extern || ctx.config.freeing_functions.contains(mangled_name)) {
                if let Some(Type::Pointer { .. }) = final_arg_tys.get(i) {
                    if let Some(var_name) = super::extract_ident_name(arg_expr) {
                        ctx.pointer_tracker.mark_optional(&var_name);
                    }
                }
            }
    }

    if !ensures.is_empty() {
        crate::codegen::verification::VerificationEngine::apply_postconditions(ctx, ensures, param_names, args_vec);
    }

    Ok((res_val, ret_ty.clone()))
}

pub(crate) fn handle_post_call_state(ctx: &mut LoweringContext, call_name: &str) {
    if call_name.contains("__empty") && call_name.contains("Ptr") {
        *ctx.pending_pointer_state = Some(crate::codegen::verification::PointerState::Empty);
    } else if (call_name.contains("__new") && call_name.contains("Box"))
        || ((call_name.contains("__alloc") || call_name.contains("__place")) && call_name.contains("Arena"))
        || call_name == "malloc" || call_name.ends_with("__malloc")
    {
        *ctx.pending_pointer_state = Some(crate::codegen::verification::PointerState::Valid);
    }
}
