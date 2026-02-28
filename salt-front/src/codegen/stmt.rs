use crate::grammar::{Stmt, SaltBlock, SaltElse, SaltFor, SaltMatch, LetElse};
use crate::grammar::pattern::Pattern;
use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use crate::codegen::expr::emit_expr;
use crate::codegen::type_bridge::{resolve_type, promote_numeric};
use std::collections::{HashMap, HashSet};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Try to extract a constant integer from an expression for affine loop bounds.
/// Returns Some(value) if the expression is a compile-time constant literal.
fn try_extract_const_int(expr: &syn::Expr) -> Option<i64> {
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
fn block_has_control_flow(stmts: &[Stmt]) -> bool {
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
            _ => {}
        }
    }
    false
}

/// Check if an if expression can be lowered to arith.select (no true control flow).
/// True for: `if cond { literal } else { literal }` patterns.
fn is_select_compatible_if(expr: &syn::ExprIf) -> bool {
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
fn is_simple_expr_block(block: &syn::Block) -> bool {
    if block.stmts.len() != 1 {
        return false;
    }
    match &block.stmts[0] {
        syn::Stmt::Expr(e, _) => is_simple_scalar_expr(e),
        _ => false,
    }
}

/// Check if an expression is a simple scalar value (literal, variable, simple arithmetic).
fn is_simple_scalar_expr(expr: &syn::Expr) -> bool {
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
fn expr_has_if(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::If(if_expr) => !is_select_compatible_if(if_expr),  // Allow select-compatible
        syn::Expr::Binary(b) => expr_has_if(&b.left) || expr_has_if(&b.right),
        syn::Expr::Unary(u) => expr_has_if(&u.expr),
        syn::Expr::Call(c) => c.args.iter().any(|a| expr_has_if(a)),
        syn::Expr::MethodCall(m) => m.args.iter().any(|a| expr_has_if(a)),
        syn::Expr::Reference(r) => expr_has_if(&r.expr),
        syn::Expr::Paren(p) => expr_has_if(&p.expr),
        syn::Expr::Field(f) => expr_has_if(&f.base),
        syn::Expr::Index(i) => expr_has_if(&i.expr) || expr_has_if(&i.index),
        syn::Expr::Block(b) => b.block.stmts.iter().any(|s| match s {
            syn::Stmt::Expr(e, _) => expr_has_if(e),
            _ => false,
        }),
        _ => false,
    }
}

/// [V25.8] Sovereign Body Analysis: Detect if statements contain tensor indexing
/// Returns true if any statement uses tensor/array indexing (A[i,j] pattern)
/// This indicates the loop benefits from polyhedral optimization (affine.for)
fn has_tensor_indexing(stmts: &[Stmt]) -> bool {
    for stmt in stmts {
        match stmt {
            // Check expressions for Index operations
            Stmt::Expr(expr, _) => {
                if expr_has_tensor_indexing(expr) {
                    return true;
                }
            }
            // Recurse into nested for-loops (critical for triple-nested matmul!)
            Stmt::For(salt_for) => {
                if has_tensor_indexing(&salt_for.body.stmts) {
                    return true;
                }
            }
            // Recurse into if/while blocks
            Stmt::If(salt_if) => {
                if has_tensor_indexing(&salt_if.then_branch.stmts) {
                    return true;
                }
                if let Some(else_branch) = &salt_if.else_branch {
                    match else_branch.as_ref() {
                        SaltElse::Block(b) => {
                            if has_tensor_indexing(&b.stmts) { return true; }
                        }
                        SaltElse::If(nested_if) => {
                            if has_tensor_indexing(&nested_if.then_branch.stmts) { return true; }
                        }
                    }
                }
            }
            Stmt::While(salt_while) => {
                if has_tensor_indexing(&salt_while.body.stmts) {
                    return true;
                }
            }
            // Check local variable initializers in syn::Stmt (wrapped)
            Stmt::Syn(syn::Stmt::Local(syn::Local { init: Some(syn::LocalInit { expr, .. }), .. })) => {
                if expr_has_tensor_indexing(expr) {
                    return true;
                }
            }
            Stmt::Syn(syn::Stmt::Expr(expr, _)) => {
                if expr_has_tensor_indexing(expr) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// Check if a syn::Expr contains tensor/array indexing (Index expressions)
fn expr_has_tensor_indexing(expr: &syn::Expr) -> bool {
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
        syn::Expr::Call(c) => c.args.iter().any(|a| expr_has_tensor_indexing(a)),
        syn::Expr::MethodCall(m) => expr_has_tensor_indexing(&m.receiver) || m.args.iter().any(|a| expr_has_tensor_indexing(a)),
        
        _ => false,
    }
}

/// Emit an scf.for loop with Sovereign Narrowing for constant-bound loops.
/// [V25.8] Source-Level IV Narrowing: Use i32 when bounds fit, eliminating index_cast overhead.
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
    // If so, we can emit iter_args for register-resident accumulation
    if let Some(reduction_info) = detect_reduction_pattern(&f.body.stmts, local_vars) {
        return emit_affine_for_reduction(ctx, out, f, lb, ub, local_vars, &var_name, reduction_info);
    }
    
    // [V25.8] Sovereign Body Analysis: Detect loop intent from body contents
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
                p.path.segments[0].ident.to_string() == acc_name
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
        
        if func_name != "vector_fma" {
            continue;
        }
        
        // vector_fma(a, b, acc) - third arg must be the same accumulator
        if call.args.len() != 3 {
            continue;
        }
        
        let third_arg_is_acc = match &call.args[2] {
            syn::Expr::Path(p) if p.path.segments.len() == 1 => {
                p.path.segments[0].ident.to_string() == acc_name
            }
            _ => false,
        };
        
        if !third_arg_is_acc {
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


/// Check if an expression is a tensor index pattern like `tensor[(i, j)]`
/// Returns (tensor_name, indices_exprs) if matched
fn extract_tensor_index(expr: &syn::Expr) -> Option<(String, Vec<&syn::Expr>)> {
    // Pattern: tensor[(i, j)] or tensor[(i,)]
    // This is typically Index(Path, Tuple)
    if let syn::Expr::Index(idx) = expr {
        // Get tensor name
        let tensor_name = match idx.expr.as_ref() {
            syn::Expr::Path(p) if p.path.segments.len() == 1 => {
                p.path.segments[0].ident.to_string()
            }
            _ => return None,
        };
        
        // Get indices from tuple
        let indices = match idx.index.as_ref() {
            syn::Expr::Tuple(t) => t.elems.iter().collect(),
            _ => return None,
        };
        
        return Some((tensor_name, indices));
    }
    None
}

/// Check if two tensor index expressions reference the same location
fn same_tensor_index(lhs: &syn::Expr, rhs_lhs: &syn::Expr) -> bool {
    // Both must be tensor indices with same tensor name and index expressions
    let lhs_info = extract_tensor_index(lhs);
    let rhs_info = extract_tensor_index(rhs_lhs);
    
    match (lhs_info, rhs_info) {
        (Some((name1, idx1)), Some((name2, idx2))) => {
            // Same tensor name and same number of indices
            if name1 != name2 || idx1.len() != idx2.len() {
                return false;
            }
            // For now, assume indices match if structure matches
            // (full expression comparison would be more complex)
            true
        }
        _ => false,
    }
}

/// Detect tensor-in-place reduction: `w[i,j] = w[i,j] - expr`
/// This handles weight update patterns in backward pass
#[allow(dead_code)]
fn detect_tensor_reduction(stmt: &Stmt) -> bool {
    let assign = match stmt {
        Stmt::Syn(syn::Stmt::Expr(syn::Expr::Assign(a), _)) => a,
        Stmt::Expr(syn::Expr::Assign(a), _) => a,
        _ => return false,
    };
    
    // LHS must be tensor index
    let _lhs_tensor = match extract_tensor_index(assign.left.as_ref()) {
        Some(info) => info,
        None => return false,
    };
    
    // RHS must be: same_tensor_index +/- expr
    let rhs_binary = match assign.right.as_ref() {
        syn::Expr::Binary(b) => b,
        _ => return false,
    };
    
    // Check if LHS of binary matches LHS assignment target
    if !same_tensor_index(assign.left.as_ref(), rhs_binary.left.as_ref()) {
        return false;
    }
    
    // Must be + or -
    matches!(rhs_binary.op, 
        syn::BinOp::Add(_) | syn::BinOp::Sub(_)
    )
}

/// Emit an scf.for with iter_args for reduction patterns.
/// This keeps the accumulator in a register instead of memory.
/// 
/// Pattern: `for j in 0..K { acc = vector_fma(a, b, acc); }`
/// Becomes: `%result = scf.for %j = 0 to K iter_args(%acc = %init) -> (vector<8xf32>) { ... scf.yield %next }`
/// 
/// V7 Upgrade: Now uses scf.for instead of affine.for for better compatibility with
/// multi-statement bodies containing vector operations.
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
        Type::Concrete(name, _) if name == "Vector8f32" => "vector<8xf32>".to_string(),
        Type::Concrete(name, _) if name == "Vector4f64" => "vector<4xf64>".to_string(),
        Type::Concrete(name, _) if name == "Vector16f32" => "vector<16xf32>".to_string(),
        _ => return Err(format!("Reduction accumulator must be f32, f64, or Vector type, got {:?}", reduction.ty)),
    };
    
    // Generate unique IDs
    let iv = format!("%iv_{}", ctx.next_id());
    let result_ssa = format!("%reduction_result_{}", ctx.next_id());
    let iter_acc = format!("%iter_acc_{}", ctx.next_id());
    
    // For alloca-based accumulators, we need to load the initial value first
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
    
    // [V25.8] Sovereign Narrowing: Determine if we can use i32 for the body
    // scf.for requires index type for bounds
    let can_narrow = ub < 2_147_483_647 && lb >= 0;
    
    // Emit index type bound constants for scf.for (required by MLIR)
    let lb_ssa = format!("%lb_{}", ctx.next_id());
    let ub_ssa = format!("%ub_{}", ctx.next_id());
    let step_ssa = format!("%step_{}", ctx.next_id());
    out.push_str(&format!("    {} = arith.constant {} : index\n", lb_ssa, lb));
    out.push_str(&format!("    {} = arith.constant {} : index\n", ub_ssa, ub));
    out.push_str(&format!("    {} = arith.constant 1 : index\n", step_ssa));
    
    // Emit scf.for with iter_args (V7: scf.for is more flexible than affine.for)
    // Pattern: %result = scf.for %i = lb to ub step 1 iter_args(%acc = %init) -> (type) { ... }
    out.push_str(&format!(
        "    {} = scf.for {} = {} to {} step {} iter_args({} = {}) -> ({}) {{\n",
        result_ssa, iv, lb_ssa, ub_ssa, step_ssa, iter_acc, init_value_ssa, mlir_ty
    ));
    
    // Enter affine context (still use this for nested optimizations)
    ctx.enter_affine_context();
    
    // [V8] Enable fast-math context for constant-bound reduction body
    // Matches the pattern already used in emit_scf_for_runtime_reduction.
    // Without this, LLVM cannot vectorize constant-bound reductions (e.g., for i in 0..128)
    ctx.emission.in_fast_math_reduction = true;
    
    // [V25.8] Narrow the IV inside the loop if possible
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
    
    // [V8] Reset fast-math context after reduction body
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

/// [V7.4] Emit scf.for with iter_args for runtime-bound reduction patterns.
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
        Type::Concrete(name, _) if name == "Vector8f32" => "vector<8xf32>".to_string(),
        Type::Concrete(name, _) if name == "Vector4f64" => "vector<4xf64>".to_string(),
        Type::Concrete(name, _) if name == "Vector16f32" => "vector<16xf32>".to_string(),
        _ => return Err(format!("V7.4 Reduction accumulator must be f32, f64, or Vector type, got {:?}", reduction.ty)),
    };
    
    // Extract range bounds from the for-loop iterator
    let (start_expr, end_expr) = match &f.iter {
        syn::Expr::Range(r) => (&r.start, &r.end),
        _ => return Err("V7.4 scf.for requires range iterator".to_string()),
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
        return Err("V7.4 scf.for requires finite upper bound".to_string());
    };
    
    // [V25.8] Convert bounds to index type for scf.for (required by MLIR)
    // Determine if we can narrow the IV to i32 inside the loop
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
    
    // For alloca-based accumulators, we need to load the initial value first
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
    
    // [V25.8] Narrow the IV inside the loop if possible
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
    // This means `sum` now refers to the register-resident iter_acc
    body_vars.insert(
        reduction.accumulator_var.clone(),
        (reduction.ty.clone(), LocalKind::SSA(iter_acc.clone()))
    );
    
    // [V7.5] Enable fast-math context for reduction body
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
            _ => return Err("V7.4 Reduction update must be an assignment".to_string()),
        };
        let (val, _) = emit_expr(ctx, out, assign.right.as_ref(), &mut body_vars, Some(&reduction.ty))?;
        val
    };
    
    // Emit scf.yield with the new accumulator value
    out.push_str(&format!("      scf.yield {} : {}\n", next_val, mlir_ty));
    out.push_str("    }\n");
    
    // [V7.5] Reset fast-math context after reduction body
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
// V8: SIMPLE SCF.FOR — Non-Reduction Runtime-Bound Loops
// ============================================================================

/// [V8] Emit scf.for for runtime-bound non-reduction loops.
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
    body_vars.insert(var_name.clone(), (Type::I64, LocalKind::SSA(iv_i64)));
    
    ctx.enter_affine_context();
    
    // Emit body
    let _body_diverges = emit_block(ctx, out, &f.body.stmts, &mut body_vars)?;
    
    ctx.exit_affine_context();
    
    // Close scf.for
    out.push_str("    }\n");
    
    Ok(false)
}

// ============================================================================
// V2.2 SHADOW REDUCTION: FFB Saturated Loop Emission
// ============================================================================

/// Information about a detected shadow update (tensor in-place modification)
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ShadowUpdateInfo {
    /// Tensor name being updated
    tensor_name: String,
    /// Index expressions (as strings for now)
    indices: Vec<String>,
    /// The delta expression to add
    delta_expr: String,
}

/// Detect if a statement is an update_tensor intrinsic call
/// Returns Some(ShadowUpdateInfo) if it's an update_tensor, None otherwise
#[allow(dead_code)]
fn detect_update_tensor_call(stmt: &Stmt) -> Option<ShadowUpdateInfo> {
    // Look for expression statements that are function calls to update_tensor
    let call = match stmt {
        Stmt::Syn(syn::Stmt::Expr(syn::Expr::Call(c), _)) => c,
        Stmt::Expr(syn::Expr::Call(c), _) => c,
        _ => return None,
    };
    
    // Check function name is update_tensor
    let func_name = match call.func.as_ref() {
        syn::Expr::Path(p) if p.path.segments.len() == 1 => {
            p.path.segments[0].ident.to_string()
        }
        _ => return None,
    };
    
    if func_name != "update_tensor" {
        return None;
    }
    
    // We found an update_tensor call - but we can't easily extract the args here
    // For now, return a placeholder that marks this as a shadow update
    Some(ShadowUpdateInfo {
        tensor_name: "detected".to_string(),
        indices: vec![],
        delta_expr: "".to_string(),
    })
}

/// Check if a loop body contains update_tensor calls (candidates for lifting)
#[allow(dead_code)]
fn has_shadow_updates(stmts: &[Stmt]) -> bool {
    for stmt in stmts {
        if detect_update_tensor_call(stmt).is_some() {
            return true;
        }
        // Check nested statements
        match stmt {
            Stmt::For(f) => {
                if has_shadow_updates(&f.body.stmts) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}



/// [ITERATOR PROTOCOL] Lower `for x in iter` to a while-loop with `.next()` calls.
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

    // [PILLAR 2: Global LVN] Clear cache at loop header entry
    ctx.emission.global_lvn.clear();

    // Heartbeat Injection (V2.0)
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
            .ok_or_else(|| format!("[ITER PROTOCOL] Cannot find Option enum in registry for {:?}", next_ty))?;
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
                .ok_or_else(|| format!("[ITER PROTOCOL] Cannot find enum '{}' in registry", name))?;
            let (_vname, payload, _disc) = info.variants.iter()
                .find(|(n, _, _)| n == "Some")
                .ok_or_else(|| format!("[ITER PROTOCOL] Enum '{}' has no 'Some' variant", name))?;
            let inner = payload.clone()
                .ok_or_else(|| format!("[ITER PROTOCOL] Option 'Some' variant has no payload type"))?;
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
                    .ok_or_else(|| format!("[ITER PROTOCOL] Enum has no 'Some' variant"))?;
                let inner = payload.clone()
                    .ok_or_else(|| format!("[ITER PROTOCOL] Option 'Some' has no payload"))?;
                (inner, info.max_payload_size)
            } else if !args.is_empty() {
                // Fallback: use the first generic arg as the payload type
                // For Option<i64>, max_payload_size is 8
                let inner = args[0].clone();
                let size = 8usize; // i64 = 8 bytes
                (inner, size)
            } else {
                return Err(format!("[ITER PROTOCOL] Cannot determine payload type for {:?}", next_ty));
            }
        },
        _ => return Err(format!("[ITER PROTOCOL] next() must return Option<T>, got {:?}", next_ty)),
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


/// [v0.9.2] Check if a Salt block always returns (is a terminal path).
/// Used for implicit guard negation: after `if cond { return x; }`, remaining code
/// implicitly runs under `!cond`.
pub fn salt_block_always_returns(stmts: &[Stmt]) -> bool {
    for stmt in stmts {
        match stmt {
            // Direct Salt grammar return
            Stmt::Return(_) => return true,
            // Salt grammar Expr wrapping syn::Expr::Return (e.g., `return -x;` parsed as expression)
            Stmt::Expr(syn::Expr::Return(_), _) => return true,
            // syn fallback: Expr::Return with optional semicolon
            Stmt::Syn(syn::Stmt::Expr(syn::Expr::Return(_), _)) => return true,
            // If-else: returns only if BOTH branches return
            Stmt::If(f) => {
                if let Some(else_branch) = &f.else_branch {
                    let then_returns = salt_block_always_returns(&f.then_branch.stmts);
                    let else_returns = match else_branch.as_ref() {
                        SaltElse::Block(b) => salt_block_always_returns(&b.stmts),
                        SaltElse::If(nested) => salt_block_always_returns(&nested.then_branch.stmts),
                    };
                    if then_returns && else_returns {
                        return true;
                    }
                }
            }
            _ => {}
        }
    }
    false
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

        // [v0.9.2] Implicit Guard Negation for path-sensitive postcondition verification.
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
                    
                    if !local_vars.contains_key(&name) {
                        let ty = if let syn::Pat::Type(pt) = &local.pat {
                            resolve_type(ctx, &crate::grammar::SynType::from_std(*pt.ty.clone()).unwrap())
                        } else if let Some(_init) = &local.init {
                            // HEURISTIC: Try to infer type from init expression ONLY if it's a simple literal or known variable.
                            // In a real compiler, we'd do a full type inference pass.
                            // For Salt, we prefer explicit types or well-behaved inference.
                            // We'll let emit_stmt handle inferring and hoisting if we skip it here.
                            continue;
                        } else {
                            Type::I32
                        };
                        
                        let alloca = format!("%local_{}_{}", name, ctx.next_id());
                        let mlir_ty = ty.to_mlir_storage_type(ctx)?;
                        ctx.emit_alloca(&mut String::new(), &alloca, &mlir_ty);
                        local_vars.insert(name, (ty, LocalKind::Ptr(alloca)));
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
            Stmt::If(f) => {
                let mut then_vars = local_vars.clone();
                hoist_allocas_in_block(ctx, &f.then_branch.stmts, &mut then_vars)?;
                if let Some(eb) = &f.else_branch {
                    let mut else_vars = local_vars.clone();
                    match eb.as_ref() {
                        SaltElse::Block(b) => { hoist_allocas_in_block(ctx, &b.stmts, &mut else_vars)?; }
                        SaltElse::If(nested) => { hoist_allocas_in_block(ctx, &nested.then_branch.stmts, &mut else_vars)?; }
                    }
                }
            }
            Stmt::For(f) => {
                let mut inner_vars = local_vars.clone();
                hoist_allocas_in_block(ctx, &f.body.stmts, &mut inner_vars)?;
            }
            Stmt::Unsafe(b) => {
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
            syn::Stmt::Local(local) => {
                let pat = match &local.pat {
                    syn::Pat::Type(pt) => &pt.pat,
                    p => p,
                };
                let name = if let syn::Pat::Ident(id) = pat { id.ident.to_string() } else { "".to_string() };
                if !name.is_empty() && local_vars.contains_key(&name) {
                    // Variable was hoisted as a Ptr.
                    let (ty, kind) = local_vars.get(&name).unwrap().clone();
                        if let Some(init) = &local.init {
                            // [V25.2] Domain Isolation: Don't pass Pointer hints to RHS
                            // This prevents Type Osmosis in expressions like train_images + (i * INPUT_SIZE)
                            let hint = if ty.k_is_ptr_type() { None } else { Some(&ty) };
                            let (val, init_ty) = emit_expr(ctx, out, &init.expr, local_vars, hint)?;
                            let val_prom = crate::codegen::type_bridge::promote_numeric(ctx, out, &val, &init_ty, &ty)?;
                            if let LocalKind::Ptr(ptr) = kind {
                                 ctx.emit_store_logical(out, &val_prom, &ptr, &ty)?;
                            }

                            // [Z3 REGISTRATION] Register variable = init_expr in Z3.
                            // Translate the init expression AST directly to a Z3 value.
                            // This enables loop invariant base-case proofs: Z3 knows
                            // `i = 0` from `let mut i: i64 = 0`.
                            if !ctx.config.no_verify && ty.is_integer() {
                                if let Ok(z3_val) = crate::codegen::expr::translate_to_z3(
                                    ctx, &init.expr, local_vars
                                ) {
                                    use z3::ast::Ast;
                                    let z3_var = ctx.mk_var(&name);
                                    ctx.z3_solver.assert(&z3_var._eq(&z3_val));
                                }
                            }
                        }
                } else {
                    // [PHASE 7: Bidirectional Type Inference]
                    // Extract type annotation FIRST to use as hint for emit_expr
                    // This enables turbofish elimination: `let x: Vec<u8> = Vec::new()`
                    let type_hint: Option<Type> = match &local.pat {
                        syn::Pat::Type(pt) => Some(resolve_type(ctx, &crate::grammar::SynType::from_std(*pt.ty.clone()).unwrap())),
                        _ => None,
                    };
                    
                    let (val, actual_ty) = if let Some(init) = &local.init {
                        emit_expr(ctx, out, &init.expr, local_vars, type_hint.as_ref())?
                    } else {
                        ("%c0".to_string(), Type::I32)
                    };
                    
                    
                    // Use type hint if provided, otherwise use inferred type
                    let target_ty = type_hint.unwrap_or_else(|| actual_ty.clone());
                    
                    emit_pattern(ctx, out, &local.pat, val, actual_ty, target_ty.clone(), local_vars)?;

                    // [Z3 REGISTRATION] Register integer literal let-bindings with Z3
                    // This enables the loop invariant base-case checker to know
                    // variable values at the point of the while loop.
                    if !ctx.config.no_verify && !name.is_empty() && target_ty.is_integer() {
                        if let Some(init) = &local.init {
                            if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(li), .. }) = &*init.expr {
                                if let Ok(int_val) = li.base10_parse::<i64>() {
                                    use z3::ast::Ast;
                                    let z3_var = ctx.mk_var(&name);
                                    let z3_val = ctx.mk_int(int_val);
                                    ctx.z3_solver.assert(&z3_var._eq(&z3_val));
                                }
                            }
                        }
                    }
                }
                
                // [SOVEREIGN V5.0] Malloc tracking via DAG-based MallocTracker.
                // If the RHS was a malloc() call, pending_malloc_result was set by expr/mod.rs.
                // Register the allocation with the MallocTracker DAG.
                if !name.is_empty() {
                    let pending = ctx.pending_malloc_result.take();
                    if pending.is_some() {
                        let alloc_id = format!("malloc:{}", name);
                        ctx.malloc_tracker.track(
                            alloc_id,
                            format!("malloc at {}", name),
                        );
                    }

                    // [ESCAPE ANALYSIS] Cast propagation: `let ctrl = ctrl_addr as Ptr<i8>`
                    // If the RHS is a cast over a malloc-tracked variable, propagate via
                    // link_dependency: the cast result depends on the source allocation.
                    if let Some(init) = &local.init {
                        if let syn::Expr::Cast(c) = &*init.expr {
                            if let syn::Expr::Path(p) = &*c.expr {
                                if p.path.segments.len() == 1 {
                                    let src = p.path.segments[0].ident.to_string();
                                    let src_alloc_id = format!("malloc:{}", src);
                                    if ctx.malloc_tracker.contains_alloc(&src_alloc_id) {
                                        ctx.malloc_tracker.link_dependency(
                                            name.clone(),
                                            src_alloc_id,
                                        );
                                    }
                                }
                            }
                        }
                    }

                    // [ESCAPE ANALYSIS] Consume pending struct dependencies.
                    // If the RHS was a struct construction, __pending_struct edges were
                    // created by emit_struct. Migrate them to this variable name.
                    ctx.malloc_tracker.drain_pending_to(&name);

                    // [SALT MEMORY MODEL] Consume pending pointer state.
                    // If the RHS was a Ptr::empty(), Box::new(), or Arena::alloc(),
                    // register the pointer state for this variable.
                    let pending_state = ctx.pending_pointer_state.take();
                    if let Some(state) = pending_state {
                        match state {
                            crate::codegen::verification::PointerState::Empty => {
                                ctx.pointer_tracker.mark_empty(&name);
                            }
                            crate::codegen::verification::PointerState::Valid => {
                                ctx.pointer_tracker.mark_valid(&name);
                            }
                            crate::codegen::verification::PointerState::Optional => {
                                ctx.pointer_tracker.mark_optional(&name);
                            }
                        }
                    }

                    // [ARENA ESCAPE ANALYSIS] Scope Ladder — Depth-based taint tracking.
                    // Detect Arena variable declarations and Arena::alloc calls.
                    if let Some(init) = &local.init {
                        // Hook 1: Arena Registration
                        // If the RHS constructs an Arena (Arena::new(...)), register
                        // this variable as an arena at the current scope depth.
                        if is_arena_constructor(&init.expr) {
                            ctx.arena_escape_tracker.register_arena(&name);
                        }

                        // Hook 2: Alloc Provenance
                        // If the RHS is arena.alloc(...) or arena.alloc_array(...),
                        // the result pointer inherits the arena's depth.
                        if let Some(arena_name) = extract_arena_alloc_receiver(&init.expr) {
                            ctx.arena_escape_tracker.register_alloc(&name, &arena_name);
                        }

                        // Hook 3: ArenaAllocator Provenance
                        // If the RHS is `ArenaAllocator { arena: my_arena }`, the allocator
                        // inherits the arena's depth. This bridges Arena → ArenaAllocator.
                        if let Some(arena_name) = extract_arena_allocator_source(&init.expr) {
                            ctx.arena_escape_tracker.register_arena_allocator(&name, &arena_name);
                        }

                        // Hook 4: Vec Allocator Provenance
                        // If the RHS is `Vec::new(alloc, cap)`, the Vec inherits the
                        // allocator's depth. This bridges ArenaAllocator → Vec.
                        if let Some(alloc_name) = extract_vec_new_allocator(&init.expr) {
                            ctx.arena_escape_tracker.register_vec_from_allocator(&name, &alloc_name);
                        }
                    }
                }
                
                Ok(false)
            }
            syn::Stmt::Expr(e, semi) => {
                let (val, _) = emit_expr(ctx, out, e, local_vars, None)?;
                let is_return = matches!(e, syn::Expr::Return(_));
                Ok((semi.is_none() && val == "%unreachable") || is_return)
            }
            // [V5.0 STRUCTURAL FORMATTING FIX] Handle macro statements
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
        Stmt::While(w) => {
            let label_header = format!("while_header_{}", ctx.next_id());
            let label_body = format!("while_body_{}", ctx.next_id());
            let label_exit = format!("while_exit_{}", ctx.next_id());
            
            out.push_str(&format!("    cf.br ^{}\n", label_header));
            out.push_str(&format!("  ^{}:\n", label_header));
            
            let (cond_val, cond_ty) = emit_expr(ctx, out, &w.cond, local_vars, None)?;
            // [POINTER TRUTHINESS] Accept Pointer types as while conditions
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
            
            // Heartbeat Injection (V2.0: simplified, uses @yielding at function level)
            if !*ctx.no_yield() {
                ctx.emit_lto_hook(out, "__salt_yield_check", &[], local_vars, None)?;
            }
            ctx.break_labels_mut().push(label_exit.clone());
            ctx.continue_labels_mut().push(label_header.clone());
            let mut body_vars = local_vars.clone();

            // === Z3 HOARE LOGIC: While Loop Verification ===
            // Strict Hoare triple: {I} while(C) {I ∧ C} body {I} → {I ∧ ¬C}
            //
            // Phase A: Base case — prove invariants hold before loop entry
            // Phase B: Inductive step — havoc, assume I∧C, emit body, verify I
            // Phase C: Post-loop — assert ¬C for subsequent code
            if !ctx.config.no_verify {
                // Collect invariants from the loop body
                let sym_ctx = crate::codegen::verification::SymbolicContext::new(ctx.z3_ctx);
                let mut invariant_exprs: Vec<syn::Expr> = Vec::new();
                for stmt in &w.body.stmts {
                    if let Stmt::Invariant(expr) = stmt {
                        invariant_exprs.push(expr.clone());
                    }
                }

                // --- Phase A: Base Case ---
                // Variable values are already registered in Z3 at the let-binding
                // emission point (see [Z3 REGISTRATION] above). This gives us
                // constraints like `i = 0` that enable invariant base-case proofs.
                ctx.z3_solver.push(); // Temporary scope for base-case checks

                // Prove each invariant holds with current (pre-loop) variable values.
                // Method: assert !I and check for UNSAT.
                //   UNSAT → I always holds → invariant proven ✓
                //   SAT   → found counterexample → invariant violation ✗
                //   Unknown → can't determine → pass conservatively
                for inv_expr in &invariant_exprs {
                    if let Ok(z3_inv) = crate::codegen::expr::translate_bool_to_z3(
                        ctx, inv_expr, &body_vars, &sym_ctx
                    ) {
                        use z3::ast::Ast;
                        ctx.z3_solver.push();
                        ctx.z3_solver.assert(&z3_inv.not());
                        let check = ctx.z3_solver.check();
                        ctx.z3_solver.pop(1);

                        if check == z3::SatResult::Sat {
                            ctx.z3_solver.pop(1); // Pop base-case registration scope
                            return Err(format!(
                                "Z3 verification failed: loop invariant does not hold at entry. \
                                 The solver found a counterexample proving the invariant is false \
                                 with current variable values."
                            ));
                        }

                        // Invariant proven or undecidable: assert as true for reasoning
                        ctx.z3_solver.assert(&z3_inv);
                    }
                }
                ctx.z3_solver.pop(1); // Pop base-case registration scope

                // Re-assert proven invariants in the main scope
                for inv_expr in &invariant_exprs {
                    if let Ok(z3_inv) = crate::codegen::expr::translate_bool_to_z3(
                        ctx, inv_expr, &body_vars, &sym_ctx
                    ) {
                        ctx.z3_solver.assert(&z3_inv);
                    }
                }

                // --- Phase B: Inductive Step Setup ---
                // Push solver scope for the loop interior
                ctx.z3_solver.push();

                // Havoc: erase pre-loop knowledge of all mutated variables
                let mutated = collect_mutations(&w.body.stmts);
                for name in &mutated {
                    if let Some((ty, _)) = body_vars.get(name) {
                        if ty.is_integer() {
                            let fresh_name = format!("{}_havoc_{}", name, ctx.next_id());
                            let z3_var = ctx.mk_var(&fresh_name);
                            ctx.symbolic_tracker.insert(name.clone(), z3_var);
                        }
                    }
                }

                // Assume invariants in the havoc'd state (inductive hypothesis)
                for inv_expr in &invariant_exprs {
                    if let Ok(z3_inv) = crate::codegen::expr::translate_bool_to_z3(
                        ctx, inv_expr, &body_vars, &sym_ctx
                    ) {
                        ctx.z3_solver.assert(&z3_inv);
                    }
                }

                // Assume loop condition is true (we are inside the loop body)
                if let Ok(z3_cond) = crate::codegen::expr::translate_bool_to_z3(
                    ctx, &w.cond, &body_vars, &sym_ctx
                ) {
                    ctx.z3_solver.assert(&z3_cond);
                }
            }

            let body_diverges = emit_block(ctx, out, &w.body.stmts, &mut body_vars)?;
            
            // === Z3 HOARE LOGIC: Post-Body Verification ===
            if !ctx.config.no_verify {
                let sym_ctx = crate::codegen::verification::SymbolicContext::new(ctx.z3_ctx);

                // --- Phase B (cont'd): Inductive Step Check ---
                // After the body, verify invariants still hold.
                // This is NOT currently enforced as a hard error because the body's
                // Z3 state mutations are complex. The push/pop isolates correctly.

                // Pop the inductive step scope
                ctx.z3_solver.pop(1);

                // --- Phase C: Post-Loop ---
                // Assert ¬C: after the loop, the condition is false.
                // This lets Z3 reason about variables in post-loop code.
                if let Ok(z3_cond) = crate::codegen::expr::translate_bool_to_z3(
                    ctx, &w.cond, local_vars, &sym_ctx
                ) {
                    use z3::ast::Ast;
                    ctx.z3_solver.assert(&z3_cond.not());
                }
            }

            ctx.break_labels_mut().pop();
            ctx.continue_labels_mut().pop();
            
            if !body_diverges {
                out.push_str(&format!("    cf.br ^{}\n", label_header));
            }
            out.push_str(&format!("  ^{}:\n", label_exit));
            Ok(false)
        }
        Stmt::If(f) => {
            emit_salt_if(ctx, out, &f.cond, &f.then_branch, &f.else_branch, local_vars)
        }
        Stmt::For(f) => {
            // 1. Extract range bounds
             let (start_expr, end_expr) = match &f.iter {
                 syn::Expr::Range(r) => (&r.start, &r.end),
                 _ => {
                     // Iterator protocol: emit while-loop with .next() calls
                     return emit_iterator_for_loop(ctx, out, f, local_vars);
                 }
             };
            
            // 2. Try to extract constant bounds for affine.for optimization
            let const_start = start_expr.as_ref().and_then(|e| try_extract_const_int(e));
            let const_end = end_expr.as_ref().and_then(|e| try_extract_const_int(e));
            
            // Use affine.for when:
            // 1. Pattern is Pat::Ident (NOT Pat::Wild - wildcards use Regular engine for RAII-Lite)
            // 2. Both bounds are constant
            // 3. Body has no control flow (affine.for requires single-block bodies)
            let is_simple_ident = matches!(&f.pat, syn::Pat::Ident(_));
            let body_has_cf = block_has_control_flow(&f.body.stmts);
            
            if is_simple_ident {
                if let (Some(lb), Some(ub)) = (const_start, const_end) {
                    if !body_has_cf {
                        return emit_affine_for(ctx, out, f, lb, ub, local_vars);
                    }
                }
            }
            
            // [V7.4] Check for reduction pattern in runtime-bound loops
            // This enables scf.for iter_args for dynamic bounds like `for j in 0..cols`
            let is_simple_ident_rt = matches!(&f.pat, syn::Pat::Ident(_));
            let body_has_cf_rt = block_has_control_flow(&f.body.stmts);
            
            if is_simple_ident_rt && !body_has_cf_rt {
                if let Some(reduction_info) = detect_reduction_pattern(&f.body.stmts, local_vars) {
                    // Extract loop variable name
                    if let syn::Pat::Ident(id) = &f.pat {
                        let var_name = id.ident.to_string();
                        return emit_scf_for_runtime_reduction(
                            ctx, out, f, local_vars, &var_name, reduction_info
                        );
                    }
                }
                
                // [V8] Non-reduction simple loop: still use scf.for for structured control flow
                // This handles write loops like `for i in 0..size { out[i] = expr }`
                return emit_scf_for_simple(ctx, out, f, local_vars);
            }
            
            // FALLBACK: Standard cf.br loop for dynamic bounds without reduction pattern
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

            // Infer Loop Type
            let loop_ty = if start_ty == Type::I64 || end_ty == Type::I64 || start_ty == Type::Usize || end_ty == Type::Usize {
                Type::I64 
            } else {
                Type::I32
            };

            let start_val = promote_numeric(ctx, out, &start_val_raw, &start_ty, &loop_ty)?;
            let end_val = promote_numeric(ctx, out, &end_val_raw, &end_ty, &loop_ty)?;
            let mlir_loop_ty = loop_ty.to_mlir_type(ctx)?;

            // 2. Setup loop variable (alloca for mutability/consistency)
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
            
            // [PILLAR 2: Global LVN] Clear cache at loop body entry
            // Each iteration starts fresh - cached values from previous iteration are stale
            ctx.emission.global_lvn.clear();
            
            // Heartbeat Injection (V2.0: simplified, uses @yielding at function level)
            if !*ctx.no_yield() {
                ctx.emit_lto_hook(out, "__salt_yield_check", &[], local_vars, None)?;
            }
            // Add loop variable to local_vars
            let mut body_vars = local_vars.clone();
            let _has_named_pattern = matches!(&f.pat, syn::Pat::Ident(_));
            
            if let syn::Pat::Ident(id) = &f.pat {
                let name = id.ident.to_string();
                body_vars.insert(name, (loop_ty.clone(), LocalKind::SSA(current_i.clone())));
            }
            
            // === Z3 HOARE LOGIC: For Loop Induction Variable Bounds ===
            // Register the induction variable with Z3 and assert domain constraints:
            //   start <= i < end
            // This enables Z3 to prove array bounds checks inside the loop body.
            let _z3_for_loop_active = if !ctx.config.no_verify && (matches!(&f.pat, syn::Pat::Ident(_)) || matches!(&f.pat, syn::Pat::Wild(_))) {
                let z3_i = ctx.mk_var(&current_i);
                ctx.symbolic_tracker.insert(current_i.clone(), z3_i.clone());
                
                ctx.z3_solver.push();
                
                // Assert: i >= 0 (or i >= start if start is not 0)
                let z3_zero = ctx.mk_int(0);
                ctx.z3_solver.assert(&z3_i.ge(&z3_zero));
                
                // Assert upper bound from range expression
                if let syn::Expr::Range(r) = &f.iter {
                    if let Some(end_expr) = &r.end {
                        if let Ok(z3_end) = crate::codegen::expr::translate_to_z3(ctx, end_expr, local_vars) {
                             ctx.z3_solver.assert(&z3_i.lt(&z3_end));
                        }
                    }
                    // Assert lower bound from range start (if explicit)
                    if let Some(start_expr) = &r.start {
                        if let Ok(z3_start) = crate::codegen::expr::translate_to_z3(ctx, start_expr, local_vars) {
                            ctx.z3_solver.assert(&z3_i.ge(&z3_start));
                        }
                    }
                }
                true
            } else {
                false
            };
            
            ctx.break_labels_mut().push(label_exit.clone());
            ctx.continue_labels_mut().push(label_header.clone());
            
            // [V1.1] RAII-Lite: Push cleanup scope for loop body
            ctx.push_cleanup_scope();
            
            let body_diverges = emit_block(ctx, out, &f.body.stmts, &mut body_vars)?;
            ctx.break_labels_mut().pop();
            ctx.continue_labels_mut().pop();

            // === Z3 HOARE LOGIC: Pop for-loop solver scope ===
            if _z3_for_loop_active {
                ctx.z3_solver.pop(1);
            }
            
            if !body_diverges {
                 // [V1.1] RAII-Lite: Emit cleanup before looping back
                 ctx.pop_and_emit_cleanup(out)?;
                 
                 let next_i = format!("%next_i_{}", ctx.next_id());
                 let c1 = format!("%c1_{}", ctx.next_id());
                 ctx.emit_const_int(out, &c1, 1, &mlir_loop_ty);
                 ctx.emit_binop(out, &next_i, "arith.addi", &current_i, &c1, &mlir_loop_ty);
                 ctx.emit_store(out, &next_i, &loop_var_ptr, &mlir_loop_ty);
                 out.push_str(&format!("    cf.br ^{}\n", label_header));
            } else {
                 // If body diverges (has return/break), still need to pop the scope
                 let _ = ctx.cleanup_stack_mut().pop();
            }

            
            // [PILLAR 2: SSA Dominance Fix] Clear global LVN at loop exit
            // Cached values from inside this loop don't dominate code after it
            ctx.emission.global_lvn.clear();
            
            out.push_str(&format!("  ^{}:\n", label_exit));
            Ok(false)
        }
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
        Stmt::Return(opt_expr) => {
            emit_cleanup_for_return(ctx, out, local_vars)?;
            if let Some(e) = opt_expr {
                // [SOVEREIGN FIX] Substitute generics in return type (T -> u8 etc.)
                let expected_ret = ctx.current_ret_ty().clone().map(|t| t.substitute(&ctx.current_type_map()));
                let (val_raw, ty) = emit_expr(ctx, out, e, local_vars, expected_ret.as_ref())?;

                // [ESCAPE ANALYSIS V5.1] Recursive escape marking.
                crate::codegen::expr::mark_expression_escaped(ctx, e);

                // [ARENA ESCAPE ANALYSIS] Law I: The Return Rule.
                // return x is valid iff depth(x) <= 1.
                // A pointer from a local arena (depth >= 2) cannot escape.
                if let Some(var_name) = extract_return_var_name(e) {
                    if let Err(msg) = ctx.arena_escape_tracker.check_return_escape(&var_name) {
                        return Err(msg);
                    }
                }

                // [v0.9.2 POSTCONDITION PIVOT] Z3 verification of ensures clauses at return site
                // Before emitting func.return, verify the postcondition holds for this return value.
                let ensures = ctx.current_ensures().clone();
                if !ensures.is_empty() {
                    // Extract requires and parameter names from the current function
                    let fn_name = ctx.current_fn_name().clone();
                    let file = ctx.config.file;
                    let (requires, param_names) = file.items.iter()
                        .filter_map(|item| {
                            if let crate::grammar::Item::Fn(f) = item {
                                if f.name.to_string() == fn_name || ctx.expansion.current_fn_name.ends_with(&f.name.to_string()) {
                                    let params: Vec<String> = f.args.iter().map(|a| a.name.to_string()).collect();
                                    return Some((f.requires.clone(), params));
                                }
                            }
                            None
                        })
                        .next()
                        .unwrap_or((vec![], vec![]));

                    match crate::codegen::verification::VerificationEngine::verify_postcondition(
                        ctx, &ensures, &requires, e, &param_names, local_vars, &fn_name,
                    ) {
                        Ok(true) => {
                            // Postcondition verified — emit MLIR marker
                            out.push_str(&format!("    // z3_postcondition_verified: ensures proven for '{}'\n", fn_name));
                        }
                        Ok(false) => {
                            // No ensures or couldn't verify — no marker
                        }
                        Err(err) => {
                            return Err(err);
                        }
                    }
                }
                
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
        Stmt::Unsafe(block) => {
            // [SAFETY GATE] Only allow unsafe blocks in privileged packages
            // (std.* and kernel.*). All other packages are rejected.
            // Uses config.file.package as fallback when current_package is None.
            let first_seg = ctx.current_package.as_ref()
                .or_else(|| ctx.config.file.package.as_ref())
                .and_then(|pkg| pkg.name.iter().next().map(|id| id.to_string()));

            let is_privileged = matches!(first_seg.as_deref(), Some("std") | Some("kernel"));

            if !is_privileged {
                return Err("unsafe blocks are not allowed in user code. All unsafe operations must go through the standard library's safe abstractions or be placed in kernel.* packages. See docs/UNSAFE.md.".to_string());
            }

            let was_unsafe = *ctx.is_unsafe_block();
            *ctx.is_unsafe_block_mut() = true;
            let mut inner_vars = local_vars.clone();
            let res = emit_block(ctx, out, &block.stmts, &mut inner_vars)?;
            *ctx.is_unsafe_block_mut() = was_unsafe;
            Ok(res)
        }
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
        Stmt::Loop(body) => {
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

            // [DIVERGENCE TRACKING] Only emit the exit block if a break
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
    }
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
    // [CONSTITUTIONAL GUARD V21.0]: Loop Induction Isolation
    // If we are binding an induction variable (actual=Usize or integer), 
    // we must NOT allow it to be 'magnetized' by a Pointer target.
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

            // TENSOR SPECIAL CASE: Tensors (memrefs) are always SSA - we mutate their contents, not the value
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
            
            // [V1.1] RAII-Lite: Register Vec types for automatic cleanup at scope exit
            if let Type::Concrete(base, args) = &target_ty {
                if base == "Vec" || base.ends_with("__Vec") || base.contains("__vec__Vec") {
                    // Determine the element type suffix for the drop function
                    let elem_suffix = if let Some(elem_ty) = args.first() {
                        elem_ty.mangle_suffix()
                    } else {
                        "T".to_string()
                    };
                    let drop_fn = format!("std__collections__vec__Vec__drop_{}", elem_suffix);
                    
                    // Register the POINTER for cleanup (drop takes &mut self)
                    // Vec is always allocated to a pointer because it's always is_mut=true
                    if let LocalKind::Ptr(ref alloca) = kind {
                        // Use Reference type so the MLIR signature expects !llvm.ptr
                        let ref_ty = Type::Reference(Box::new(target_ty.clone()), true);
                        ctx.register_owned_resource(alloca, &drop_fn, &name, ref_ty);
                    }
                }
            }
            
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
                        // [FIX] cmpxchg tuples store the success flag as native i1,
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
            let struct_name = ps.path.segments.last().unwrap().ident.to_string();
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

// [POINTER SAFETY] Helper to detect `p.addr != 0` or `p.addr == 0` check
fn get_narrowing_target(cond: &syn::Expr) -> Option<(String, bool)> {
    // [POINTER TRUTHINESS] Bare pointer: `if ptr { ... }` => narrowing target = ptr, is_neq=true
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
    // [POINTER TRUTHINESS] Accept Pointer types as if conditions
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

    // [POINTER SAFETY] Flow-Sensitive Narrowing
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

    // [SSA DOMINANCE] Save LVN cache before then-branch
    ctx.emission.global_lvn.push_snapshot();

    out.push_str(&format!("  ^{}:\n", label_then));
    let mut then_vars = local_vars.clone();
    // [v0.9.2] Push branch condition for Z3 postcondition verification
    ctx.emission.path_conditions.push(cond.clone());
    let then_diverges = emit_block(ctx, out, &then_branch.stmts, &mut then_vars)?;
    ctx.emission.path_conditions.pop();
    if !then_diverges {
        out.push_str(&format!("    cf.br ^{}\n", label_merge));
    }

    // [SSA DOMINANCE] Restore LVN cache after then-branch — discard branch-local values
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

        // [SSA DOMINANCE] Save LVN cache before else-branch
        ctx.emission.global_lvn.push_snapshot();

        out.push_str(&format!("  ^{}:\n", label_else));
        let mut else_vars = local_vars.clone();
        // [v0.9.2] Push negated condition for else branch
        let negated_cond = syn::Expr::Unary(syn::ExprUnary {
            attrs: vec![],
            op: syn::UnOp::Not(syn::token::Not::default()),
            expr: Box::new(cond.clone()),
        });
        ctx.emission.path_conditions.push(negated_cond);
        else_diverges = match else_branch.as_ref().unwrap().as_ref() {
            SaltElse::Block(b) => emit_block(ctx, out, &b.stmts, &mut else_vars)?,
            SaltElse::If(nested) => {
                 emit_salt_if(ctx, out, &nested.cond, &nested.then_branch, &nested.else_branch, &mut else_vars)?
            }
        };
        ctx.emission.path_conditions.pop();
        if !else_diverges {
            out.push_str(&format!("    cf.br ^{}\n", label_merge));
        }

        // [SSA DOMINANCE] Restore LVN cache after else-branch
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
    // UNLESS we want to prevent reuse of names? No, reuse is fine if new definition.
    
    // Safety: If a variable is consumed in ANY branch executed, it is consumed.
    // Since we don't know which branch took, we must assume consumed if used in EITHER (for safety).
    // But logically, if I check `if x { move y } else { keep y }`. After: y is maybe moved.
    // Salt requires definitive move? Or partial move tracking?
    // For now, Union is safe (over-conservative).
    // Filtering by `local_vars` ensures we don't leak inner names.
    
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
    
    // [Z3 VERIFICATION] Check exhaustiveness for enum types
    use crate::codegen::verification::{check_exhaustiveness, ExhaustivenessResult};
    match check_exhaustiveness(ctx, &scrutinee_ty, &match_expr.arms) {
        ExhaustivenessResult::Exhaustive => {
            // Good - all variants covered
        }
        ExhaustivenessResult::MissingVariants(missing) => {
            // Report warning (could make this an error in strict mode)
            eprintln!("WARNING: Non-exhaustive match on {:?}, missing variants: {:?}", 
                scrutinee_ty, missing);
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
    
    // Track if any arm doesn't diverge (we need merge block)
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
                // [MATCH GUARD FIX] Pattern bindings must be available in the guard scope.
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
            // Get the enum name (all segments except the last) and variant name (last segment)
            if path.is_empty() {
                return Err("Empty variant path".to_string());
            }
            let variant_name = path.last().unwrap().to_string();
            
            // The scrutinee_ty should be an enum type
            // For specialized generic enums (e.g. Result<File, IOError>), 
            // we need the fully-mangled name to match the enum_registry entry.
            let enum_name = match scrutinee_ty {
                Type::Enum(name) => name.clone(),
                Type::Concrete(_, _) => scrutinee_ty.mangle_suffix(),
                _ => return Err(format!("Cannot match variant on non-enum type: {:?}", scrutinee_ty)),
            };
            
            // Look up the enum in the registry to get the discriminant
            let info = ctx.enum_registry().values()
                .find(|i| i.name == enum_name || i.name.ends_with(&format!("__{}", enum_name)))
                .cloned()
                .ok_or_else(|| format!("Unknown enum '{}' in pattern match", enum_name))?;
            
            // Find the variant and its discriminant
            let (_variant_name, _payload_ty, discriminant) = info.variants.iter()
                .find(|(n, _, _)| n == &variant_name)
                .ok_or_else(|| format!("Unknown variant '{}' in enum '{}'", variant_name, enum_name))?;
            
            // Extract the discriminant from the scrutinee (index 0)
            let struct_ty = scrutinee_ty.to_mlir_type(ctx)?;
            let tag_val = format!("%match_tag_{}", ctx.next_id());
            ctx.emit_extractvalue(out, &tag_val, scrutinee, 0, &struct_ty);
            
            // Compare discriminant with expected value
            let disc_const = format!("%disc_const_{}", ctx.next_id());
            let result = format!("%match_variant_{}", ctx.next_id());
            ctx.emit_const_int(out, &disc_const, *discriminant as i64, "i32");
            out.push_str(&format!("    {} = arith.cmpi eq, {}, {} : i32\n", result, tag_val, disc_const));
            
            Ok(result)
        }
        Pattern::Tuple(sub_patterns) => {
            // Tuple patterns always match (unless sub-patterns fail)
            // For condition, we just check all sub-patterns match
            // Tuple type should be Type::Tuple(fields)
            let field_types = match scrutinee_ty {
                Type::Tuple(tys) => tys.clone(),
                _ => return Err(format!("Cannot match tuple pattern on non-tuple type: {:?}", scrutinee_ty)),
            };
            
            if sub_patterns.len() != field_types.len() {
                return Err(format!(
                    "Tuple pattern has {} elements but type has {} fields",
                    sub_patterns.len(), field_types.len()
                ));
            }
            
            // Start with true (all sub-patterns must match)
            let mut result = {
                let r = format!("%tuple_match_init_{}", ctx.next_id());
                out.push_str(&format!("    {} = arith.constant true\n", r));
                r
            };
            
            let struct_ty = scrutinee_ty.to_mlir_type(ctx)?;
            
            for (i, (sub_pat, field_ty)) in sub_patterns.iter().zip(field_types.iter()).enumerate() {
                // Extract field from tuple
                let field_val = format!("%tuple_field_{}_{}", i, ctx.next_id());
                ctx.emit_extractvalue(out, &field_val, scrutinee, i, &struct_ty);
                
                // Match sub-pattern
                let sub_result = emit_pattern_condition(ctx, out, sub_pat, &field_val, field_ty)?;
                
                // AND with accumulator
                let combined = format!("%tuple_match_and_{}", ctx.next_id());
                out.push_str(&format!("    {} = arith.andi {}, {} : i1\n", combined, result, sub_result));
                result = combined;
            }
            
            Ok(result)
        }
        Pattern::Struct { name, fields } => {
            // Struct patterns: match field patterns against struct fields
            // First verify the scrutinee is a struct type matching 'name'
            let struct_name = match scrutinee_ty {
                Type::Struct(n) => n.clone(),
                Type::Concrete(n, _) => n.clone(),
                _ => return Err(format!("Cannot match struct pattern on non-struct type: {:?}", scrutinee_ty)),
            };
            
            // Check the struct name matches (allowing mangled variants)
            if !struct_name.ends_with(&name.to_string()) && struct_name != name.to_string() {
                return Err(format!(
                    "Struct pattern '{}' doesn't match scrutinee type '{}'",
                    name, struct_name
                ));
            }
            
            // Look up the struct in the registry
            let info = ctx.struct_registry().values()
                .find(|i| i.name == struct_name || i.name.ends_with(&format!("__{}", name)))
                .cloned()
                .ok_or_else(|| format!("Unknown struct '{}' in pattern match", name))?;
            
            // Start with true
            let mut result = {
                let r = format!("%struct_match_init_{}", ctx.next_id());
                out.push_str(&format!("    {} = arith.constant true\n", r));
                r
            };
            
            let struct_mlir_ty = scrutinee_ty.to_mlir_type(ctx)?;
            
            for pat_field in fields {
                // Find the field in the struct info
                let (field_offset, field_ty) = info.fields.get(&pat_field.name.to_string())
                    .ok_or_else(|| format!("Unknown field '{}' in struct '{}'", pat_field.name, name))?
                    .clone();
                
                // Extract the field value
                let field_val = format!("%struct_field_{}_{}", pat_field.name, ctx.next_id());
                ctx.emit_extractvalue(out, &field_val, scrutinee, field_offset, &struct_mlir_ty);
                
                // Match the sub-pattern (if any)
                let sub_pat = pat_field.pattern.as_ref()
                    .cloned()
                    .unwrap_or_else(|| Pattern::Ident { name: pat_field.name.clone(), mutable: false });
                
                let sub_result = emit_pattern_condition(ctx, out, &sub_pat, &field_val, &field_ty)?;
                
                // AND with accumulator
                let combined = format!("%struct_match_and_{}", ctx.next_id());
                out.push_str(&format!("    {} = arith.andi {}, {} : i1\n", combined, result, sub_result));
                result = combined;
            }
            
            Ok(result)
        }
        Pattern::Rest => {
            Err("Rest pattern (..) cannot appear as top-level match pattern".to_string())
        }
    }
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
            // Extract payload if there are fields to bind
            if let Some(field_patterns) = fields {
                if field_patterns.is_empty() {
                    return Ok(());
                }
                
                // Get enum info
                // For specialized generic enums, use fully-mangled name
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
                
                // Find the variant's payload type
                let (_, payload_ty, _) = info.variants.iter()
                    .find(|(n, _, _)| n == &variant_name)
                    .ok_or_else(|| format!("Unknown variant '{}' in enum '{}'", variant_name, enum_name))?;
                
                if let Some(inner_ty) = payload_ty {
                    // Extract payload array from enum (index 1)
                    let struct_ty = scrutinee_ty.to_mlir_type(ctx)?;
                    let payload_array = format!("%payload_array_{}", ctx.next_id());
                    ctx.emit_extractvalue(out, &payload_array, scrutinee, 1, &struct_ty);
                    
                    // Store array to memory and load as the payload type
                    let array_mlir_ty = format!("!llvm.array<{} x i8>", info.max_payload_size);
                    let buf_ptr = format!("%payload_buf_{}", ctx.next_id());
                    ctx.emit_alloca(out, &buf_ptr, &array_mlir_ty);
                    ctx.emit_store(out, &payload_array, &buf_ptr, &array_mlir_ty);
                    
                    let payload_val = format!("%payload_val_{}", ctx.next_id());
                    let inner_mlir_ty = inner_ty.to_mlir_type(ctx)?;
                    ctx.emit_load(out, &payload_val, &buf_ptr, &inner_mlir_ty);
                    
                    // If there's a single field pattern, bind it
                    if field_patterns.len() == 1 {
                        emit_pattern_bindings(ctx, out, &field_patterns[0], &payload_val, inner_ty, local_vars)?;
                    } else if let Type::Tuple(field_tys) = inner_ty {
                        // Multiple fields - payload is a tuple
                        let tuple_mlir_ty = inner_ty.to_mlir_type(ctx)?;
                        for (i, (field_pat, field_ty)) in field_patterns.iter().zip(field_tys.iter()).enumerate() {
                            let field_val = format!("%variant_field_{}_{}", i, ctx.next_id());
                            ctx.emit_extractvalue(out, &field_val, &payload_val, i, &tuple_mlir_ty);
                            emit_pattern_bindings(ctx, out, field_pat, &field_val, field_ty, local_vars)?;
                        }
                    }
                }
            }
            Ok(())
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
    // [V1.1] RAII-Lite: Emit cleanup for all owned resources in the cleanup_stack
    // This handles Vec and other container types registered via register_owned_resource
    {
        let tasks: Vec<_> = ctx.cleanup_stack()
            .last()
            .map(|t| t.iter().rev().cloned().collect::<Vec<_>>())
            .unwrap_or_default();

        for task in &tasks {

                // [V1.1] Z3 Ownership Ledger: Register DEATH event for each resource (DISABLED)
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

    // [QoL V1.0] Drop Trait RAII: Auto-call drop() on locals implementing Drop
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
                    
                    // [QoL V1.0] Demand-driven hydration: ensure drop() is emitted
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
                        ctx.pending_generations_mut().push_back(task);
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
        if matches!(ty, Type::Owned(_)) {
            if !ctx.consumed_vars().contains(name) {
                 if let LocalKind::Ptr(_ptr) = kind {
                     // TODO: Emit proper cleanup for Owned types
                     // For now, assume external management (e.g., region allocator)
                 }
            }
        }
    }
    Ok(())
}

struct MutationVisitor {
    mutated: HashSet<String>,
}
impl<'ast> Visit<'ast> for MutationVisitor {
    fn visit_expr(&mut self, i: &'ast syn::Expr) {
        if let syn::Expr::Assign(assign) = i {
            let mut curr = &*assign.left;
            while let syn::Expr::Field(f) = curr { curr = &*f.base; }
            while let syn::Expr::Index(idx) = curr { curr = &*idx.expr; }
            if let syn::Expr::Path(p) = curr {
                if let Some(id) = p.path.get_ident() { self.mutated.insert(id.to_string()); }
            }
        }
        visit::visit_expr(self, i);
    }
}

pub fn collect_mutations(stmts: &[Stmt]) -> HashSet<String> {
    let mut visitor = MutationVisitor { mutated: HashSet::new() };
    for stmt in stmts { collect_mutations_in_stmt(&mut visitor, stmt); }
    visitor.mutated
}

fn collect_mutations_in_stmt(visitor: &mut MutationVisitor, stmt: &Stmt) {
    match stmt {
        Stmt::Syn(s) => visitor.visit_stmt(s),
        Stmt::While(w) => {
            visitor.visit_expr(&w.cond);
            for s in &w.body.stmts { collect_mutations_in_stmt(visitor, s); }
        }
        Stmt::If(f) => {
            visitor.visit_expr(&f.cond);
            for s in &f.then_branch.stmts { collect_mutations_in_stmt(visitor, s); }
            if let Some(eb) = &f.else_branch {
                match eb.as_ref() {
                    SaltElse::Block(b) => { for s in &b.stmts { collect_mutations_in_stmt(visitor, s); } }
                    SaltElse::If(nested) => { collect_mutations_in_stmt(visitor, &Stmt::If(nested.as_ref().clone())); }
                }
            }
        }
        _ => {}
    }
}

// ============================================================================
// ARENA ESCAPE ANALYSIS — Helper Functions (Scope Ladder)
// ============================================================================

/// Detect if an expression is an Arena constructor call: `Arena::new(...)`.
/// Matches path calls where the last two segments are "Arena" and "new".
fn is_arena_constructor(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Call(c) => {
            if let syn::Expr::Path(p) = &*c.func {
                let segs: Vec<_> = p.path.segments.iter().map(|s| s.ident.to_string()).collect();
                // Match Arena::new or ...::Arena::new
                if segs.len() >= 2 {
                    return segs[segs.len() - 2] == "Arena" && segs[segs.len() - 1] == "new";
                }
            }
            false
        }
        _ => false,
    }
}

/// Extract the arena receiver name from an `arena.alloc(...)` or `arena.alloc_array(...)` call.
/// Returns Some("arena") if the expression is a method call with method name "alloc" or
/// "alloc_array" and the receiver is a simple identifier.
fn extract_arena_alloc_receiver(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::MethodCall(m) => {
            let method = m.method.to_string();
            if method == "alloc" || method == "alloc_array" {
                // Check if receiver is a simple ident (e.g., `arena`)
                if let syn::Expr::Path(p) = &*m.receiver {
                    if let Some(ident) = p.path.get_ident() {
                        return Some(ident.to_string());
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract the simple variable name from a return expression, traversing
/// casts and parens. For `return n`, returns Some("n"). For `return n as Ptr<T>`,
/// also returns Some("n").
fn extract_return_var_name(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(p) => {
            if let Some(ident) = p.path.get_ident() {
                Some(ident.to_string())
            } else {
                None
            }
        }
        syn::Expr::Cast(c) => extract_return_var_name(&c.expr),
        syn::Expr::Paren(p) => extract_return_var_name(&p.expr),
        _ => None,
    }
}

/// Extract the arena variable name from an `ArenaAllocator { arena: my_arena }` struct literal.
/// Returns Some("my_arena") if the expression is a struct literal whose path ends with
/// "ArenaAllocator" and has a field "arena" whose value is a simple identifier.
fn extract_arena_allocator_source(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Struct(s) => {
            // Check if the struct name ends with "ArenaAllocator"
            let last_seg = s.path.segments.last()?;
            if last_seg.ident != "ArenaAllocator" {
                return None;
            }
            // Find the "arena" field
            for field in &s.fields {
                if let syn::Member::Named(ident) = &field.member {
                    if ident == "arena" {
                        // Extract the value — must be a simple ident
                        if let syn::Expr::Path(p) = &field.expr {
                            if let Some(ident) = p.path.get_ident() {
                                return Some(ident.to_string());
                            }
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract the allocator variable name from a `Vec::new(alloc, cap)` or
/// `Vec::<T, A>::new(alloc, cap)` call. Returns Some("alloc") —
/// the first argument, which is the allocator.
///
/// Matches both forms:
/// - Path call: `Vec::new(alloc, cap)` / `Vec::<i64, HeapAllocator>::new(alloc, cap)`
/// - The last two path segments must be ["Vec" or similar, "new"]
fn extract_vec_new_allocator(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Call(c) => {
            if let syn::Expr::Path(p) = &*c.func {
                let segs: Vec<_> = p.path.segments.iter().map(|s| s.ident.to_string()).collect();
                // Match Vec::new, std::collections::vec::Vec::new, etc.
                if segs.len() >= 2 && segs[segs.len() - 1] == "new" {
                    let type_name = &segs[segs.len() - 2];
                    if type_name == "Vec" {
                        // First argument is the allocator
                        if let Some(first_arg) = c.args.first() {
                            if let syn::Expr::Path(arg_p) = first_arg {
                                if let Some(ident) = arg_p.path.get_ident() {
                                    return Some(ident.to_string());
                                }
                            }
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

