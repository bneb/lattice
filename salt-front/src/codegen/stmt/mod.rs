use crate::grammar::{Stmt, SaltBlock, SaltElse, SaltFor, SaltIf, SaltMatch, LetElse};
use crate::grammar::pattern::Pattern;
use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use crate::codegen::expr::emit_expr;
use crate::codegen::type_bridge::{resolve_type, promote_numeric};
use std::collections::HashMap;
use syn::spanned::Spanned;
pub mod analysis;
use self::analysis::*;
pub mod helpers;
pub(crate) use self::helpers::*;

/// Emit an scf.for loop with KeuOS Narrowing for constant-bound loops.
/// Source-Level IV Narrowing: Use i32 when bounds fit, eliminating index_cast overhead.
/// This maintains LLVM's ability to optimize while reducing per-iteration instruction count.
fn emit_affine_for(
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
    let _body_diverges = emit_block(ctx, out, &f.body.stmts, &mut body_vars)?;
    
    ctx.exit_affine_context();
    
    // Close affine.for
    out.push_str("    }\n");
    
    Ok(false)
}

/// Information about a detected reduction pattern
struct ReductionInfo {
    /// Name of the accumulator variable (e.g., "sum" or "acc")
    accumulator_var: String,
    /// Initial value SSA name - for Alloca, this is the pointer; for SSA, this is the value
    init_ssa: String,
    /// Type of the accumulator
    ty: Type,
    /// True if the accumulator is an alloca (mut variable), requiring load/store wrapper
    is_alloca: bool,
    /// Kind of reduction: Simple (acc + expr), FMA (vector_fma(a, b, acc))
    kind: ReductionKind,
    /// Statement index where the reduction update occurs (for multi-statement bodies)
    update_stmt_idx: usize,
}

/// Kind of reduction operation
#[derive(Clone, Debug)]
enum ReductionKind {
    /// Simple binary: acc = acc + expr or acc = acc - expr
    Add,
    /// FMA intrinsic: acc = vector_fma(a, b, acc)
    VectorFma,
}

/// Detect if the loop body is a simple reduction pattern: `acc = acc + expr`
/// Returns Some(info) if detected, None otherwise.
///
/// Supports multi-statement bodies where the last assignment is the reduction
/// update and preceding statements are let-bindings (loads, temporaries).
/// This handles patterns like rmsnorm's:
///   `for i in 0..n { let v = x[i]; ss = ss + v * v; }`
fn detect_reduction_pattern(
    stmts: &[Stmt],
    local_vars: &HashMap<String, (Type, LocalKind)>,
) -> Option<ReductionInfo> {
    // First, try vector reduction (multi-statement support)
    if let Some(info) = detect_vector_reduction_pattern(stmts, local_vars) {
        return Some(info);
    }
    
    // Fall back to scalar reduction — now supports multi-statement bodies.
    // Scan from the END to find the reduction update statement.
    // All preceding statements must be let-bindings (safe setup).
    if stmts.is_empty() {
        return None;
    }
    
    // Find the reduction update: scan backwards for `acc = acc + expr`
    let mut update_idx = None;
    for (idx, stmt) in stmts.iter().enumerate().rev() {
        let assign = match stmt {
            Stmt::Syn(syn::Stmt::Expr(syn::Expr::Assign(a), _)) => a,
            Stmt::Expr(syn::Expr::Assign(a), _) => a,
            _ => continue,
        };
        
        // LHS must be a simple identifier (the accumulator)
        let acc_name = match assign.left.as_ref() {
            syn::Expr::Path(p) if p.path.segments.len() == 1 => {
                p.path.segments[0].ident.to_string()
            }
            _ => continue,
        };
        
        // RHS must be: acc + <expr> or acc - <expr>
        let rhs_binary = match assign.right.as_ref() {
            syn::Expr::Binary(b) => b,
            _ => continue,
        };
        
        // LHS of binary must be the same accumulator
        let lhs_is_acc = match rhs_binary.left.as_ref() {
            syn::Expr::Path(p) if p.path.segments.len() == 1 => {
                p.path.segments[0].ident == acc_name
            }
            _ => false,
        };
        
        if !lhs_is_acc {
            continue;
        }
        
        // Must be + or - (common reduction ops)
        let is_add_or_sub = matches!(rhs_binary.op, 
            syn::BinOp::Add(_) | syn::BinOp::AddAssign(_) | 
            syn::BinOp::Sub(_) | syn::BinOp::SubAssign(_)
        );
        
        if !is_add_or_sub {
            continue;
        }
        
        // Verify all preceding statements are let-bindings (safe setup)
        let all_preceding_are_lets = stmts[..idx].iter().all(|s| {
            matches!(s, 
                Stmt::Syn(syn::Stmt::Local(_)) | 
                Stmt::LetElse(_)
            )
        });
        
        if !all_preceding_are_lets {
            continue;
        }
        
        // Accumulator must be a scalar f32 or f64 local var
        if let Some((ty, kind)) = local_vars.get(&acc_name) {
            if matches!(ty, Type::F32 | Type::F64) {
                let (init_ssa, is_alloca) = match kind {
                    LocalKind::SSA(s) => (s.clone(), false),
                    LocalKind::Ptr(ptr) => (ptr.clone(), true),
                };
                update_idx = Some((idx, acc_name, ty.clone(), init_ssa, is_alloca));
                break;
            }
        }
    }
    
    let (idx, acc_name, ty, init_ssa, is_alloca) = update_idx?;
    
    Some(ReductionInfo {
        accumulator_var: acc_name,
        init_ssa,
        ty,
        is_alloca,
        kind: ReductionKind::Add,
        update_stmt_idx: idx,
    })
}

/// Detect vector reduction patterns in multi-statement loop bodies.
/// Specifically looks for: `acc = vector_fma(a, b, acc)` where acc is a vector type.
/// 
/// Supports loops like:
/// ```salt
/// for v in 0..98 {
///     let w_vec = vector_load(w_ptr + offset);
///     let x_vec = vector_load(x_ptr + offset); 
///     acc = vector_fma(w_vec, x_vec, acc);
/// }
/// ```
fn detect_vector_reduction_pattern(
    stmts: &[Stmt],
    local_vars: &HashMap<String, (Type, LocalKind)>,
) -> Option<ReductionInfo> {
    // We're looking for a vector_fma call that updates an accumulator
    // The last statement should be the reduction update
    
    for (idx, stmt) in stmts.iter().enumerate() {
        // Look for: acc = vector_fma(a, b, acc)
        let assign = match stmt {
            Stmt::Syn(syn::Stmt::Expr(syn::Expr::Assign(a), _)) => a,
            Stmt::Expr(syn::Expr::Assign(a), _) => a,
            _ => continue,
        };
        
        // LHS must be a simple identifier (the accumulator)
        let acc_name = match assign.left.as_ref() {
            syn::Expr::Path(p) if p.path.segments.len() == 1 => {
                p.path.segments[0].ident.to_string()
            }
            _ => continue,
        };
        
        // RHS must be a function call to vector_fma
        let call = match assign.right.as_ref() {
            syn::Expr::Call(c) => c,
            _ => continue,
        };
        
        // Function name must be vector_fma
        let func_name = match call.func.as_ref() {
            syn::Expr::Path(p) if p.path.segments.len() == 1 => {
                p.path.segments[0].ident.to_string()
            }
            _ => continue,
        };
        
        if func_name != "vector_fma" && func_name != "v_fma" {
            continue;
        }
        
        // vector_fma(a, b, acc) - third arg must be the same accumulator
        // v_fma(acc, a, b) - first arg must be the same accumulator
        if call.args.len() != 3 {
            continue;
        }
        
        let acc_arg_idx = if func_name == "v_fma" { 0 } else { 2 };
        
        let acc_arg_is_acc = match &call.args[acc_arg_idx] {
            syn::Expr::Path(p) if p.path.segments.len() == 1 => {
                p.path.segments[0].ident == acc_name
            }
            _ => false,
        };
        
        if !acc_arg_is_acc {
            continue;
        }
        
        // Found a vector_fma reduction! Get type info
        let (ty, kind) = local_vars.get(&acc_name)?;
        
        let (init_ssa, is_alloca) = match kind {
            LocalKind::SSA(s) => (s.clone(), false),
            LocalKind::Ptr(ptr) => (ptr.clone(), true),
        };
        
        // Must be a vector type
        let is_vector_type = matches!(ty, 
            Type::Concrete(name, _) if name.starts_with("Vector")
        ) || matches!(ty,
            Type::Struct(name) if name.starts_with("Vector")
        );
        
        if !is_vector_type {
            continue;
        }
        
        return Some(ReductionInfo {
            accumulator_var: acc_name,
            init_ssa,
            ty: ty.clone(),
            is_alloca,
            kind: ReductionKind::VectorFma,
            update_stmt_idx: idx,
        });
    }
    
    None
}

/// Emit an scf.for with iter_args for reduction patterns.
/// This keeps the accumulator in a register instead of memory.
/// 
/// Pattern: `for j in 0..K { acc = vector_fma(a, b, acc); }`
/// Becomes: `%result = scf.for %j = 0 to K iter_args(%acc = %init) -> (vector<8xf32>) { ... scf.yield %next }`
/// 
/// Upgrade: Now uses scf.for instead of affine.for for better compatibility with
/// multi-statement bodies containing vector operations.
#[allow(clippy::too_many_arguments)] // REASON: all 8 params independently meaningful; bundling would obscure intent
fn emit_affine_for_reduction(
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
        emit_stmt(ctx, out, stmt, &mut body_vars)?;
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
fn emit_z3_for_loop_bounds(
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
fn emit_scf_for_runtime_reduction(
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
    
    // Enable fast-math context for reduction body
    // Allows LLVM to reorder FP operations for vectorization
    ctx.emission.in_fast_math_reduction = true;
    
    // Emit statements before the reduction update
    let stmts = &f.body.stmts;
    let update_idx = reduction.update_stmt_idx;
    
    for stmt in stmts.iter().take(update_idx) {
        emit_stmt(ctx, out, stmt, &mut body_vars)?;
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

// ============================================================================
// SIMPLE SCF.FOR — Non-Reduction Runtime-Bound Loops
// ============================================================================

/// Emit scf.for for runtime-bound non-reduction loops.
/// This handles the common case of simple write loops like:
///   for i in 0..size { out[i] = expr }
/// which would otherwise fall to cf.br basic-block loops.
/// scf.for enables LLVM to see a clean loop structure for vectorization.
fn emit_scf_for_simple(
    ctx: &mut LoweringContext,
    out: &mut String,
    f: &SaltFor,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<bool, String> {
    use crate::codegen::expr::emit_expr;
    
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
    
    // === Z3 HOARE LOGIC: For Loop Induction Variable Bounds ===
    // Register the induction variable with Z3 and assert domain constraints.
    let _z3_for_loop_active = emit_z3_for_loop_bounds(ctx, &iv_i64, &f.iter, &*local_vars);
    
    ctx.enter_affine_context();
    
    // Emit body
    let _body_diverges = emit_block(ctx, out, &f.body.stmts, &mut body_vars)?;
    
    ctx.exit_affine_context();
    
    // === Z3 HOARE LOGIC: Pop for-loop solver scope ===
    if _z3_for_loop_active {
        ctx.z3_solver.pop(1);
    }
    
    // Close scf.for
    out.push_str("    }\n");
    
    Ok(false)
}

// ============================================================================
// FFB Saturated Loop Emission
// ============================================================================

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
fn emit_iterator_for_loop(
    ctx: &mut LoweringContext,
    out: &mut String,
    f: &SaltFor,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<bool, String> {
    use crate::codegen::expr::emit_method_call;

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
        crate::codegen::stmt::emit_pattern(
            ctx, out, &f.pat, payload_val.clone(), inner_ty.clone(), inner_ty.clone(), &mut body_vars
        )?;
    }

    // 8. Emit the loop body
    ctx.break_labels_mut().push(label_exit.clone());
    ctx.continue_labels_mut().push(label_header.clone());
    ctx.push_cleanup_scope();

    let body_diverges = emit_block(ctx, out, &f.body.stmts, &mut body_vars)?;
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

fn salt_if_always_returns(f: &crate::grammar::SaltIf) -> bool {
    let Some(else_branch) = &f.else_branch else { return false; };
    salt_block_always_returns(&f.then_branch.stmts) && salt_else_always_returns(else_branch.as_ref())
}

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



fn salt_else_always_returns(else_branch: &SaltElse) -> bool {
    match else_branch {
        SaltElse::Block(b) => salt_block_always_returns(&b.stmts),
        SaltElse::If(nested) => salt_block_always_returns(&nested.then_branch.stmts),
    }
}

pub fn emit_block(ctx: &mut LoweringContext, out: &mut String, stmts: &[Stmt], local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<bool, String> {
    // 1. Preamble Pass: Hoist all allocas to function entry
    hoist_allocas_in_block(ctx, stmts, local_vars)?;

    let mut emitted_terminator = false;
    let mut pushed_guards: usize = 0;
    for stmt in stmts {
        if emit_stmt(ctx, out, stmt, local_vars)? {
            emitted_terminator = true;
            break;
        }

        // Implicit Guard Negation for path-sensitive postcondition verification.
        // After `if cond { return ...; }` (no else), remaining code runs under `!cond`.
        if let Stmt::If(f) = stmt {
            if f.else_branch.is_none() && salt_block_always_returns(&f.then_branch.stmts) {
                let negated_cond = syn::Expr::Unary(syn::ExprUnary {
                    attrs: vec![],
                    op: syn::UnOp::Not(syn::token::Not::default()),
                    expr: Box::new(f.cond.clone()),
                });
                ctx.emission.path_conditions.push(negated_cond);
                pushed_guards += 1;
            }
        }
    }

    // Clean up implicit guards when exiting block scope
    for _ in 0..pushed_guards {
        ctx.emission.path_conditions.pop();
    }

    // If block is empty and not terminated, it must have at least one instruction
    // or a branch to merge to be MLIR-valid.
    Ok(emitted_terminator)
}

fn hoist_allocas_if_block(ctx: &mut LoweringContext, f: &SaltIf, local_vars: &HashMap<String, (Type, LocalKind)>) -> Result<(), String> {
    let mut then_vars = local_vars.clone();
    hoist_allocas_in_block(ctx, &f.then_branch.stmts, &mut then_vars)?;
    let Some(eb) = &f.else_branch else { return Ok(()); };
    let mut else_vars = local_vars.clone();
    hoist_allocas_in_else_branch(ctx, eb.as_ref(), &mut else_vars)
}

fn hoist_allocas_in_block(ctx: &mut LoweringContext, stmts: &[Stmt], local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<(), String> {
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
                            // HEURISTIC: Try to infer type from init expression ONLY if it's a simple literal or known variable.
                            // In a real compiler, a full type inference pass would be done.
                            // Salt prefers explicit types or well-behaved inference.
                            // emit_stmt handles inferring and hoisting if this is skipped here.
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

pub fn emit_stmt(ctx: &mut LoweringContext, out: &mut String, stmt: &Stmt, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<bool, String> {
    match stmt {
        Stmt::Syn(s) => match s {
            syn::Stmt::Local(local) => emit_local_stmt(ctx, out, local, local_vars),
            syn::Stmt::Expr(e, semi) => {
                let (val, _) = emit_expr(ctx, out, e, local_vars, None)?;
                let is_return = matches!(e, syn::Expr::Return(_));
                Ok((semi.is_none() && val == "%unreachable") || is_return)
            }
            // Handle macro statements
            // syn parses `macro!(...);` at statement position as Stmt::Macro,
            // not Stmt::Expr(Expr::Macro). Route through emit_expr for handling
            // by the macro dispatch logic (e.g., __fstring_append_expr!).
            syn::Stmt::Macro(ref sm) => {
                let expr_macro = syn::ExprMacro {
                    attrs: sm.attrs.clone(),
                    mac: sm.mac.clone(),
                };
                let (_, _) = emit_expr(ctx, out, &syn::Expr::Macro(expr_macro), local_vars, None)?;
                Ok(false)
            }
            _ => Ok(false),
        },
        Stmt::While(w) => emit_while_stmt(ctx, out, w, local_vars),
        Stmt::If(f) => {
            emit_salt_if(ctx, out, &f.cond, &f.then_branch, &f.else_branch, local_vars)
        }
        Stmt::For(f) => emit_for_stmt(ctx, out, f, local_vars),
        Stmt::MapWindow { addr, size: _, region, body } => {
            let (_addr_val, _addr_ty) = emit_expr(ctx, out, addr, local_vars, None)?;
            let packed_win_var = format!("%packed_win_{}", ctx.next_id());
            
            let mut inner_vars = local_vars.clone();
            let win_ty = Type::Window(Box::new(Type::U8), region.to_string());
            inner_vars.insert(region.to_string(), (win_ty, LocalKind::SSA(packed_win_var)));

            ctx.region_stack_mut().push(region.to_string());
            emit_block(ctx, out, &body.stmts, &mut inner_vars)?;
            ctx.region_stack_mut().pop();
            Ok(false)
        }
        Stmt::Move(expr) => {
             if let syn::Expr::Path(p) = expr {
                 let name = p.path.get_ident().map(|id| id.to_string()).unwrap_or_default();
                 ctx.consumed_vars_mut().insert(name.clone());
                 ctx.consumption_locs_mut().insert(name, "explicit move".to_string());
             }
             Ok(false)
        }
        Stmt::Return(opt_expr) => emit_return_stmt(ctx, out, opt_expr, local_vars),
        Stmt::Expr(expr, _) => {
            let (val, _) = emit_expr(ctx, out, expr, local_vars, None)?;
            Ok(val == "%unreachable")
        }
        Stmt::Invariant(e) => {
            let (cond, _) = emit_expr(ctx, out, e, local_vars, None)?;
            // Lower loop invariant to standard MLIR runtime assertion.
            // Uses scf.if (not cf.cond_br) because invariants live inside
            // loop bodies that may use affine.for or scf.for.
            let true_const = format!("%inv_true_{}", ctx.next_id());
            let violated = format!("%inv_violated_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.constant true\n", true_const));
            out.push_str(&format!("    {} = arith.xori {}, {} : i1\n", violated, cond, true_const));
            ctx.ensure_external_declaration("__salt_contract_violation", &[], &Type::Unit)?;
            out.push_str(&format!("    scf.if {} {{\n", violated));
            out.push_str("      func.call @__salt_contract_violation() : () -> ()\n");
            out.push_str("      scf.yield\n");
            out.push_str("    }\n");
            Ok(false)
        }
        Stmt::Unsafe(block) => emit_unsafe_stmt(ctx, out, block, local_vars),
        Stmt::DynamicCheck(block) => emit_dynamic_check_stmt(ctx, out, block, local_vars),
        Stmt::WithRegion { region, body } => {
            ctx.region_stack_mut().push(region.to_string());
            let mut inner_vars = local_vars.clone();
            let res = emit_block(ctx, out, &body.stmts, &mut inner_vars)?;
            ctx.region_stack_mut().pop();
            Ok(res)
        }
        Stmt::Break => {
            let label = ctx.break_labels().last().ok_or("Break outside of loop")?.clone();
            out.push_str(&format!("    cf.br ^{}\n", label));
            Ok(true)
        }
        Stmt::Continue => {
            let label = ctx.continue_labels().last().ok_or("Continue outside of loop")?.clone();
            out.push_str(&format!("    cf.br ^{}\n", label));
            Ok(true)
        }
        Stmt::Match(match_expr) => {
            emit_match(ctx, out, match_expr, local_vars)
        }
        Stmt::LetElse(let_else) => {
            emit_let_else(ctx, out, let_else, local_vars)
        }
        Stmt::Loop(body) => emit_loop_stmt(ctx, out, body, local_vars),
    }
}

fn emit_local_stmt(ctx: &mut LoweringContext, out: &mut String, local: &syn::Local, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<bool, String>  {
    let pat = match &local.pat {
        syn::Pat::Type(pt) => &pt.pat,
        p => p,
    };
    let name = if let syn::Pat::Ident(id) = pat { id.ident.to_string() } else { "".to_string() };
    
    if !name.is_empty() && local_vars.contains_key(&name) {
        emit_hoisted_local_init(ctx, out, local, &name, local_vars)?;
    } else {
        emit_unhoisted_local_init(ctx, out, local, &name, local_vars)?;
    }
    
    if !name.is_empty() {
        emit_local_malloc_tracking(ctx, local, &name);
        emit_local_pointer_tracking(ctx, local, &name, local_vars);
        emit_local_arena_tracking(ctx, local, &name);
    }
    
    Ok(false)
}

fn emit_hoisted_local_init(ctx: &mut LoweringContext, out: &mut String, local: &syn::Local, name: &str, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<(), String> {
    let (ty, kind) = local_vars.get(name).ok_or_else(|| format!("Local variable {} lost during emission", name))?.clone();
    if let Some(init) = &local.init {
        let hint = if ty.k_is_ptr_type() { None } else { Some(&ty) };
        let (val, init_ty) = emit_expr(ctx, out, &init.expr, local_vars, hint)?;
        
        if ty.is_affine() {
            if let Some(rhs_var_name) = crate::codegen::expr::extract_ident_name(&init.expr) {
                ctx.consumed_vars_mut().insert(rhs_var_name);
            }
        }
        
        let val_prom = crate::codegen::type_bridge::promote_numeric(ctx, out, &val, &init_ty, &ty)?;
        if let LocalKind::Ptr(ptr) = kind {
             ctx.emit_store_logical(out, &val_prom, &ptr, &ty)?;
        }

        if !ctx.config.no_verify && ty.is_integer() {
            if let Ok(z3_val) = crate::codegen::expr::translate_to_z3(ctx, &init.expr, local_vars) {
                use crate::z3_shim::ast::Ast;
                let z3_var = ctx.mk_var(name);
                ctx.z3_solver.assert(&z3_var._eq(&z3_val));
            }
        }
    }
    Ok(())
}



/// If the local init is an integer literal, assert equality in Z3 solver.
fn assert_local_lit_int_in_z3(ctx: &mut LoweringContext, name: &str, init: &Option<syn::LocalInit>) {
    let init_expr = match init { Some(i) => &i.expr, None => return };
    let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(li), .. }) = &**init_expr else { return; };
    let Ok(int_val) = li.base10_parse::<i64>() else { return; };
    use crate::z3_shim::ast::Ast;
    let z3_var = ctx.mk_var(name);
    let z3_val = ctx.mk_int(int_val);
    ctx.z3_solver.assert(&z3_var._eq(&z3_val));
}

fn emit_unhoisted_local_init(ctx: &mut LoweringContext, out: &mut String, local: &syn::Local, name: &str, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<(), String> {
    let type_hint: Option<Type> = match &local.pat {
        syn::Pat::Type(pt) => Some(resolve_type(ctx, &crate::grammar::SynType::from_std(*pt.ty.clone()).map_err(|e| e.to_string())?)),
        _ => None,
    };
    
    let (val, actual_ty) = if let Some(init) = &local.init {
        let (v, t) = emit_expr(ctx, out, &init.expr, local_vars, type_hint.as_ref())?;
        
        if t.is_affine() {
            if let Some(rhs_var_name) = crate::codegen::expr::extract_ident_name(&init.expr) {
                ctx.consumed_vars_mut().insert(rhs_var_name);
            }
        }
        (v, t)
    } else {
        ("%c0".to_string(), Type::I32)
    };
    
    let target_ty = type_hint.unwrap_or_else(|| actual_ty.clone());
    emit_pattern(ctx, out, &local.pat, val, actual_ty, target_ty.clone(), local_vars)?;

    if !ctx.config.no_verify && !name.is_empty() && target_ty.is_integer() {
        assert_local_lit_int_in_z3(ctx, name, &local.init);
    }
    Ok(())
}

fn emit_local_malloc_tracking(ctx: &mut LoweringContext, local: &syn::Local, name: &str) {
    let pending = ctx.pending_malloc_result.take();
    if pending.is_some() {
        let alloc_id = format!("malloc:{}", name);
        ctx.malloc_tracker.track(alloc_id, format!("malloc at {}", name));
    }

    if let Some(init) = &local.init {
        if let syn::Expr::Cast(c) = &*init.expr {
            if let syn::Expr::Path(p) = &*c.expr {
                if p.path.segments.len() == 1 {
                    let src = p.path.segments[0].ident.to_string();
                    let src_alloc_id = format!("malloc:{}", src);
                    if ctx.malloc_tracker.contains_alloc(&src_alloc_id) {
                        ctx.malloc_tracker.link_dependency(name.to_string(), src_alloc_id);
                    }
                }
            }
        }
    }

    ctx.malloc_tracker.drain_pending_to(name);
}

fn emit_local_pointer_tracking(ctx: &mut LoweringContext, local: &syn::Local, name: &str, local_vars: &HashMap<String, (Type, LocalKind)>) {
    let pending_state = ctx.pending_pointer_state.take();
    if let Some(state) = pending_state {
        match state {
            crate::codegen::verification::PointerState::Empty => ctx.pointer_tracker.mark_empty(name),
            crate::codegen::verification::PointerState::Valid => ctx.pointer_tracker.mark_valid(name),
            crate::codegen::verification::PointerState::Optional => ctx.pointer_tracker.mark_optional(name),
            crate::codegen::verification::PointerState::Freed => ctx.pointer_tracker.mark_freed(name),
            crate::codegen::verification::PointerState::Uninitialized => ctx.pointer_tracker.mark_uninitialized(name),
        }
    } else if local.init.is_none() {
        if let Some((ty, _)) = local_vars.get(name) {
            if ty.k_is_ptr_type() {
                ctx.pointer_tracker.mark_uninitialized(name);
            }
        }
    }
}

fn emit_local_arena_tracking(ctx: &mut LoweringContext, local: &syn::Local, name: &str) {
    if let Some(init) = &local.init {
        if is_arena_constructor(&init.expr) {
            ctx.arena_escape_tracker.register_arena(name);
        }
        if let Some(arena_name) = extract_arena_alloc_receiver(&init.expr) {
            ctx.arena_escape_tracker.register_alloc(name, &arena_name);
        }
        if let Some(arena_name) = extract_arena_allocator_source(&init.expr) {
            ctx.arena_escape_tracker.register_arena_allocator(name, &arena_name);
        }
        if let Some(alloc_name) = extract_vec_new_allocator(&init.expr) {
            ctx.arena_escape_tracker.register_vec_from_allocator(name, &alloc_name);
        }
    }
}




/// Phase A: Prove loop invariants hold at entry (base case).
fn prove_while_loop_base_case(
    ctx: &mut LoweringContext,
    stmts: &[Stmt],
    bv: &HashMap<String, (Type, LocalKind)>,
) -> Result<Vec<syn::Expr>, String> {
    if ctx.config.no_verify { return Ok(vec![]); }
    let sc = crate::codegen::verification::SymbolicContext::new(ctx.z3_ctx);
    let mut inv: Vec<syn::Expr> = Vec::new();
    for s in stmts { if let Stmt::Invariant(e) = s { inv.push(e.clone()); } }
    ctx.z3_solver.push();
    for e in &inv {
        if let Ok(z) = crate::codegen::expr::translate_bool_to_z3(ctx, e, bv, &sc) {
            ctx.z3_solver.push(); ctx.z3_solver.assert(&z.not());
            let ck = ctx.z3_solver.check(); ctx.z3_solver.pop(1);
            if ck == crate::z3_shim::SatResult::Sat {
                ctx.z3_solver.pop(1);
                return Err("Z3 verification failed: loop invariant does not hold at entry.                      The solver found a counterexample proving the invariant is false                      with current variable values.".to_string());
            }
            ctx.z3_solver.assert(&z);
        }
    }
    ctx.z3_solver.pop(1);
    for e in &inv {
        if let Ok(z) = crate::codegen::expr::translate_bool_to_z3(ctx, e, bv, &sc) {
            ctx.z3_solver.assert(&z);
        }
    }
    Ok(inv)
}

/// Phase B: Inductive step for while loop verification.
fn setup_while_loop_inductive_step(
    ctx: &mut LoweringContext,
    stmts: &[Stmt],
    bv: &mut HashMap<String, (Type, LocalKind)>,
    cond: &syn::Expr,
    inv: &[syn::Expr],
) -> Result<(), String> {
    if ctx.config.no_verify { return Ok(()); }
    let sc = crate::codegen::verification::SymbolicContext::new(ctx.z3_ctx);
    ctx.z3_solver.push();
    for n in &collect_mutations(stmts) {
        if let Some((ty, _)) = bv.get(n) {
            if ty.is_integer() {
                let f = format!("{}_havoc_{}", n, ctx.next_id());
                ctx.symbolic_tracker.insert(n.clone(), ctx.mk_var(&f));
            }
        }
    }
    for e in inv {
        if let Ok(z) = crate::codegen::expr::translate_bool_to_z3(ctx, e, bv, &sc) {
            ctx.z3_solver.assert(&z);
        }
    }
    if let Ok(z) = crate::codegen::expr::translate_bool_to_z3(ctx, cond, bv, &sc) {
        ctx.z3_solver.assert(&z);
    }
    Ok(())
}

/// Phase C: Pop inductive scope and assert not(cond) for post-loop.
fn verify_while_loop_post_body(
    ctx: &mut LoweringContext,
    cond: &syn::Expr,
    lv: &HashMap<String, (Type, LocalKind)>,
) {
    if ctx.config.no_verify { return; }
    let sc = crate::codegen::verification::SymbolicContext::new(ctx.z3_ctx);
    ctx.z3_solver.pop(1);
    if let Ok(z) = crate::codegen::expr::translate_bool_to_z3(ctx, cond, lv, &sc) {
        ctx.z3_solver.assert(&z.not());
    }
}
fn emit_while_stmt(ctx: &mut LoweringContext, out: &mut String, w: &crate::grammar::SaltWhile, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<bool, String>  {
            let label_header = format!("while_header_{}", ctx.next_id());
            let label_body = format!("while_body_{}", ctx.next_id());
            let label_exit = format!("while_exit_{}", ctx.next_id());
            
            out.push_str(&format!("    cf.br ^{}\n", label_header));
            out.push_str(&format!("  ^{}:\n", label_header));
            
            let (cond_val, cond_ty) = emit_expr(ctx, out, &w.cond, local_vars, None)?;
            // Accept Pointer types as while conditions
            let cond_val = if cond_ty.k_is_ptr_type() {
                let id = ctx.next_id();
                let int_val = format!("%ptrtoint_{}", id);
                let zero_val = format!("%ptr_zero_{}", ctx.next_id());
                let cmp_val = format!("%ptr_nonnull_{}", id);
                out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", int_val, cond_val));
                out.push_str(&format!("    {} = arith.constant 0 : i64\n", zero_val));
                out.push_str(&format!("    {} = arith.cmpi ne, {}, {} : i64\n", cmp_val, int_val, zero_val));
                cmp_val
            } else if cond_ty != Type::Bool {
                return Err(format!("While condition must be boolean, found {:?}", cond_ty));
            } else {
                cond_val
            };
            
            let loc = ctx.loc_tag(w.cond.span());
            out.push_str(&format!("    cf.cond_br {}, ^{}, ^{}{}\n", cond_val, label_body, label_exit, loc));
            out.push_str(&format!("  ^{}:\n", label_body));
            
            // Heartbeat Injection (simplified, uses @yielding at function level)
            if !*ctx.no_yield() {
                ctx.emit_lto_hook(out, "__salt_yield_check", &[], local_vars, None)?;
            }
            ctx.break_labels_mut().push(label_exit.clone());
            ctx.continue_labels_mut().push(label_header.clone());
            let mut body_vars = local_vars.clone();

            // === Z3 HOARE LOGIC: While Loop Verification ===
            let invariant_exprs = prove_while_loop_base_case(ctx, &w.body.stmts, &body_vars)?;
            setup_while_loop_inductive_step(ctx, &w.body.stmts, &mut body_vars, &w.cond, &invariant_exprs)?;

            // `while p.addr() != 0 { ... }` — p is Valid inside body (push/pop isolates body state)
            let ptr_narrowing = get_narrowing_target(&w.cond);
            if let Some((ref var, true)) = ptr_narrowing { ctx.pointer_tracker.push_scope(); ctx.pointer_tracker.mark_valid(var); }
            let body_diverges = emit_block(ctx, out, &w.body.stmts, &mut body_vars)?;
            if ptr_narrowing.is_some() { ctx.pointer_tracker.pop_scope(); }

            verify_while_loop_post_body(ctx, &w.cond, local_vars);
            ctx.break_labels_mut().pop();
            ctx.continue_labels_mut().pop();
            
            if !body_diverges {
                out.push_str(&format!("    cf.br ^{}\n", label_header));
            }
            out.push_str(&format!("  ^{}:\n", label_exit));
            Ok(false)
        }

fn emit_for_stmt(ctx: &mut LoweringContext, out: &mut String, f: &crate::grammar::SaltFor, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<bool, String>  {
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

fn emit_cf_br_for_loop(ctx: &mut LoweringContext, out: &mut String, f: &crate::grammar::SaltFor, start_expr: Option<&syn::Expr>, end_expr: Option<&syn::Expr>, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<bool, String> {
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
    
    ctx.break_labels_mut().push(label_exit.clone());
    ctx.continue_labels_mut().push(label_header.clone());
    ctx.push_cleanup_scope();
    
    let body_diverges = emit_block(ctx, out, &f.body.stmts, &mut body_vars)?;
    ctx.break_labels_mut().pop();
    ctx.continue_labels_mut().pop();

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
fn emit_return_stmt(ctx: &mut LoweringContext, out: &mut String, opt_expr: &Option<syn::Expr>, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<bool, String>  {
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

fn emit_loop_stmt(ctx: &mut LoweringContext, out: &mut String, body: &crate::grammar::SaltBlock, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<bool, String>  {
            let label_body = format!("loop_body_{}", ctx.next_id());
            let label_exit = format!("loop_exit_{}", ctx.next_id());
            
            out.push_str(&format!("    cf.br ^{}\n", label_body));
            out.push_str(&format!("  ^{}:\n", label_body));
            
            // Heartbeat Injection
            if !*ctx.no_yield() {
                ctx.emit_lto_hook(out, "__salt_yield_check", &[], local_vars, None)?;
            }
            ctx.break_labels_mut().push(label_exit.clone());
            ctx.continue_labels_mut().push(label_body.clone());
            let mut body_vars = local_vars.clone();
            let body_diverges = emit_block(ctx, out, &body.stmts, &mut body_vars)?;
            ctx.break_labels_mut().pop();
            ctx.continue_labels_mut().pop();
            
            if !body_diverges {
                out.push_str(&format!("    cf.br ^{}\n", label_body));
            }

            // Only emit the exit block if a break
            // actually targets it. An infinite `loop { }` with no break
            // produces an exit block with zero predecessors, which crashes
            // MLIR's dominance tree computation in salt-opt.
            let break_target = format!("cf.br ^{}", label_exit);
            let break_was_used = out.contains(&break_target);
            if break_was_used {
                out.push_str(&format!("  ^{}:\n", label_exit));
                Ok(false)
            } else {
                // Infinite loop — no exit path exists. Signal divergence.
                Ok(true)
            }
        }

fn emit_unsafe_stmt(ctx: &mut LoweringContext, out: &mut String, block: &crate::grammar::SaltBlock, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<bool, String>  {
            // Only allow unsafe blocks in privileged packages
            // (std.* and kernel.*). All other packages are rejected.
            // Uses config.file.package as fallback when current_package is None.
            let first_seg = ctx.current_package.as_ref()
                .or(ctx.config.file.package.as_ref())
                .and_then(|pkg| pkg.name.iter().next().map(|id| id.to_string()));

            let fn_name = ctx.current_fn_name();
            let is_privileged = matches!(first_seg.as_deref(), Some("std") | Some("kernel") | Some("basalt"))
                || fn_name.starts_with("std__") || fn_name.starts_with("kernel__") || fn_name.starts_with("basalt__");

            if !is_privileged {
                return Err("unsafe blocks are not allowed in user code. All unsafe operations must go through the standard library's safe abstractions or be placed in kernel.* or basalt packages. See docs/UNSAFE.md.".to_string());
            }

            let was_unsafe = *ctx.is_unsafe_block();
            *ctx.is_unsafe_block_mut() = true;
            let mut inner_vars = local_vars.clone();
            let res = emit_block(ctx, out, &block.stmts, &mut inner_vars)?;
            *ctx.is_unsafe_block_mut() = was_unsafe;
            Ok(res)
        }

fn emit_dynamic_check_stmt(ctx: &mut LoweringContext, out: &mut String, block: &crate::grammar::SaltBlock, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<bool, String>  {
            let was_dynamic = *ctx.is_dynamic_check_block();
            *ctx.is_dynamic_check_block_mut() = true;
            let mut inner_vars = local_vars.clone();
            let res = emit_block(ctx, out, &block.stmts, &mut inner_vars)?;
            *ctx.is_dynamic_check_block_mut() = was_dynamic;
            Ok(res)
        }




/// Register Vec type for RAII-lite cleanup at scope exit in pattern matching.
fn register_vec_pattern_cleanup(
    ctx: &mut LoweringContext,
    target_ty: &Type,
    name: &str,
    kind: &LocalKind,
) {
    let Type::Concrete(base, args) = target_ty else { return; };
    if base != "Vec" && !base.ends_with("__Vec") && !base.contains("__vec__Vec") { return; }
    let suffix = args.first().map(|t| t.mangle_suffix()).unwrap_or_else(|| "T".to_string());
    let drop_fn = format!("std__collections__vec__Vec__drop_{}", suffix);
    let LocalKind::Ptr(ref alloca) = kind else { return; };
    let ref_ty = Type::Reference(Box::new(target_ty.clone()), true);
    ctx.register_owned_resource(alloca, &drop_fn, name, ref_ty);
}

pub fn emit_pattern(
    ctx: &mut LoweringContext,
    out: &mut String,
    pat: &syn::Pat,
    val: String,
    actual_ty: Type,
    target_ty: Type,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<(), String> {
    // Loop Induction Isolation
    // If binding an induction variable (actual=Usize or integer),
    // it must NOT be allowed to be 'magnetized' by a Pointer target.
    // This prevents the "Usize to Pointer" contamination from loop bodies.
    let final_target = if (actual_ty == Type::Usize || actual_ty.is_integer()) && target_ty.k_is_ptr_type() {
        actual_ty.clone() // Use the actual type, not the magnetized Pointer target
    } else {
        target_ty.clone()
    };
    
    match pat {
        syn::Pat::Ident(id) => {
            let name = id.ident.to_string();
            let val_prom = crate::codegen::type_bridge::promote_numeric(ctx, out, &val, &actual_ty, &final_target)?;
            let is_mut = id.mutability.is_some() || matches!(final_target, Type::Struct(_) | Type::Array(..) | Type::Owned(_));

            // TENSOR SPECIAL CASE: Tensors (memrefs) are always SSA - their contents are mutated, not the value
            if matches!(target_ty, Type::Tensor(..)) {
                local_vars.insert(name, (target_ty, LocalKind::SSA(val_prom)));
                return Ok(());
            }

            let kind = if let Some((existing_ty, LocalKind::Ptr(existing_ptr))) = local_vars.get(&name).cloned() {
                let val_final = crate::codegen::type_bridge::promote_numeric(ctx, out, &val_prom, &target_ty, &existing_ty)?;
                ctx.emit_store_logical(out, &val_final, &existing_ptr, &existing_ty)?;
                return Ok(());
            } else if is_mut {
                let alloca = format!("%local_{}_{}", name, ctx.next_id());
                let mlir_ty = target_ty.to_mlir_storage_type(ctx)?;
                ctx.emit_alloca(out, &alloca, &mlir_ty);
                
                ctx.emit_store_logical(out, &val_prom, &alloca, &target_ty)?;
                LocalKind::Ptr(alloca)
            } else {
                LocalKind::SSA(val_prom.clone())
            };
            
            register_vec_pattern_cleanup(ctx, &target_ty, &name, &kind);
            
            local_vars.insert(name, (target_ty, kind));
            Ok(())
        }
        syn::Pat::Type(pt) => emit_pattern(ctx, out, &pt.pat, val, actual_ty, target_ty, local_vars),
        syn::Pat::Tuple(tuple) => {
            if let Type::Tuple(elems) = &actual_ty {
                if tuple.elems.len() != elems.len() {
                    return Err(format!("Tuple pattern length mismatch: expected {}, found {}", elems.len(), tuple.elems.len()));
                }
                let struct_ty = actual_ty.to_mlir_type(ctx)?;
                for (i, p) in tuple.elems.iter().enumerate() {
                    let raw_val = format!("%tuple_ext_{}_{}", i, ctx.next_id());
                    ctx.emit_extractvalue(out, &raw_val, &val, i, &struct_ty);
                    let elem_ty = &elems[i];
                    
                    let final_val = if *elem_ty == Type::Bool {
                        // cmpxchg tuples store the success flag as native i1,
                        // not as i8. Check if the struct field is already i1 before truncating.
                        let is_already_i1 = struct_ty.contains("i1");
                        if is_already_i1 {
                            raw_val  // Already i1, no truncation needed
                        } else {
                            let trunc = format!("%b_trunc_pat_t_{}", ctx.next_id());
                            ctx.emit_trunc(out, &trunc, &raw_val, "i8", "i1");
                            trunc
                        }
                    } else {
                        raw_val
                    };
                    emit_pattern(ctx, out, p, final_val, elem_ty.clone(), elem_ty.clone(), local_vars)?;
                }
                Ok(())
            } else {
                Err(format!("Expected tuple type for destructuring, found {:?}", actual_ty))
            }
        }
        syn::Pat::Struct(ps) => {
            let struct_name = ps.path.segments.last().ok_or_else(|| "Empty path in struct pattern".to_string())?.ident.to_string();
            let info = ctx.struct_registry().values().find(|i| i.name == struct_name).cloned().ok_or(format!("Unknown struct {}", struct_name))?.clone();
            
            let struct_ty_mlir = actual_ty.to_mlir_type(ctx)?;
            for field_pat in &ps.fields {
                let field_name = match &field_pat.member {
                    syn::Member::Named(id) => id.to_string(),
                    _ => return Err("Unnamed members in struct pattern not supported".to_string()),
                };
                
                if let Some((idx, field_ty)) = info.fields.get(&field_name) {
                    let raw_val = format!("%struct_ext_{}_{}", field_name, ctx.next_id());
                    ctx.emit_extractvalue(out, &raw_val, &val, *idx, &struct_ty_mlir);
                    
                    let final_val = if *field_ty == Type::Bool {
                        let trunc = format!("%b_trunc_pat_s_{}", ctx.next_id());
                        ctx.emit_trunc(out, &trunc, &raw_val, "i8", "i1");
                        trunc
                    } else {
                        raw_val
                    };
                    emit_pattern(ctx, out, &field_pat.pat, final_val, field_ty.clone(), field_ty.clone(), local_vars)?;
                } else {
                    return Err(format!("Field {} not found in struct {}", field_name, struct_name));
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

// Helper to detect `p.addr != 0` or `p.addr == 0` check
fn get_narrowing_target(cond: &syn::Expr) -> Option<(String, bool)> {
    // Bare pointer: `if ptr { ... }` => narrowing target = ptr, is_neq=true
    if let syn::Expr::Path(p) = cond {
        if let Some(ident) = p.path.get_ident() {

            return Some((ident.to_string(), true));
        }
    }
    
    if let syn::Expr::Binary(bin) = cond {
        // Check if RHS is 0
        let is_zero = if let syn::Expr::Lit(l) = &*bin.right {
             if let syn::Lit::Int(vals) = &l.lit { vals.base10_parse::<u64>().unwrap_or(1) == 0 } else { false }
        } else { false };
        
        if is_zero {
             // Check if LHS is p.addr
             if let syn::Expr::Field(f) = &*bin.left {
                 if let syn::Member::Named(id) = &f.member {
                     if id == "addr" {
                         if let syn::Expr::Path(p) = &*f.base {
                             if let Some(ident) = p.path.get_ident() {
                                 let var_name = ident.to_string();
                                 // != 0 (is_neq=true) or == 0 (is_neq=false)
                                 if let syn::BinOp::Ne(_) = bin.op { return Some((var_name, true)); }
                                 if let syn::BinOp::Eq(_) = bin.op { return Some((var_name, false)); }
                             }
                         }
                     }
                 }
             }
        }
    }
    None
}

pub fn emit_salt_if(
    ctx: &mut LoweringContext,
    out: &mut String,
    cond: &syn::Expr,
    then_branch: &SaltBlock,
    else_branch: &Option<Box<SaltElse>>,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<bool, String> {
    let label_then = format!("then_{}", ctx.next_id());
    let label_else = format!("else_{}", ctx.next_id());
    let label_merge = format!("merge_{}", ctx.next_id());

    let (cond_val, cond_ty) = emit_expr(ctx, out, cond, local_vars, None)?;
    // Accept Pointer types as if conditions
    let cond_val = if cond_ty.k_is_ptr_type() {
        let id = ctx.next_id();
        let int_val = format!("%ptrtoint_{}", id);
        let zero_val = format!("%ptr_zero_{}", ctx.next_id());
        let cmp_val = format!("%ptr_nonnull_{}", id);
        out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", int_val, cond_val));
        out.push_str(&format!("    {} = arith.constant 0 : i64\n", zero_val));
        out.push_str(&format!("    {} = arith.cmpi ne, {}, {} : i64\n", cmp_val, int_val, zero_val));
        cmp_val
    } else if cond_ty != Type::Bool {
        return Err(format!("If condition must be boolean, found {:?}", cond_ty));
    } else {
        cond_val
    };

    // Flow-Sensitive Narrowing
    let narrowing = get_narrowing_target(cond);
    
    // Save state (Push Scope) for Then branch
    ctx.pointer_tracker.push_scope();

    // Apply narrowing for Then
    if let Some((var, is_neq)) = &narrowing {

        if *is_neq { 
            // p != 0 -> Valid in Then
            ctx.pointer_tracker.mark_valid(var); 
        } else { 
            // p == 0 -> Empty in Then
            ctx.pointer_tracker.mark_empty(var); 
        }
    } 

    let loc = ctx.loc_tag(cond.span());
    let has_else = else_branch.is_some();
    if has_else {
         out.push_str(&format!("    cf.cond_br {}, ^{}, ^{}{}\n", cond_val, label_then, label_else, loc));
    } else {
         out.push_str(&format!("    cf.cond_br {}, ^{}, ^{}{}\n", cond_val, label_then, label_merge, loc));
    }

    let state_before = ctx.consumed_vars().clone();
    let locs_before = ctx.consumption_locs().clone();

    // Save LVN cache before then-branch
    ctx.emission.global_lvn.push_snapshot();

    out.push_str(&format!("  ^{}:\n", label_then));
    let mut then_vars = local_vars.clone();
    // Push branch condition for Z3 postcondition verification
    ctx.emission.path_conditions.push(cond.clone());
    let then_diverges = emit_block(ctx, out, &then_branch.stmts, &mut then_vars)?;
    ctx.emission.path_conditions.pop();
    if !then_diverges {
        out.push_str(&format!("    cf.br ^{}\n", label_merge));
    }

    // Restore LVN cache after then-branch — discard branch-local values
    ctx.emission.global_lvn.pop_snapshot();

    // Restore Pre-If state for Else/Merge (pop "Then" scope)
    let pre_if_state_opt = ctx.pointer_tracker.pop_scope();
    if let Some(pre_if_state) = pre_if_state_opt {
        ctx.pointer_tracker.restore_state(pre_if_state);
    }

    let state_after_then = ctx.consumed_vars().clone();
    let locs_after_then = ctx.consumption_locs().clone();

    // Restore state for Else branch
    *ctx.consumed_vars_mut() = state_before.clone();
    *ctx.consumption_locs_mut() = locs_before.clone();

    let mut else_diverges = false;
    if has_else {
        // Save state (Push Scope) for Else branch (which is Pre-If currently)
        ctx.pointer_tracker.push_scope();

        // Apply narrowing for Else
        if let Some((var, is_neq)) = &narrowing {
            if *is_neq { 
                 // Else of != 0 (== 0) -> Empty
                ctx.pointer_tracker.mark_empty(var); 
            } else { 
                 // Else of == 0 (!= 0) -> Valid
                ctx.pointer_tracker.mark_valid(var); 
            }
        }

        // Save LVN cache before else-branch
        ctx.emission.global_lvn.push_snapshot();

        out.push_str(&format!("  ^{}:\n", label_else));
        let mut else_vars = local_vars.clone();
        // Push negated condition for else branch
        let negated_cond = syn::Expr::Unary(syn::ExprUnary {
            attrs: vec![],
            op: syn::UnOp::Not(syn::token::Not::default()),
            expr: Box::new(cond.clone()),
        });
        ctx.emission.path_conditions.push(negated_cond);
        else_diverges = if let Some(eb) = else_branch {
            match eb.as_ref() {
                SaltElse::Block(b) => emit_block(ctx, out, &b.stmts, &mut else_vars)?,
                SaltElse::If(nested) => {
                     emit_salt_if(ctx, out, &nested.cond, &nested.then_branch, &nested.else_branch, &mut else_vars)?
                }
            }
        } else {
            false
        };
        ctx.emission.path_conditions.pop();
        if !else_diverges {
            out.push_str(&format!("    cf.br ^{}\n", label_merge));
        }

        // Restore LVN cache after else-branch
        ctx.emission.global_lvn.pop_snapshot();

        // Restore Pre-If state for Merge (pop "Else" scope)
        let pre_if_state_opt = ctx.pointer_tracker.pop_scope();
        if let Some(pre_if_state) = pre_if_state_opt {
            ctx.pointer_tracker.restore_state(pre_if_state);
        }
    }
    
    let state_after_else = ctx.consumed_vars().clone();
    let locs_after_else = ctx.consumption_locs().clone();

    // MERGE: Union of consumed vars, but filtered to outer scope
    // We only care about variables that existed BEFORE the if (in local_vars)
    // Local vars defined inside branches are out of scope, so their consumption status is irrelevant
    // UNLESS preventing reuse of names? No, reuse is fine if new definition.
    
    // Safety: If a variable is consumed in ANY branch executed, it is consumed.
    // Since the taken branch is unknown, consumption must be assumed if used in EITHER (for safety).
    // But logically, if I check `if x { move y } else { keep y }`. After: y is maybe moved.
    // Salt requires definitive move? Or partial move tracking?
    // For now, Union is safe (over-conservative).
    // Filtering by `local_vars` prevents leaking inner names.
    
    let mut final_consumed = state_before.clone();
    let mut final_locs = locs_before.clone();
    
    // Add Then-consumed outer vars
    for v in state_after_then.iter() {
        if local_vars.contains_key(v) {
             final_consumed.insert(v.clone());
             if let Some(l) = locs_after_then.get(v) { final_locs.insert(v.clone(), l.clone()); }
        }
    }
    // Add Else-consumed outer vars
    for v in state_after_else.iter() {
        if local_vars.contains_key(v) {
             final_consumed.insert(v.clone());
             if let Some(l) = locs_after_else.get(v) { final_locs.insert(v.clone(), l.clone()); }
        }
    }
    
    *ctx.consumed_vars_mut() = final_consumed;
    *ctx.consumption_locs_mut() = final_locs;

    if !then_diverges || !else_diverges || !has_else {
        out.push_str(&format!("  ^{}:\n", label_merge));
        Ok(false)
    } else {
        Ok(true)
    }
}

// ============================================================================
// PHASE 2: Match Expression Codegen
// ============================================================================

/// Emit match expression
/// 
/// Strategy: Chain of conditional branches for each arm
pub fn emit_match(
    ctx: &mut LoweringContext,
    out: &mut String,
    match_expr: &SaltMatch,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<bool, String> {
    // Evaluate scrutinee
    let (scrutinee_val, scrutinee_ty) = emit_expr(ctx, out, &match_expr.scrutinee, local_vars, None)?;
    
    if match_expr.arms.is_empty() {
        return Err("Match expression must have at least one arm".to_string());
    }
    
    // Check exhaustiveness for enum types
    use crate::codegen::verification::{check_exhaustiveness, ExhaustivenessResult};
    match check_exhaustiveness(ctx, &scrutinee_ty, &match_expr.arms) {
        ExhaustivenessResult::Exhaustive => {
            // Good - all variants covered
        }
        ExhaustivenessResult::MissingVariants(_missing) => {
        }
        ExhaustivenessResult::Unverifiable(_reason) => {
            // Can't verify - skip silently for non-enum types
        }
    }
    // Generate labels
    let merge_label = format!("match_merge_{}", ctx.next_id());
    
    // Collect arm labels and check labels
    let mut arm_labels: Vec<String> = Vec::new();
    let mut check_labels: Vec<String> = Vec::new();
    
    for i in 0..match_expr.arms.len() {
        arm_labels.push(format!("match_arm_{}_{}", i, ctx.next_id()));
        if i + 1 < match_expr.arms.len() {
            check_labels.push(format!("match_check_{}_{}", i + 1, ctx.next_id()));
        }
    }
    
    // Track if any arm doesn't diverge (merge block needed)
    let mut any_non_diverging = false;
    
    // Emit chain of checks
    for (i, arm) in match_expr.arms.iter().enumerate() {
        let arm_label = &arm_labels[i];
        let next_check = if i + 1 < match_expr.arms.len() {
            &check_labels[i]
        } else {
            arm_label
        };
        
        // Check if wildcard/catch-all
        let is_wildcard = matches!(&arm.pattern, Pattern::Wildcard) || 
                         matches!(&arm.pattern, Pattern::Ident { mutable: _, name: _ });
        
        if is_wildcard {
            out.push_str(&format!("    cf.br ^{}\n", arm_label));
        } else {
            let cond = emit_pattern_condition(ctx, out, &arm.pattern, &scrutinee_val, &scrutinee_ty)?;
            
            let final_cond = if let Some(guard) = &arm.guard {
                // Pattern bindings must be available in the guard scope
                // For example, `Ok(v) if v > 0 => ...` needs `v` to resolve in the guard.
                // We emit bindings into a temporary scope for guard evaluation.
                let mut guard_vars = local_vars.clone();
                emit_pattern_bindings(ctx, out, &arm.pattern, &scrutinee_val, &scrutinee_ty, &mut guard_vars)?;
                
                let (guard_val, guard_ty) = emit_expr(ctx, out, guard, &mut guard_vars, Some(&Type::Bool))?;
                if guard_ty != Type::Bool {
                    return Err(format!("Match guard must be boolean, found {:?}", guard_ty));
                }
                let combined = format!("%guard_and_{}", ctx.next_id());
                out.push_str(&format!("    {} = arith.andi {}, {} : i1\n", combined, cond, guard_val));
                combined
            } else {
                cond
            };
            
            let loc = ctx.loc_tag(match_expr.scrutinee.span());
            out.push_str(&format!("    cf.cond_br {}, ^{}, ^{}{}\n", final_cond, arm_label, next_check, loc));
        }
        
        if i + 1 < match_expr.arms.len() && !is_wildcard {
            out.push_str(&format!("  ^{}:\n", next_check));
        }
    }
    
    // Emit arm bodies
    for (i, arm) in match_expr.arms.iter().enumerate() {
        out.push_str(&format!("  ^{}:\n", arm_labels[i]));
        
        let mut arm_vars = local_vars.clone();
        emit_pattern_bindings(ctx, out, &arm.pattern, &scrutinee_val, &scrutinee_ty, &mut arm_vars)?;
        
        let arm_diverges = emit_block(ctx, out, &arm.body.stmts, &mut arm_vars)?;
        
        if !arm_diverges {
            any_non_diverging = true;
            out.push_str(&format!("    cf.br ^{}\n", merge_label));
        }
    }
    
    if any_non_diverging {
        out.push_str(&format!("  ^{}:\n", merge_label));
    }
    
    Ok(!any_non_diverging)
}

/// Emit condition for a variant pattern match (discriminant comparison).
fn emit_variant_pattern_condition(
    ctx: &mut LoweringContext,
    out: &mut String,
    path: &[syn::Ident],
    scrutinee: &str,
    scrutinee_ty: &Type,
) -> Result<String, String> {
    if path.is_empty() {
        return Err("Empty variant path".to_string());
    }
    let variant_name = path.last().ok_or_else(|| "Failed to get variant name".to_string())?.to_string();
    let enum_name = match scrutinee_ty {
        Type::Enum(name) => name.clone(),
        Type::Concrete(_, _) => scrutinee_ty.mangle_suffix(),
        _ => return Err(format!("Cannot match variant on non-enum type: {:?}", scrutinee_ty)),
    };
    let info = ctx.enum_registry().values()
        .find(|i| i.name == enum_name || i.name.ends_with(&format!("__{}", enum_name)))
        .cloned()
        .ok_or_else(|| format!("Unknown enum '{}' in pattern match", enum_name))?;
    let (_, _, discriminant) = info.variants.iter()
        .find(|(n, _, _)| n == &variant_name)
        .ok_or_else(|| format!("Unknown variant '{}' in enum '{}'", variant_name, enum_name))?;
    let struct_ty = scrutinee_ty.to_mlir_type(ctx)?;
    let tag_val = format!("%match_tag_{}", ctx.next_id());
    ctx.emit_extractvalue(out, &tag_val, scrutinee, 0, &struct_ty);
    let disc_const = format!("%disc_const_{}", ctx.next_id());
    let result = format!("%match_variant_{}", ctx.next_id());
    ctx.emit_const_int(out, &disc_const, *discriminant as i64, "i32");
    out.push_str(&format!("    {} = arith.cmpi eq, {}, {} : i32\n", result, tag_val, disc_const));
    Ok(result)
}

/// Emit condition for a tuple pattern match (AND of all element conditions).
fn emit_tuple_pattern_condition(
    ctx: &mut LoweringContext,
    out: &mut String,
    sub_patterns: &[Pattern],
    scrutinee: &str,
    scrutinee_ty: &Type,
) -> Result<String, String> {
    let field_types = match scrutinee_ty {
        Type::Tuple(tys) => tys.clone(),
        _ => return Err(format!("Cannot match tuple pattern on non-tuple type: {:?}", scrutinee_ty)),
    };
    if sub_patterns.len() != field_types.len() {
        return Err(format!("Tuple pattern has {} elements but type has {} fields",
            sub_patterns.len(), field_types.len()));
    }
    let mut result = format!("%tuple_match_init_{}", ctx.next_id());
    out.push_str(&format!("    {} = arith.constant true\n", result));
    let struct_ty = scrutinee_ty.to_mlir_type(ctx)?;
    for (i, (sub_pat, field_ty)) in sub_patterns.iter().zip(field_types.iter()).enumerate() {
        let field_val = format!("%tuple_field_{}_{}", i, ctx.next_id());
        ctx.emit_extractvalue(out, &field_val, scrutinee, i, &struct_ty);
        let sub_result = emit_pattern_condition(ctx, out, sub_pat, &field_val, field_ty)?;
        let combined = format!("%tuple_match_and_{}", ctx.next_id());
        out.push_str(&format!("    {} = arith.andi {}, {} : i1\n", combined, result, sub_result));
        result = combined;
    }
    Ok(result)
}

/// Emit condition for a struct pattern match (AND of all field conditions).
fn emit_struct_pattern_condition(
    ctx: &mut LoweringContext,
    out: &mut String,
    name: &syn::Ident,
    fields: &[crate::grammar::pattern::PatternField],
    scrutinee: &str,
    scrutinee_ty: &Type,
) -> Result<String, String> {
    let struct_name = match scrutinee_ty {
        Type::Struct(n) | Type::Concrete(n, _) => n.clone(),
        _ => return Err(format!("Cannot match struct pattern on non-struct type: {:?}", scrutinee_ty)),
    };
    if !struct_name.ends_with(&name.to_string()) && *name != struct_name {
        return Err(format!("Struct pattern '{}' doesn't match scrutinee type '{}'", name, struct_name));
    }
    let info = ctx.struct_registry().values()
        .find(|i| i.name == struct_name || i.name.ends_with(&format!("__{}", name)))
        .cloned()
        .ok_or_else(|| format!("Unknown struct '{}' in pattern match", name))?;
    let mut result = format!("%struct_match_init_{}", ctx.next_id());
    out.push_str(&format!("    {} = arith.constant true\n", result));
    let struct_mlir_ty = scrutinee_ty.to_mlir_type(ctx)?;
    for pat_field in fields {
        emit_struct_field_condition(ctx, out, pat_field, &info.fields, &info.name, scrutinee, &struct_mlir_ty, &mut result)?;
    }
    Ok(result)
}

/// Process one field of a struct pattern, updating the accumulator condition.
#[allow(clippy::too_many_arguments)]
// REASON: 8 args are context, out, scrutinee, field, idx, cond, local_vars, pattern_vars —
// each independently meaningful; bundling would obscure the data flow
fn emit_struct_field_condition(
    ctx: &mut LoweringContext,
    out: &mut String,
    pat_field: &crate::grammar::pattern::PatternField,
    fields: &HashMap<String, (usize, Type)>,
    struct_name: &str,
    scrutinee: &str,
    struct_mlir_ty: &str,
    result: &mut String,
) -> Result<(), String> {
    let (field_offset, field_ty) = fields.get(&pat_field.name.to_string())
        .ok_or_else(|| format!("Unknown field '{}' in struct '{}'", pat_field.name, struct_name))?
        .clone();
    let field_val = format!("%struct_field_{}_{}", pat_field.name, ctx.next_id());
    ctx.emit_extractvalue(out, &field_val, scrutinee, field_offset, struct_mlir_ty);
    let sub_pat = pat_field.pattern.as_ref()
        .cloned()
        .unwrap_or_else(|| Pattern::Ident { name: pat_field.name.clone(), mutable: false });
    let sub_result = emit_pattern_condition(ctx, out, &sub_pat, &field_val, &field_ty)?;
    let combined = format!("%struct_match_and_{}", ctx.next_id());
    out.push_str(&format!("    {} = arith.andi {}, {} : i1\n", combined, result, sub_result));
    *result = combined;
    Ok(())
}

/// Emit condition for a pattern (returns SSA value of type i1)
fn emit_pattern_condition(
    ctx: &mut LoweringContext,
    out: &mut String,
    pattern: &Pattern,
    scrutinee: &str,
    scrutinee_ty: &Type,
) -> Result<String, String> {
    match pattern {
        Pattern::Wildcard | Pattern::Ident { .. } => {
            let result = format!("%match_true_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.constant true\n", result));
            Ok(result)
        }
        Pattern::Literal(lit) => {
            let mlir_ty = scrutinee_ty.to_mlir_type(ctx)?;
            
            match lit {
                syn::Lit::Int(int_lit) => {
                    let int_val: i64 = int_lit.base10_parse().map_err(|e| e.to_string())?;
                    
                    let const_val = format!("%match_const_{}", ctx.next_id());
                    let result = format!("%match_cmp_{}", ctx.next_id());
                    
                    out.push_str(&format!("    {} = arith.constant {} : {}\n", const_val, int_val, mlir_ty));
                    out.push_str(&format!("    {} = arith.cmpi eq, {}, {} : {}\n", result, scrutinee, const_val, mlir_ty));
                    
                    Ok(result)
                }
                syn::Lit::Bool(bool_lit) => {
                    let const_val = format!("%match_const_{}", ctx.next_id());
                    let result = format!("%match_cmp_{}", ctx.next_id());
                    let bool_val = if bool_lit.value() { "true" } else { "false" };
                    
                    out.push_str(&format!("    {} = arith.constant {}\n", const_val, bool_val));
                    out.push_str(&format!("    {} = arith.cmpi eq, {}, {} : i1\n", result, scrutinee, const_val));
                    
                    Ok(result)
                }
                _ => Err(format!("Unsupported literal type in pattern: {:?}", lit)),
            }
        }
        Pattern::Or(patterns) => {
            if patterns.is_empty() {
                return Err("Empty or-pattern".to_string());
            }
            
            let mut result = emit_pattern_condition(ctx, out, &patterns[0], scrutinee, scrutinee_ty)?;
            
            for pat in patterns.iter().skip(1) {
                let next_cond = emit_pattern_condition(ctx, out, pat, scrutinee, scrutinee_ty)?;
                let combined = format!("%match_or_{}", ctx.next_id());
                out.push_str(&format!("    {} = arith.ori {}, {} : i1\n", combined, result, next_cond));
                result = combined;
            }
            
            Ok(result)
        }
        Pattern::Variant { path, fields: _ } => {
            emit_variant_pattern_condition(ctx, out, path, scrutinee, scrutinee_ty)
        }
        Pattern::Tuple(sub_patterns) => {
            emit_tuple_pattern_condition(ctx, out, sub_patterns, scrutinee, scrutinee_ty)
        }
        Pattern::Struct { name, fields } => {
            emit_struct_pattern_condition(ctx, out, name, fields, scrutinee, scrutinee_ty)
        }
        Pattern::Rest => {
            Err("Rest pattern (..) cannot appear as top-level match pattern".to_string())
        }
    }
}

/// Emit bindings for a variant pattern: extract payload and bind sub-patterns.
fn emit_variant_pattern_bindings(
    ctx: &mut LoweringContext,
    out: &mut String,
    path: &[syn::Ident],
    fields: &Option<Vec<Pattern>>,
    scrutinee: &str,
    scrutinee_ty: &Type,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<(), String> {
    let field_patterns = match fields {
        Some(fp) if !fp.is_empty() => fp,
        _ => return Ok(()),
    };
    let enum_name = match scrutinee_ty {
        Type::Enum(name) => name.clone(),
        Type::Concrete(_, _) => scrutinee_ty.mangle_suffix(),
        _ => return Err(format!("Cannot bind variant on non-enum type: {:?}", scrutinee_ty)),
    };
    let variant_name = path.last().map(|i| i.to_string()).unwrap_or_default();
    let info = ctx.enum_registry().values()
        .find(|i| i.name == enum_name || i.name.ends_with(&format!("__{}", enum_name)))
        .cloned()
        .ok_or_else(|| format!("Unknown enum '{}' in pattern binding", enum_name))?;
    let (_, payload_ty, _) = info.variants.iter()
        .find(|(n, _, _)| n == &variant_name)
        .ok_or_else(|| format!("Unknown variant '{}'", variant_name))?;
    if let Some(inner_ty) = payload_ty {
        emit_variant_payload_bindings(ctx, out, field_patterns, inner_ty,
            scrutinee, scrutinee_ty, info.max_payload_size, local_vars)?;
    }
    Ok(())
}

/// Extract variant payload from an enum and bind field sub-patterns.
#[allow(clippy::too_many_arguments)]
// REASON: 8 args are ctx, out, variant_name, payload_ty, field_patterns,
// scrutinee, idx, local_vars — each independently meaningful
fn emit_variant_payload_bindings(
    ctx: &mut LoweringContext,
    out: &mut String,
    field_patterns: &[Pattern],
    inner_ty: &Type,
    scrutinee: &str,
    scrutinee_ty: &Type,
    max_payload_size: usize,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<(), String> {
    let struct_ty = scrutinee_ty.to_mlir_type(ctx)?;
    let payload_array = format!("%payload_array_{}", ctx.next_id());
    ctx.emit_extractvalue(out, &payload_array, scrutinee, 1, &struct_ty);
    let array_mlir_ty = format!("!llvm.array<{} x i8>", max_payload_size);
    let buf_ptr = format!("%payload_buf_{}", ctx.next_id());
    ctx.emit_alloca(out, &buf_ptr, &array_mlir_ty);
    ctx.emit_store(out, &payload_array, &buf_ptr, &array_mlir_ty);
    let payload_val = format!("%payload_val_{}", ctx.next_id());
    let inner_mlir_ty = inner_ty.to_mlir_type(ctx)?;
    ctx.emit_load(out, &payload_val, &buf_ptr, &inner_mlir_ty);
    if field_patterns.len() == 1 {
        emit_pattern_bindings(ctx, out, &field_patterns[0], &payload_val, inner_ty, local_vars)?;
    } else if let Type::Tuple(field_tys) = inner_ty {
        let tuple_mlir_ty = inner_ty.to_mlir_type(ctx)?;
        for (i, (field_pat, field_ty)) in field_patterns.iter().zip(field_tys.iter()).enumerate() {
            let field_val = format!("%variant_field_{}_{}", i, ctx.next_id());
            ctx.emit_extractvalue(out, &field_val, &payload_val, i, &tuple_mlir_ty);
            emit_pattern_bindings(ctx, out, field_pat, &field_val, field_ty, local_vars)?;
        }
    }
    Ok(())
}

/// Emit pattern bindings (introduce variables from pattern into scope)
fn emit_pattern_bindings(
    ctx: &mut LoweringContext,
    out: &mut String,
    pattern: &Pattern,
    scrutinee: &str,
    scrutinee_ty: &Type,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<(), String> {
    match pattern {
        Pattern::Ident { name, mutable: _ } => {
            local_vars.insert(name.to_string(), (scrutinee_ty.clone(), LocalKind::SSA(scrutinee.to_string())));
            Ok(())
        }
        Pattern::Wildcard | Pattern::Literal { .. } => {
            Ok(())
        }
        Pattern::Or(patterns) => {
            // For OR patterns, only bind from the first alternative
            // (All alternatives must bind the same names with same types)
            if let Some(first) = patterns.first() {
                emit_pattern_bindings(ctx, out, first, scrutinee, scrutinee_ty, local_vars)?;
            }
            Ok(())
        }
        Pattern::Variant { path, fields } => {
            emit_variant_pattern_bindings(ctx, out, path, fields, scrutinee, scrutinee_ty, local_vars)
        }
        Pattern::Tuple(sub_patterns) => {
            let field_types = match scrutinee_ty {
                Type::Tuple(tys) => tys.clone(),
                _ => return Err(format!("Cannot bind tuple pattern on non-tuple type: {:?}", scrutinee_ty)),
            };
            
            let struct_ty = scrutinee_ty.to_mlir_type(ctx)?;
            
            for (i, (sub_pat, field_ty)) in sub_patterns.iter().zip(field_types.iter()).enumerate() {
                let field_val = format!("%tuple_bind_{}_{}", i, ctx.next_id());
                ctx.emit_extractvalue(out, &field_val, scrutinee, i, &struct_ty);
                emit_pattern_bindings(ctx, out, sub_pat, &field_val, field_ty, local_vars)?;
            }
            Ok(())
        }
        Pattern::Struct { name, fields } => {
            let struct_name = match scrutinee_ty {
                Type::Struct(n) => n.clone(),
                Type::Concrete(n, _) => n.clone(),
                _ => return Err(format!("Cannot bind struct pattern on non-struct type: {:?}", scrutinee_ty)),
            };
            
            let info = ctx.struct_registry().values()
                .find(|i| i.name == struct_name || i.name.ends_with(&format!("__{}", name)))
                .cloned()
                .ok_or_else(|| format!("Unknown struct '{}' in pattern binding", name))?;
            
            let struct_mlir_ty = scrutinee_ty.to_mlir_type(ctx)?;
            
            for pat_field in fields {
                let (field_offset, field_ty) = info.fields.get(&pat_field.name.to_string())
                    .ok_or_else(|| format!("Unknown field '{}' in struct '{}'", pat_field.name, name))?
                    .clone();
                
                let field_val = format!("%struct_bind_{}_{}", pat_field.name, ctx.next_id());
                ctx.emit_extractvalue(out, &field_val, scrutinee, field_offset, &struct_mlir_ty);
                
                // If pattern is None, bind to the field name itself
                let sub_pat = pat_field.pattern.as_ref()
                    .cloned()
                    .unwrap_or_else(|| Pattern::Ident { name: pat_field.name.clone(), mutable: false });
                
                emit_pattern_bindings(ctx, out, &sub_pat, &field_val, &field_ty, local_vars)?;
            }
            Ok(())
        }
        Pattern::Rest => {
            // Rest pattern (..) doesn't bind anything
            Ok(())
        }
    }
}

// ============================================================================
// PHASE 6: Let-Else Codegen
// ============================================================================

/// Emit let-else statement
pub fn emit_let_else(
    ctx: &mut LoweringContext,
    out: &mut String,
    let_else: &LetElse,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<bool, String> {
    let (init_val, init_ty) = emit_expr(ctx, out, &let_else.init, local_vars, None)?;
    
    let bind_label = format!("let_else_bind_{}", ctx.next_id());
    let else_label = format!("let_else_else_{}", ctx.next_id());
    let continue_label = format!("let_else_continue_{}", ctx.next_id());
    
    if let_else.pattern.is_irrefutable() {
        emit_pattern_bindings(ctx, out, &let_else.pattern, &init_val, &init_ty, local_vars)?;
        return Ok(false);
    }
    
    let cond = emit_pattern_condition(ctx, out, &let_else.pattern, &init_val, &init_ty)?;
    
    out.push_str(&format!("    cf.cond_br {}, ^{}, ^{}\n", cond, bind_label, else_label));
    
    out.push_str(&format!("  ^{}:\n", bind_label));
    emit_pattern_bindings(ctx, out, &let_else.pattern, &init_val, &init_ty, local_vars)?;
    out.push_str(&format!("    cf.br ^{}\n", continue_label));
    
    out.push_str(&format!("  ^{}:\n", else_label));
    let mut else_vars = local_vars.clone();
    let else_diverges = emit_block(ctx, out, &let_else.else_block.stmts, &mut else_vars)?;
    
    if !else_diverges {
        out.push_str("    // WARNING: let-else else block must diverge\n");
        out.push_str("    llvm.unreachable\n");
    }
    
    out.push_str(&format!("  ^{}:\n", continue_label));
    
    Ok(false)
}

pub fn emit_cleanup_for_return(ctx: &mut LoweringContext, out: &mut String, local_vars: &HashMap<String, (Type, LocalKind)>) -> Result<(), String> {
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


