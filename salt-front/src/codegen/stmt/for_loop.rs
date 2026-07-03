use crate::grammar::SaltFor;
use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use crate::codegen::expr::emit_expr;
use crate::codegen::expr::emit_method_call;
use crate::codegen::type_bridge::promote_numeric;
use std::collections::HashMap;
use syn::spanned::Spanned;
// Import reduction types and functions from sibling module
use super::for_loop_reduction::*;
use super::for_loop_emit::*;
// Import analysis helpers
use super::analysis::{has_tensor_indexing, try_extract_const_int, block_has_control_flow};
/// Emit an scf.for loop with KeuOS Narrowing for constant-bound loops.
pub(crate) fn emit_affine_for(
    ctx: &mut LoweringContext,
    out: &mut String,
    f: &SaltFor,
    lb: i64,
    ub: i64,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<bool, String> {
    
    // Get loop variable name - Affine engine only accepts simple identifiers
    // Pat::Wild and complex patterns go through the Regular engine where RAII-Lite lives
    let var_name = if let syn::Pat::Ident(id) = &f.pat {
        id.ident.to_string()
    } else {
        return Err("Affine for-loop requires simple identifier pattern".to_string());
    };
    
    // Check if this is a reduction loop (sum = sum + expr pattern)
    // If so, iter_args can be emitted for register-resident accumulation
    if let Some(reduction_info) = detect_reduction_pattern(&f.body.stmts, local_vars) {
        return emit_affine_for_reduction(ctx, out, f, lb, ub, local_vars, &var_name, reduction_info);
    }
    
    // KeuOS Body Analysis: Detect loop intent from body contents
    // - Tensor indexing (A[i,j]) -> Use affine.for + Usize for polyhedral optimization
    // - Pointer arithmetic (ptr + offset) -> Use scf.for + i32 for instruction density
    let uses_tensor_indexing = has_tensor_indexing(&f.body.stmts);
    
    let iv = format!("%iv_{}", ctx.next_id());
    let mut body_vars = local_vars.clone();
    
    if uses_tensor_indexing {
        // ANALYTICAL PATH (MatMul): affine.for + Usize for polyhedral tiling
        out.push_str(&format!("    affine.for {} = {} to {} {{\n", iv, lb, ub));
        body_vars.insert(var_name.clone(), (Type::Usize, LocalKind::SSA(iv.clone())));
    } else {
        // PROCEDURAL PATH: Use scf.for with i32 for instruction density (window_access)
        let can_narrow = ub < 2_147_483_647 && lb >= 0;
        
        // Emit index type bounds for scf.for (required by MLIR)
        let lb_ssa = format!("%lb_{}", ctx.next_id());
        let ub_ssa = format!("%ub_{}", ctx.next_id());
        let step_ssa = format!("%step_{}", ctx.next_id());
        out.push_str(&format!("    {} = arith.constant {} : index\n", lb_ssa, lb));
        out.push_str(&format!("    {} = arith.constant {} : index\n", ub_ssa, ub));
        out.push_str(&format!("    {} = arith.constant 1 : index\n", step_ssa));
        
        out.push_str(&format!("    scf.for {} = {} to {} step {} {{\n", iv, lb_ssa, ub_ssa, step_ssa));
        
        // Narrow IV inside loop
        if can_narrow {
            let iv_i32 = format!("%iv_i32_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.index_cast {} : index to i32\n", iv_i32, iv));
            body_vars.insert(var_name.clone(), (Type::I32, LocalKind::SSA(iv_i32)));
        } else {
            let iv_i64 = format!("%iv_i64_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.index_cast {} : index to i64\n", iv_i64, iv));
            body_vars.insert(var_name.clone(), (Type::I64, LocalKind::SSA(iv_i64)));
        }
    }
    
    // Enter affine context for nested loops
    ctx.enter_affine_context();
    
    // Emit body
    let _body_diverges = super::emit_block(ctx, out, &f.body.stmts, &mut body_vars)?;
    
    ctx.exit_affine_context();
    
    // Close affine.for
    out.push_str("    }\n");
    
    Ok(false)
}
// ============================================================ SIMPLE SCF.FOR — Non-Reduction Runtime-Bound Loops
/// Emit scf.for for runtime-bound non-reduction loops.
/// This handles the common case of simple write loops like:
///   for i in 0..size { out[i] = expr }
/// which would otherwise fall to cf.br basic-block loops.
/// scf.for enables LLVM to see a clean loop structure for vectorization.
pub(crate) fn emit_scf_for_simple(
    ctx: &mut LoweringContext,
    out: &mut String,
    f: &SaltFor,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<bool, String> {
    
    // Get loop variable name
    let var_name = if let syn::Pat::Ident(id) = &f.pat {
        id.ident.to_string()
    } else {
        return Err("scf.for requires simple identifier pattern".to_string());
    };
    
    // Extract bounds from range expression
    let (start_expr, end_expr) = match &f.iter {
        syn::Expr::Range(r) => (&r.start, &r.end),
        _ => return Err("scf.for requires range expression".to_string()),
    };
    
    let (start_val, start_ty) = if let Some(start) = start_expr {
        emit_expr(ctx, out, start, local_vars, None)?
    } else {
        let v = format!("%c0_{}", ctx.next_id());
        ctx.emit_const_int(out, &v, 0, "i32");
        (v, Type::I32)
    };
    
    let (end_val, end_ty) = if let Some(end) = end_expr {
        emit_expr(ctx, out, end, local_vars, None)?
    } else {
        return Err("scf.for requires upper bound".to_string());
    };
    
    // Cast bounds to index type (required by scf.for)
    let lb_idx = format!("%lb_idx_{}", ctx.next_id());
    let ub_idx = format!("%ub_idx_{}", ctx.next_id());
    let step = format!("%step_{}", ctx.next_id());
    let start_mlir_ty = start_ty.to_mlir_type(ctx)?;
    let end_mlir_ty = end_ty.to_mlir_type(ctx)?;
    out.push_str(&format!("    {} = arith.index_cast {} : {} to index\n", lb_idx, start_val, start_mlir_ty));
    out.push_str(&format!("    {} = arith.index_cast {} : {} to index\n", ub_idx, end_val, end_mlir_ty));
    out.push_str(&format!("    {} = arith.constant 1 : index\n", step));
    
    // Generate unique IV
    let iv = format!("%iv_{}", ctx.next_id());
    
    // Emit scf.for (no iter_args — this is a side-effecting loop)
    out.push_str(&format!("    scf.for {} = {} to {} step {} {{\n", iv, lb_idx, ub_idx, step));
    
    // Cast IV to i64 inside loop body
    let iv_i64 = format!("%iv_i64_{}", ctx.next_id());
    out.push_str(&format!("    {} = arith.index_cast {} : index to i64\n", iv_i64, iv));
    
    // Set up body vars with loop variable
    let mut body_vars = local_vars.clone();
    body_vars.insert(var_name.clone(), (Type::I64, LocalKind::SSA(iv_i64.clone())));
    
    // Register the induction variable with Z3 and assert domain constraints.
    let _z3_for_loop_active = emit_z3_for_loop_bounds(ctx, &iv_i64, &f.iter, &*local_vars);

    // Track loop upper bound for pointer bounds verification
    let ub_name = if let syn::Expr::Range(ref r) = f.iter {
        r.end.as_ref().and_then(|e| {
            if let syn::Expr::Path(p) = &**e { p.path.get_ident().map(|i| i.to_string()) }
            else { None }
        })
    } else { None };
    if let Some(ref name) = ub_name {
        crate::codegen::verification::loop_bounds::push_loop_bound(name.clone());
    }

    // Verify for-loop invariants: concrete unrolling if bounds are constants
    let const_start = start_expr.as_ref().and_then(|e| try_extract_const_int(e));
    let const_end = end_expr.as_ref().and_then(|e| try_extract_const_int(e));
    let _loop_invariants = if _z3_for_loop_active {
        if let (Some(s), Some(e)) = (const_start, const_end) {
            crate::codegen::verification::array_tracker::prove_for_loop_concrete(
                ctx, &f.body.stmts, &body_vars, &iv_i64, s, e, &var_name,
            )?
        } else {
            prove_for_loop_invariants(ctx, &f.body.stmts, &body_vars, &iv_i64, start_expr)?
        }
    } else {
        Vec::new()
    };

    ctx.enter_affine_context();

    // Emit body
    let _body_diverges = super::emit_block(ctx, out, &f.body.stmts, &mut body_vars)?;

    // Bump array versions for indexed stores in the body (Z3 inductive step infra)
    crate::codegen::verification::array_tracker::process_array_stores_in_body(&f.body.stmts);

    // Inductive step: prove invariant(i) is preserved → invariant(i+1)
    super::for_loop_emit::check_inductive_step(ctx, &_loop_invariants, &var_name, &body_vars)?;

    ctx.exit_affine_context();

    crate::codegen::verification::loop_bounds::pop_loop_bound();

    if _z3_for_loop_active {
        ctx.z3_solver.pop(1);
    }
    
    // Close scf.for
    out.push_str("    }\n");
    
    Ok(false)
}
// ============================================================ FFB Saturated Loop Emission
/// Lower `for x in iter` to a while-loop with `.next()` calls.
///
/// Desugaring:
/// ```text
/// for x in iter_expr {
///     body
/// }
/// ```
/// becomes:
/// ```text
/// let mut _iter = iter_expr;
/// loop {
///     let _opt = _iter.next();
///     if _opt is None: break;
///     let x = _opt.payload;
///     body
/// }
/// ```
///
/// MLIR pattern:
///   1. Evaluate iterator → alloca (mutable state for .next() mutation)
///   2. Header: call .next() → Option<T> (tag=i32, payload=[N x i8])
///   3. Extract tag (extractvalue index 0), cmpi eq with 0 (None)
///   4. If None → exit; if Some → extract payload, bind, emit body, branch back
pub(crate) fn emit_iterator_for_loop(
    ctx: &mut LoweringContext,
    out: &mut String,
    f: &SaltFor,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<bool, String> {
    // 1. Evaluate the iterator expression once
    let (iter_val, iter_ty) = emit_expr(ctx, out, &f.iter, local_vars, None)?;
    // 2. Store iterator in alloca (it's mutable state — .next() modifies it)
    let iter_mlir_ty = iter_ty.to_mlir_storage_type(ctx)?;
    let iter_ptr = format!("%iter_ptr_{}", ctx.next_id());
    ctx.emit_alloca(out, &iter_ptr, &iter_mlir_ty);
    ctx.emit_store(out, &iter_val, &iter_ptr, &iter_mlir_ty);
    // Register the iterator in local_vars so emit_method_call can find it
    let iter_var_name = format!("__iter_{}", ctx.next_id());
    local_vars.insert(iter_var_name.clone(), (iter_ty.clone(), LocalKind::Ptr(iter_ptr.clone())));
    // 3. Create basic block labels
    let label_header = format!("iter_header_{}", ctx.next_id());
    let label_body = format!("iter_body_{}", ctx.next_id());
    let label_exit = format!("iter_exit_{}", ctx.next_id());
    out.push_str(&format!("    cf.br ^{}\n", label_header));
    out.push_str(&format!("  ^{}:\n", label_header));
    // Clear LVN cache at loop header entry
    ctx.emission.global_lvn.clear();
    // Heartbeat Injection 
    if !*ctx.no_yield() {
        ctx.emit_lto_hook(out, "__salt_yield_check", &[], local_vars, None)?;
    }
    // 4. Call .next() on the iterator
    //    Build a synthetic syn::ExprMethodCall to reuse existing method dispatch
    let iter_ident: syn::Expr = syn::parse_str(&iter_var_name)
        .map_err(|e| format!("Failed to parse iterator ident: {}", e))?;
    let method_call: syn::ExprMethodCall = syn::parse_quote! {
        #iter_ident.next()
    };
    let (next_result, next_ty) = emit_method_call(ctx, out, &method_call, local_vars, None)?;
    // 5. Extract tag from Option (discriminant at index 0)
    //    Option layout: { i32 (tag), [N x i8] (payload) }
    //    Look up the actual None discriminant from the enum registry
    let option_mlir_ty = next_ty.to_mlir_type(ctx)?;
    let tag_val = format!("%iter_tag_{}", ctx.next_id());
    ctx.emit_extractvalue(out, &tag_val, &next_result, 0, &option_mlir_ty);
    // Find the None discriminant from the enum registry
    let none_disc = {
        let mangled = next_ty.mangle_suffix();
        let registry = ctx.enum_registry();
        let info = registry.values()
            .find(|i| i.name == mangled || mangled.ends_with(&format!("__{}", i.name)) || i.name == "Option")
            .ok_or_else(|| format!("Cannot find Option enum in registry for {:?}", next_ty))?;
        info.variants.iter()
            .find(|(n, _, _)| n == "None")
            .map(|(_, _, disc)| *disc as i64)
            .unwrap_or(1) // Fallback: None is second variant (disc=1)
    };
    // Compare tag with None discriminant
    let none_const = format!("%iter_none_{}", ctx.next_id());
    let is_none = format!("%iter_is_none_{}", ctx.next_id());
    ctx.emit_const_int(out, &none_const, none_disc, "i32");
    out.push_str(&format!("    {} = arith.cmpi eq, {}, {} : i32\n", is_none, tag_val, none_const));
    // Branch: None → exit, Some → body
    out.push_str(&format!("    cf.cond_br {}, ^{}, ^{}\n", is_none, label_exit, label_body));
    // 6. Body block: extract payload and bind to loop variable
    out.push_str(&format!("  ^{}:\n", label_body));
    // Determine the payload type from the Option's inner type
    let payload_ty = match &next_ty {
        Type::Enum(name) => {
            // Look up the enum in the registry to find the Some variant's payload type
            let info = ctx.enum_registry().values()
                .find(|i| i.name == *name || name.ends_with(&format!("__{}", i.name)))
                .cloned()
                .ok_or_else(|| format!("Cannot find enum '{}' in registry", name))?;
            let (_vname, payload, _disc) = info.variants.iter()
                .find(|(n, _, _)| n == "Some")
                .ok_or_else(|| format!("Enum '{}' has no 'Some' variant", name))?;
            let inner = payload.clone()
                .ok_or_else(|| "Option 'Some' variant has no payload type".to_string())?;
            (inner, info.max_payload_size)
        },
        Type::Concrete(base, args) => {
            // For monomorphized Option<T>, try to resolve via registry or infer from args
            let mangled = next_ty.mangle_suffix();
            let info = ctx.enum_registry().values()
                .find(|i| i.name == mangled || i.name == *base)
                .cloned();
            if let Some(info) = info {
                let (_vname, payload, _disc) = info.variants.iter()
                    .find(|(n, _, _)| n == "Some")
                    .ok_or_else(|| "Enum has no 'Some' variant".to_string())?;
                let inner = payload.clone()
                    .ok_or_else(|| "Option 'Some' has no payload".to_string())?;
                (inner, info.max_payload_size)
            } else if !args.is_empty() {
                // Fallback: use the first generic arg as the payload type
                // For Option<i64>, max_payload_size is 8
                let inner = args[0].clone();
                let size = 8usize; // i64 = 8 bytes
                (inner, size)
            } else {
                return Err(format!("Cannot determine payload type for {:?}", next_ty));
            }
        },
        _ => return Err(format!("next() must return Option<T>, got {:?}", next_ty)),
    };
    let (inner_ty, max_payload_size) = payload_ty;
    // Extract the payload byte array from the Option (index 1)
    let payload_array = format!("%iter_payload_{}", ctx.next_id());
    ctx.emit_extractvalue(out, &payload_array, &next_result, 1, &option_mlir_ty);
    // Store the byte array to memory and load as the correct type
    let array_mlir_ty = format!("!llvm.array<{} x i8>", max_payload_size);
    let buf_ptr = format!("%iter_buf_{}", ctx.next_id());
    ctx.emit_alloca(out, &buf_ptr, &array_mlir_ty);
    ctx.emit_store(out, &payload_array, &buf_ptr, &array_mlir_ty);
    let payload_val = format!("%iter_val_{}", ctx.next_id());
    let inner_mlir_ty = inner_ty.to_mlir_type(ctx)?;
    ctx.emit_load(out, &payload_val, &buf_ptr, &inner_mlir_ty);
    // 7. Bind the payload to the loop variable pattern
    let mut body_vars = local_vars.clone();
    if let syn::Pat::Ident(id) = &f.pat {
        let name = id.ident.to_string();
        body_vars.insert(name, (inner_ty.clone(), LocalKind::SSA(payload_val.clone())));
    } else if let syn::Pat::Wild(_) = &f.pat {
        // Wildcard — don't bind
    } else {
        // For more complex patterns, use emit_pattern
        super::emit_pattern(
            ctx, out, &f.pat, payload_val.clone(), inner_ty.clone(), inner_ty.clone(), &mut body_vars
        )?;
    }
    // 8. Emit the loop body
    ctx.break_labels_mut().push(label_exit.clone());
    ctx.continue_labels_mut().push(label_header.clone());
    ctx.push_cleanup_scope();
    let body_diverges = super::emit_block(ctx, out, &f.body.stmts, &mut body_vars)?;
    ctx.break_labels_mut().pop();
    ctx.continue_labels_mut().pop();
    if !body_diverges {
        ctx.pop_and_emit_cleanup(out)?;
        out.push_str(&format!("    cf.br ^{}\n", label_header));
    } else {
        let _ = ctx.cleanup_stack_mut().pop();
    }
    // 9. Exit block
    ctx.emission.global_lvn.clear();
    out.push_str(&format!("  ^{}:\n", label_exit));
    // Clean up the temporary iterator variable
    local_vars.remove(&iter_var_name);
    Ok(false)
}
pub(crate) fn emit_for_stmt(ctx: &mut LoweringContext, out: &mut String, f: &crate::grammar::SaltFor, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<bool, String>  {
    let (start_expr, end_expr) = match &f.iter {
        syn::Expr::Range(r) => (&r.start, &r.end),
        _ => return emit_iterator_for_loop(ctx, out, f, local_vars),
    };
    
    let const_start = start_expr.as_ref().and_then(|e| try_extract_const_int(e));
    let const_end = end_expr.as_ref().and_then(|e| try_extract_const_int(e));
    let is_simple_ident = matches!(&f.pat, syn::Pat::Ident(_));
    let body_has_cf = block_has_control_flow(&f.body.stmts);
    
    if is_simple_ident {
        if let (Some(lb), Some(ub)) = (const_start, const_end) {
            if !body_has_cf {
                return emit_affine_for(ctx, out, f, lb, ub, local_vars);
            }
        }
    }
    
    if is_simple_ident && !body_has_cf {
        if let Some(reduction_info) = detect_reduction_pattern(&f.body.stmts, local_vars) {
            if let syn::Pat::Ident(id) = &f.pat {
                return emit_scf_for_runtime_reduction(ctx, out, f, local_vars, &id.ident.to_string(), reduction_info);
            }
        }
        return emit_scf_for_simple(ctx, out, f, local_vars);
    }
    
    emit_cf_br_for_loop(ctx, out, f, start_expr.as_deref(), end_expr.as_deref(), local_vars)
}
pub(crate) fn emit_cf_br_for_loop(ctx: &mut LoweringContext, out: &mut String, f: &crate::grammar::SaltFor, start_expr: Option<&syn::Expr>, end_expr: Option<&syn::Expr>, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<bool, String> {
    let label_header = format!("for_header_{}", ctx.next_id());
    let label_body = format!("for_body_{}", ctx.next_id());
    let label_exit = format!("for_exit_{}", ctx.next_id());
    let (start_val_raw, start_ty) = if let Some(start) = start_expr {
        emit_expr(ctx, out, start, local_vars, None)?
    } else {
        let v = format!("%c0_{}", ctx.next_id());
        ctx.emit_const_int(out, &v, 0, "i32");
        (v, Type::I32)
    };
    
    let (end_val_raw, end_ty) = if let Some(end) = end_expr {
        emit_expr(ctx, out, end, local_vars, None)?
    } else {
        return Err("Infinite for-loops not supported yet".to_string());
    };
    let loop_ty = if start_ty == Type::I64 || end_ty == Type::I64 || start_ty == Type::Usize || end_ty == Type::Usize {
        Type::I64 
    } else {
        Type::I32
    };
    let start_val = promote_numeric(ctx, out, &start_val_raw, &start_ty, &loop_ty)?;
    let end_val = promote_numeric(ctx, out, &end_val_raw, &end_ty, &loop_ty)?;
    let mlir_loop_ty = loop_ty.to_mlir_type(ctx)?;
    let loop_var_ptr = format!("%for_var_ptr_{}", ctx.next_id());
    ctx.emit_alloca(out, &loop_var_ptr, &mlir_loop_ty);
    ctx.emit_store(out, &start_val, &loop_var_ptr, &mlir_loop_ty);
    out.push_str(&format!("    cf.br ^{}\n", label_header));
    out.push_str(&format!("  ^{}:\n", label_header));
    
    let current_i = format!("%i_{}", ctx.next_id());
    ctx.emit_load(out, &current_i, &loop_var_ptr, &mlir_loop_ty);
    
    let cond_i1 = format!("%for_cond_{}", ctx.next_id());
    ctx.emit_cmp(out, &cond_i1, "arith.cmpi", "slt", &current_i, &end_val, &mlir_loop_ty);
    let loc = ctx.loc_tag(f.iter.span());
    out.push_str(&format!("    cf.cond_br {}, ^{}, ^{}{}\n", cond_i1, label_body, label_exit, loc));
    
    out.push_str(&format!("  ^{}:\n", label_body));
    
    ctx.emission.global_lvn.clear();
    if !*ctx.no_yield() {
        ctx.emit_lto_hook(out, "__salt_yield_check", &[], local_vars, None)?;
    }
    
    let mut body_vars = local_vars.clone();
    if let syn::Pat::Ident(id) = &f.pat {
        body_vars.insert(id.ident.to_string(), (loop_ty.clone(), LocalKind::SSA(current_i.clone())));
    }
    
    let _z3_for_loop_active = if matches!(&f.pat, syn::Pat::Ident(_)) || matches!(&f.pat, syn::Pat::Wild(_)) {
        emit_z3_for_loop_bounds(ctx, &current_i, &f.iter, &*local_vars)
    } else {
        false
    };

    // Track loop upper bound for pointer bounds verification
    let ub_name = if let syn::Expr::Range(ref r) = f.iter {
        r.end.as_ref().and_then(|e| {
            if let syn::Expr::Path(p) = &**e { p.path.get_ident().map(|i| i.to_string()) }
            else { None }
        })
    } else { None };
    if let Some(ref name) = ub_name {
        crate::codegen::verification::loop_bounds::push_loop_bound(name.clone());
    }

    // Verify for-loop invariants: concrete unrolling if bounds are constants
    let const_start = start_expr.and_then(try_extract_const_int);
    let const_end = end_expr.and_then(try_extract_const_int);
    let loop_invariants = if _z3_for_loop_active {
        if let (Some(s), Some(e)) = (const_start, const_end) {
            let var_name = if let syn::Pat::Ident(id) = &f.pat { id.ident.to_string() } else { String::new() };
            crate::codegen::verification::array_tracker::prove_for_loop_concrete(
                ctx, &f.body.stmts, &body_vars, &current_i, s, e, &var_name,
            )?
        } else {
            let start_boxed: Option<Box<syn::Expr>> = start_expr.map(|e| Box::new(e.clone()));
            super::for_loop_emit::prove_for_loop_invariants(ctx, &f.body.stmts, &body_vars, &current_i, &start_boxed)?
        }
    } else { Vec::new() };

    ctx.break_labels_mut().push(label_exit.clone());
    ctx.continue_labels_mut().push(label_header.clone());
    ctx.push_cleanup_scope();

    let body_diverges = super::emit_block(ctx, out, &f.body.stmts, &mut body_vars)?;
    ctx.break_labels_mut().pop();
    ctx.continue_labels_mut().pop();

    // Array store tracking + inductive step
    crate::codegen::verification::array_tracker::process_array_stores_in_body(&f.body.stmts);
    let var_name = if let syn::Pat::Ident(id) = &f.pat { id.ident.to_string() } else { String::new() };
    super::for_loop_emit::check_inductive_step(ctx, &loop_invariants, &var_name, &body_vars)?;

    crate::codegen::verification::loop_bounds::pop_loop_bound();

    if _z3_for_loop_active {
        ctx.z3_solver.pop(1);
    }
    
    if !body_diverges {
         ctx.pop_and_emit_cleanup(out)?;
         let next_i = format!("%next_i_{}", ctx.next_id());
         let c1 = format!("%c1_{}", ctx.next_id());
         ctx.emit_const_int(out, &c1, 1, &mlir_loop_ty);
         ctx.emit_binop(out, &next_i, "arith.addi", &current_i, &c1, &mlir_loop_ty);
         ctx.emit_store(out, &next_i, &loop_var_ptr, &mlir_loop_ty);
         out.push_str(&format!("    cf.br ^{}\n", label_header));
    } else {
         let _ = ctx.cleanup_stack_mut().pop();
    }
    
    ctx.emission.global_lvn.clear();
    out.push_str(&format!("  ^{}:\n", label_exit));
    Ok(false)
}
