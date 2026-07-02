use crate::grammar::{Stmt, SaltFor};
use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use std::collections::HashMap;

// Import reduction types from sibling module
use super::for_loop_reduction::*;

// REASON: all 8 params independently meaningful; bundling would obscure intent
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_affine_for_reduction(
    ctx: &mut LoweringContext,
    out: &mut String,
    f: &SaltFor,
    lb: i64,
    ub: i64,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
    var_name: &str,
    reduction: ReductionInfo,
) -> Result<bool, String> {
    use crate::codegen::expr::emit_expr;
    
    // Determine MLIR type for iter_args - now supports vector types!
    let mlir_ty = match &reduction.ty {
        Type::F32 => "f32".to_string(),
        Type::F64 => "f64".to_string(),
        Type::Concrete(name, _) if name == "Vector4f32" => "vector<4xf32>".to_string(),
        Type::Concrete(name, _) if name == "Vector8f32" => "vector<8xf32>".to_string(),
        Type::Concrete(name, _) if name == "Vector4f64" => "vector<4xf64>".to_string(),
        Type::Concrete(name, _) if name == "Vector16f32" => "vector<16xf32>".to_string(),
        Type::Struct(name) if name == "Vector4f32" => "vector<4xf32>".to_string(),
        Type::Struct(name) if name == "Vector8f32" => "vector<8xf32>".to_string(),
        Type::Struct(name) if name == "Vector4f64" => "vector<4xf64>".to_string(),
        Type::Struct(name) if name == "Vector16f32" => "vector<16xf32>".to_string(),
        _ => return Err(format!("Reduction accumulator must be f32, f64, or Vector type, got {:?}", reduction.ty)),
    };
    
    // Generate unique IDs
    let iv = format!("%iv_{}", ctx.next_id());
    let result_ssa = format!("%reduction_result_{}", ctx.next_id());
    let iter_acc = format!("%iter_acc_{}", ctx.next_id());
    
    // For alloca-based accumulators, the initial value must be loaded first
    let init_value_ssa = if reduction.is_alloca {
        let load_ssa = format!("%reduction_init_{}", ctx.next_id());
        out.push_str(&format!(
            "    {} = llvm.load {} : !llvm.ptr -> {}\n",
            load_ssa, reduction.init_ssa, mlir_ty
        ));
        load_ssa
    } else {
        reduction.init_ssa.clone()
    };
    
    // KeuOS Narrowing: Determine if i32 can be used for the body
    // scf.for requires index type for bounds
    let can_narrow = ub < 2_147_483_647 && lb >= 0;
    
    // Emit index type bound constants for scf.for (required by MLIR)
    let lb_ssa = format!("%lb_{}", ctx.next_id());
    let ub_ssa = format!("%ub_{}", ctx.next_id());
    let step_ssa = format!("%step_{}", ctx.next_id());
    out.push_str(&format!("    {} = arith.constant {} : index\n", lb_ssa, lb));
    out.push_str(&format!("    {} = arith.constant {} : index\n", ub_ssa, ub));
    out.push_str(&format!("    {} = arith.constant 1 : index\n", step_ssa));
    
    // Emit scf.for with iter_args (scf.for is more flexible than affine.for)
    // Pattern: %result = scf.for %i = lb to ub step 1 iter_args(%acc = %init) -> (type) { ... }
    out.push_str(&format!(
        "    {} = scf.for {} = {} to {} step {} iter_args({} = {}) -> ({}) {{\n",
        result_ssa, iv, lb_ssa, ub_ssa, step_ssa, iter_acc, init_value_ssa, mlir_ty
    ));
    
    // Enter affine context (still use this for nested optimizations)
    ctx.enter_affine_context();
    
    // Enable fast-math context for constant-bound reduction body
    // Matches the pattern already used in emit_scf_for_runtime_reduction.
    // Without this, LLVM cannot vectorize constant-bound reductions (e.g., for i in 0..128)
    ctx.emission.in_fast_math_reduction = true;
    
    // Narrow the IV inside the loop if possible
    let mut body_vars = local_vars.clone();
    if can_narrow {
        let iv_i32 = format!("%iv_i32_{}", ctx.next_id());
        out.push_str(&format!("    {} = arith.index_cast {} : index to i32\n", iv_i32, iv));
        body_vars.insert(var_name.to_string(), (Type::I32, LocalKind::SSA(iv_i32)));
    } else {
        let iv_i64 = format!("%iv_i64_{}", ctx.next_id());
        out.push_str(&format!("    {} = arith.index_cast {} : index to i64\n", iv_i64, iv));
        body_vars.insert(var_name.to_string(), (Type::I64, LocalKind::SSA(iv_i64)));
    }
    
    // Shadow the accumulator with the iter_args parameter
    // This means `acc` now refers to the register-resident iter_acc
    body_vars.insert(
        reduction.accumulator_var.clone(),
        (reduction.ty.clone(), LocalKind::SSA(iter_acc.clone()))
    );
    
    // For vector reductions, emit ALL statements up to and including the reduction
    // This handles multi-statement bodies like:
    // { let w_vec = vector_load(...); let x_vec = vector_load(...); acc = vector_fma(w_vec, x_vec, acc); }
    let stmts = &f.body.stmts;
    let update_idx = reduction.update_stmt_idx;
    
    // Emit statements before the reduction update
    for stmt in stmts.iter().take(update_idx) {
        crate::codegen::stmt::emit_stmt(ctx, out, stmt, &mut body_vars)?;
    }
    
    // Get the next value from the reduction statement
    let next_val = match &reduction.kind {
        ReductionKind::Add => {
            // Original: acc = acc + expr, so emit the RHS
            let stmt = &stmts[update_idx];
            let assign = match stmt {
                Stmt::Syn(syn::Stmt::Expr(syn::Expr::Assign(a), _)) => a,
                Stmt::Expr(syn::Expr::Assign(a), _) => a,
                _ => return Err("Reduction update must be an assignment".to_string()),
            };
            let (val, _) = emit_expr(ctx, out, assign.right.as_ref(), &mut body_vars, Some(&reduction.ty))?;
            val
        },
        ReductionKind::VectorFma => {
            // acc = vector_fma(a, b, acc) - emit the vector_fma call
            let stmt = &stmts[update_idx];
            let assign = match stmt {
                Stmt::Syn(syn::Stmt::Expr(syn::Expr::Assign(a), _)) => a,
                Stmt::Expr(syn::Expr::Assign(a), _) => a,
                _ => return Err("Vector FMA reduction must be an assignment".to_string()),
            };
            // The RHS is vector_fma(a, b, acc) which will use iter_acc for acc
            let (val, _) = emit_expr(ctx, out, assign.right.as_ref(), &mut body_vars, Some(&reduction.ty))?;
            val
        },
    };
    
    // Emit scf.yield with the new accumulator value
    out.push_str(&format!("      scf.yield {} : {}\n", next_val, mlir_ty));
    
    // Reset fast-math context after reduction body
    ctx.emission.in_fast_math_reduction = false;
    
    ctx.exit_affine_context();
    
    // Close scf.for
    out.push_str("    }\n");
    
    // For alloca-based accumulators, store the result back
    if reduction.is_alloca {
        out.push_str(&format!(
            "    llvm.store {}, {} : {}, !llvm.ptr\n",
            result_ssa, reduction.init_ssa, mlir_ty
        ));
    }
    
    // Update the original accumulator variable to point to the result.
    // ONLY for non-alloca accumulators — for alloca-based ones (let mut ss),
    // the result was already stored back to the alloca above, and subsequent
    // code (ss = ss / N) must read from the alloca to get the correct chain.
    // Setting SSA here for alloca-based accumulators breaks the reassignment
    // chain because emit_lvalue generates a spill without updating the SSA mapping.
    if !reduction.is_alloca {
        local_vars.insert(
            reduction.accumulator_var,
            (reduction.ty, LocalKind::SSA(result_ssa))
        );
    }
    
    Ok(false)
}
/// Register a for-loop induction variable with the Z3 solver and assert domain bounds.
/// Extracted to eliminate duplication across three for-loop emitting functions.
pub(crate) fn emit_z3_for_loop_bounds(
    ctx: &mut LoweringContext,
    var_name: &str,
    iter: &syn::Expr,
    local_vars: &HashMap<String, (Type, LocalKind)>,
) -> bool {
    if !ctx.config.no_verify {
        let z3_i = ctx.mk_var(var_name);
        ctx.symbolic_tracker.insert(var_name.to_string(), z3_i.clone());
        ctx.z3_solver.push();
        let z3_zero = ctx.mk_int(0);
        ctx.z3_solver.assert(&z3_i.ge(&z3_zero));
        if let syn::Expr::Range(r) = iter {
            if let Some(end_expr) = &r.end {
                if let Ok(z3_end) = crate::codegen::expr::translate_to_z3(ctx, end_expr, local_vars) { ctx.z3_solver.assert(&z3_i.lt(&z3_end)) }
            }
            if let Some(start_expr) = &r.start {
                if let Ok(z3_start) = crate::codegen::expr::translate_to_z3(ctx, start_expr, local_vars) { ctx.z3_solver.assert(&z3_i.ge(&z3_start)) }
            }
        }
        true
    } else {
        false
    }
}
/// Emit scf.for with iter_args for runtime-bound reduction patterns.
/// Unlike emit_affine_for_reduction which uses constant bounds, this works with
/// dynamic bounds like `for j in 0..cols` where `cols` is a runtime variable.
/// 
/// This enables the "Register Coronation" pattern: the accumulator lives in
/// a register (iter_args) instead of the stack, eliminating Store-to-Load-Forwarding
/// bottlenecks and enabling LLVM vectorization.
pub(crate) fn emit_scf_for_runtime_reduction(
    ctx: &mut LoweringContext,
    out: &mut String,
    f: &SaltFor,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
    var_name: &str,
    reduction: ReductionInfo,
) -> Result<bool, String> {
    use crate::codegen::expr::emit_expr;
    
    // Determine MLIR type for iter_args
    let mlir_ty = match &reduction.ty {
        Type::F32 => "f32".to_string(),
        Type::F64 => "f64".to_string(),
        Type::Concrete(name, _) if name == "Vector4f32" => "vector<4xf32>".to_string(),
        Type::Concrete(name, _) if name == "Vector8f32" => "vector<8xf32>".to_string(),
        Type::Concrete(name, _) if name == "Vector4f64" => "vector<4xf64>".to_string(),
        Type::Concrete(name, _) if name == "Vector16f32" => "vector<16xf32>".to_string(),
        Type::Struct(name) if name == "Vector4f32" => "vector<4xf32>".to_string(),
        Type::Struct(name) if name == "Vector8f32" => "vector<8xf32>".to_string(),
        Type::Struct(name) if name == "Vector4f64" => "vector<4xf64>".to_string(),
        Type::Struct(name) if name == "Vector16f32" => "vector<16xf32>".to_string(),
        _ => return Err(format!("Reduction accumulator must be f32, f64, or Vector type, got {:?}", reduction.ty)),
    };
    
    // Extract range bounds from the for-loop iterator
    let (start_expr, end_expr) = match &f.iter {
        syn::Expr::Range(r) => (&r.start, &r.end),
        _ => return Err("scf.for requires range iterator".to_string()),
    };
    
    // Emit start and end bounds as SSA values
    let (start_val_raw, start_ty) = if let Some(start) = start_expr {
        emit_expr(ctx, out, start, local_vars, None)?
    } else {
        let v = format!("%c0_{}", ctx.next_id());
        out.push_str(&format!("    {} = arith.constant 0 : index\n", v));
        (v, Type::Usize)
    };
    
    let (end_val_raw, end_ty) = if let Some(end) = end_expr {
        emit_expr(ctx, out, end, local_vars, None)?
    } else {
        return Err("scf.for requires finite upper bound".to_string());
    };
    
    // Convert bounds to index type for scf.for (required by MLIR)
    // Determine if the IV can be narrowed to i32 inside the loop
    let can_narrow = matches!(start_ty, Type::I32 | Type::U32) && 
                     matches!(end_ty, Type::I32 | Type::U32);
    
    let lb_ssa = format!("%lb_idx_{}", ctx.next_id());
    let ub_ssa = format!("%ub_idx_{}", ctx.next_id());
    let step_ssa = format!("%step_{}", ctx.next_id());
    
    // Cast start to index
    if start_ty == Type::Usize {
        // Already index, just copy
        out.push_str(&format!("    {} = arith.constant 0 : index\n", lb_ssa));
        out.push_str(&format!("    {} = arith.addi {}, {} : index\n", lb_ssa, start_val_raw, lb_ssa));
    } else {
        let start_mlir = start_ty.to_mlir_type(ctx)?;
        out.push_str(&format!("    {} = arith.index_cast {} : {} to index\n", lb_ssa, start_val_raw, start_mlir));
    }
    
    // Cast end to index
    if end_ty == Type::Usize {
        // Already index, just copy
        out.push_str(&format!("    {} = arith.constant 0 : index\n", ub_ssa));
        out.push_str(&format!("    {} = arith.addi {}, {} : index\n", ub_ssa, end_val_raw, ub_ssa));
    } else {
        let end_mlir = end_ty.to_mlir_type(ctx)?;
        out.push_str(&format!("    {} = arith.index_cast {} : {} to index\n", ub_ssa, end_val_raw, end_mlir));
    }
    
    // Step is always 1
    out.push_str(&format!("    {} = arith.constant 1 : index\n", step_ssa));
    
    // Generate unique IDs
    let iv = format!("%iv_{}", ctx.next_id());
    let result_ssa = format!("%reduction_result_{}", ctx.next_id());
    let iter_acc = format!("%iter_acc_{}", ctx.next_id());
    
    // For alloca-based accumulators, the initial value must be loaded first
    let init_value_ssa = if reduction.is_alloca {
        let load_ssa = format!("%reduction_init_{}", ctx.next_id());
        out.push_str(&format!(
            "    {} = llvm.load {} : !llvm.ptr -> {}\n",
            load_ssa, reduction.init_ssa, mlir_ty
        ));
        load_ssa
    } else {
        reduction.init_ssa.clone()
    };
    
    // Emit scf.for with iter_args
    out.push_str(&format!(
        "    {} = scf.for {} = {} to {} step {} iter_args({} = {}) -> ({}) {{\n",
        result_ssa, iv, lb_ssa, ub_ssa, step_ssa, iter_acc, init_value_ssa, mlir_ty
    ));
    
    // Narrow the IV inside the loop if possible
    let mut body_vars = local_vars.clone();
    let z3_iv_name = if can_narrow {
        let iv_i32 = format!("%iv_i32_{}", ctx.next_id());
        out.push_str(&format!("    {} = arith.index_cast {} : index to i32\n", iv_i32, iv));
        body_vars.insert(var_name.to_string(), (Type::I32, LocalKind::SSA(iv_i32.clone())));
        iv_i32
    } else {
        let iv_i64 = format!("%iv_i64_{}", ctx.next_id());
        out.push_str(&format!("    {} = arith.index_cast {} : index to i64\n", iv_i64, iv));
        body_vars.insert(var_name.to_string(), (Type::I64, LocalKind::SSA(iv_i64.clone())));
        iv_i64
    };
    
    // Shadow the accumulator with the iter_args parameter
    // This means `sum` now refers to the register-resident iter_acc
    body_vars.insert(
        reduction.accumulator_var.clone(),
        (reduction.ty.clone(), LocalKind::SSA(iter_acc.clone()))
    );

    // === Z3 HOARE LOGIC: For Loop Induction Variable Bounds ===
    let _z3_for_loop_active = emit_z3_for_loop_bounds(ctx, &z3_iv_name, &f.iter, &*local_vars);

    // Track loop upper bound for pointer bounds verification
    let ub_name = if let syn::Expr::Range(ref r) = f.iter {
        r.end.as_ref().and_then(|e| {
            if let syn::Expr::Path(p) = &**e { p.path.get_ident().map(|i| i.to_string()) }
            else { None }
        })
    } else { None };
    if let Some(ref name) = ub_name {
        crate::codegen::verification::loop_bounds::set_loop_bound_name(Some(name.clone()));
    }

    // Enable fast-math context for reduction body
    // Allows LLVM to reorder FP operations for vectorization
    ctx.emission.in_fast_math_reduction = true;

    // Emit statements before the reduction update
    let stmts = &f.body.stmts;
    let update_idx = reduction.update_stmt_idx;

    for stmt in stmts.iter().take(update_idx) {
        crate::codegen::stmt::emit_stmt(ctx, out, stmt, &mut body_vars)?;
    }

    // Get the next value from the reduction statement
    let next_val = {
        let stmt = &stmts[update_idx];
        let assign = match stmt {
            Stmt::Syn(syn::Stmt::Expr(syn::Expr::Assign(a), _)) => a,
            Stmt::Expr(syn::Expr::Assign(a), _) => a,
            _ => return Err("Reduction update must be an assignment".to_string()),
        };
        let (val, _) = emit_expr(ctx, out, assign.right.as_ref(), &mut body_vars, Some(&reduction.ty))?;
        val
    };

    // Emit scf.yield with the new accumulator value
    out.push_str(&format!("      scf.yield {} : {}\n", next_val, mlir_ty));
    out.push_str("    }\n");

    crate::codegen::verification::loop_bounds::set_loop_bound_name(None);

    // === Z3 HOARE LOGIC: Pop for-loop solver scope ===
    if _z3_for_loop_active {
        ctx.z3_solver.pop(1);
    }
    
    // Reset fast-math context after reduction body
    ctx.emission.in_fast_math_reduction = false;
    
    // For alloca-based accumulators, store the result back
    if reduction.is_alloca {
        out.push_str(&format!(
            "    llvm.store {}, {} : {}, !llvm.ptr\n",
            result_ssa, reduction.init_ssa, mlir_ty
        ));
    }
    
    // Update the original accumulator variable to point to the result.
    // ONLY for non-alloca accumulators — for alloca-based ones (let mut ss),
    // the result was already stored back to the alloca above, and subsequent
    // reassignments (ss = ss / N) must read from the alloca for correct chaining.
    if !reduction.is_alloca {
        local_vars.insert(
            reduction.accumulator_var,
            (reduction.ty, LocalKind::SSA(result_ssa))
        );
    }
    
    Ok(false)
}

