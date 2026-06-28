use crate::grammar::Stmt;
use crate::types::Type;
use crate::codegen::context::LocalKind;
use std::collections::HashMap;

/// Information about a detected reduction pattern
pub(crate) struct ReductionInfo {
    /// Name of the accumulator variable (e.g., "sum" or "acc")
    pub(crate) accumulator_var: String,
    /// Initial value SSA name - for Alloca, this is the pointer; for SSA, this is the value
    pub(crate) init_ssa: String,
    /// Type of the accumulator
    pub(crate) ty: Type,
    /// True if the accumulator is an alloca (mut variable), requiring load/store wrapper
    pub(crate) is_alloca: bool,
    /// Kind of reduction: Simple (acc + expr), FMA (vector_fma(a, b, acc))
    pub(crate) kind: ReductionKind,
    /// Statement index where the reduction update occurs (for multi-statement bodies)
    pub(crate) update_stmt_idx: usize,
}

/// Kind of reduction operation
#[derive(Clone, Debug)]
pub(crate) enum ReductionKind {
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
pub(crate) fn detect_reduction_pattern(
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
pub(crate) fn detect_vector_reduction_pattern(
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


