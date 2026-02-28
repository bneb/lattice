use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use super::utils::*;
use crate::codegen::type_bridge::*;
use std::collections::HashMap;
use super::{emit_expr, unify_types};

pub fn emit_if_expr(ctx: &mut LoweringContext, out: &mut String, if_expr: &syn::ExprIf, local_vars: &mut HashMap<String, (Type, LocalKind)>, expected: Option<&Type>) -> Result<(String, Type), String> {
    // FFB OPTIMIZATION: In affine context, emit arith.select for simple scalar if expressions
    // This avoids cf.cond_br which breaks affine.for's single-block requirement
    if ctx.is_in_affine_context() {
        if let Some((_, else_branch)) = &if_expr.else_branch {
            // Check if both branches are simple scalar expressions
            if is_simple_scalar_branch(&if_expr.then_branch) && is_simple_scalar_else(else_branch) {
                return emit_if_as_select(ctx, out, if_expr, local_vars, expected);
            }
        }
    }
    
    // 1. Emit Condition
    let cond_res = emit_expr(ctx, out, &if_expr.cond, local_vars, Some(&Type::Bool))?;
    let (cond_val, cond_ty) = cond_res;
    if cond_ty != Type::Bool {
        return Err(format!("If condition must be bool, found {:?}", cond_ty));
    }

    let merge_block = format!("merge_{}", ctx.next_id());
    let then_block = format!("then_{}", ctx.next_id());
    let else_block = format!("else_{}", ctx.next_id());

    // Sovereign Heuristic Removed. We must unify.
    // To do this, we need to know types of branches.
    // We can't emit alloc for result yet because we don't know the type.
    // We will assume that if we are in a block that returns, we allocate result ptr?
    // Actually, MLIR supports block arguments. But Salt uses allocas.
    // Since we can't know the type beforehand without analyzing the branches (which requires emitting them or a separate pass),
    // and we want to emit into `out` linearly...
    // We will use temporary buffers for branches to determine type.
    
    let mut then_out = String::new();
    // [v0.9.2] Push branch condition as path constraint for Z3 postcondition verification
    ctx.emission.path_conditions.push((*if_expr.cond).clone());
    let (then_val, then_actual) = emit_block_expr(ctx, &mut then_out, &if_expr.then_branch, local_vars, expected)?;
    ctx.emission.path_conditions.pop();
    
    let mut else_out = String::new();
    let (else_val, else_actual) = if let Some((_, else_branch)) = &if_expr.else_branch {
        // [v0.9.2] Push negated condition for else branch
        let negated_cond = syn::Expr::Unary(syn::ExprUnary {
            attrs: vec![],
            op: syn::UnOp::Not(syn::token::Not::default()),
            expr: Box::new((*if_expr.cond).clone()),
        });
        ctx.emission.path_conditions.push(negated_cond);
        let result = match else_branch.as_ref() {
             syn::Expr::Block(b) => emit_block_expr(ctx, &mut else_out, &b.block, local_vars, expected)?,
             syn::Expr::If(i) => emit_if_expr(ctx, &mut else_out, i, local_vars, expected)?,
             _ => return Err("Unsupported else branch".to_string())
        };
        ctx.emission.path_conditions.pop();
        result
    } else {
        // [v0.9.2] No else-branch: if the then-branch always terminates (returns),
        // then any subsequent code only runs when condition is FALSE.
        // Push the negated condition as a permanent path constraint.
        if then_actual == Type::Never {
            let negated_cond = syn::Expr::Unary(syn::ExprUnary {
                attrs: vec![],
                op: syn::UnOp::Not(syn::token::Not::default()),
                expr: Box::new((*if_expr.cond).clone()),
            });
            ctx.emission.path_conditions.push(negated_cond);
        }
        (String::new(), Type::Unit)
    };

    let result_ty = if if_expr.else_branch.is_none() {
        Type::Unit
    } else {
        unify_types(&then_actual, &else_actual)?
    };

    // Now emit the conditional branch in main `out`
    out.push_str(&format!("    cf.cond_br {}, ^{}, ^{}\n", cond_val, then_block, else_block));
    
    // Result pointer
    let result_ptr = if result_ty != Type::Unit && result_ty != Type::Never {
         let ptr_name = format!("%if_res_ptr_{}", ctx.next_id());
         let mlir_ty = result_ty.to_mlir_type(ctx)?;
         ctx.emit_alloca(out, &ptr_name, &mlir_ty);
         ptr_name
    } else {
        String::new()
    };
    
    // Then Block
    out.push_str(&format!("  ^{}:\n", then_block));
    out.push_str(&then_out);
    if result_ty != Type::Unit && result_ty != Type::Never && then_actual != Type::Never {
        let mlir_ty = then_actual.to_mlir_type(ctx)?;
        out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", then_val, result_ptr, mlir_ty));
    }
    out.push_str(&format!("    cf.br ^{}\n", merge_block));
    
    // Else Block
    out.push_str(&format!("  ^{}:\n", else_block));
    out.push_str(&else_out);
    if result_ty != Type::Unit && result_ty != Type::Never && else_actual != Type::Never {
        let mlir_ty = else_actual.to_mlir_type(ctx)?;
        out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", else_val, result_ptr, mlir_ty));
    }
    out.push_str(&format!("    cf.br ^{}\n", merge_block));
    
    // Merge
    out.push_str(&format!("  ^{}:\n", merge_block));
    if result_ty != Type::Unit && result_ty != Type::Never {
        let res_val = format!("%if_res_{}", ctx.next_id());
        let mlir_ty = result_ty.to_mlir_type(ctx)?;
        out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", res_val, result_ptr, mlir_ty));
        Ok((res_val, result_ty))
    } else {
        Ok(("".to_string(), Type::Unit))
    }
}

/// Check if a block is a simple scalar expression (for select optimization)
fn is_simple_scalar_branch(block: &syn::Block) -> bool {
    if block.stmts.len() != 1 {
        return false;
    }
    match &block.stmts[0] {
        syn::Stmt::Expr(e, _) => is_simple_scalar_for_select(e),
        _ => false,
    }
}

/// Check if an else branch is simple scalar
fn is_simple_scalar_else(else_branch: &syn::Expr) -> bool {
    match else_branch {
        syn::Expr::Block(b) => is_simple_scalar_branch(&b.block),
        syn::Expr::If(_) => false,  // Chained if-else is more complex
        _ => is_simple_scalar_for_select(else_branch),
    }
}

/// Check if an expression is a simple scalar value suitable for arith.select
fn is_simple_scalar_for_select(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Lit(lit) => matches!(lit.lit, syn::Lit::Int(_) | syn::Lit::Float(_)),
        syn::Expr::Path(_) => true,  // Variable reference - could be any type
        syn::Expr::Unary(u) => is_simple_scalar_for_select(&u.expr),
        syn::Expr::Paren(p) => is_simple_scalar_for_select(&p.expr),
        syn::Expr::Cast(c) => is_simple_scalar_for_select(&c.expr),
        _ => false,
    }
}

/// Emit an if expression as arith.select (for affine context optimization)
pub(crate) fn emit_if_as_select(ctx: &mut LoweringContext, out: &mut String, if_expr: &syn::ExprIf, local_vars: &mut HashMap<String, (Type, LocalKind)>, expected: Option<&Type>) -> Result<(String, Type), String> {
    // 1. Emit Condition
    let (cond_val, cond_ty) = emit_expr(ctx, out, &if_expr.cond, local_vars, Some(&Type::Bool))?;
    if cond_ty != Type::Bool {
        return Err(format!("If condition must be bool, found {:?}", cond_ty));
    }
    
    // 2. Emit then branch expression
    let (then_val, then_ty) = emit_block_expr(ctx, out, &if_expr.then_branch, local_vars, expected)?;
    
    // 3. Emit else branch expression
    let (else_val, else_ty) = if let Some((_, else_branch)) = &if_expr.else_branch {
        match else_branch.as_ref() {
            syn::Expr::Block(b) => emit_block_expr(ctx, out, &b.block, local_vars, expected)?,
            _ => emit_expr(ctx, out, else_branch, local_vars, expected)?,
        }
    } else {
        return Err("arith.select requires else branch".to_string());
    };
    
    // 4. Unify types
    let result_ty = unify_types(&then_ty, &else_ty)?;
    let mlir_ty = result_ty.to_mlir_type(ctx)?;
    
    // 5. Emit arith.select
    let result = format!("%select_{}", ctx.next_id());
    out.push_str(&format!("    {} = arith.select {}, {}, {} : {}\n", result, cond_val, then_val, else_val, mlir_ty));
    
    Ok((result, result_ty))
}

pub fn emit_block_expr(ctx: &mut LoweringContext, out: &mut String, b: &syn::Block, local_vars: &mut HashMap<String, (Type, LocalKind)>, expected_ty: Option<&Type>) -> Result<(String, Type), String> {
    let mut block_vars = local_vars.clone();
    let mut last_res = ("%unit".to_string(), Type::Unit);
    
    for (i, stmt) in b.stmts.iter().enumerate() {
        // Check if this is the final expression which determines the block's value
        if i == b.stmts.len() - 1 {
            if let syn::Stmt::Expr(e, None) = stmt {
                let (mut val, mut ty) = emit_expr(ctx, out, e, &mut block_vars, expected_ty)?;
                if let Some(target) = expected_ty {
                     val = crate::codegen::type_bridge::promote_numeric(ctx, out, &val, &ty, target)?;
                     ty = target.clone();
                }
                last_res = (val, ty);
                continue;
            }
        }
        
        // Otherwise emit as a statement (declarations, expressions with semi, etc.)
        let grammar_stmt = crate::grammar::Stmt::Syn(stmt.clone());
        if crate::codegen::stmt::emit_stmt(ctx, out, &grammar_stmt, &mut block_vars)? {
             return Ok(("%unreachable".to_string(), Type::Never));
        }
    }
    Ok(last_res)
}

pub fn emit_match(ctx: &mut LoweringContext, out: &mut String, m: &syn::ExprMatch, local_vars: &mut HashMap<String, (Type, LocalKind)>, expected_ty: Option<&Type>) -> Result<(String, Type), String> {
    let (scrutinee_val, scrutinee_ty) = emit_expr(ctx, out, &m.expr, local_vars, None)?;
    
    let enum_mangled_name = scrutinee_ty.mangle_suffix();
    
    let info = ctx.enum_registry().values().find(|i| i.name == enum_mangled_name).cloned().ok_or(format!("Unknown enum {}", enum_mangled_name))?;
    
    // Result handling
    let ret_ty = if let Some(ty) = expected_ty { ty.clone() } else { Type::U64 }; // Default
    let res_ptr = format!("%match_res_ptr_{}", ctx.next_id());
    let mlir_ty = ret_ty.to_mlir_type(ctx)?;
    if ret_ty != Type::Unit {
        ctx.emit_alloca(out, &res_ptr, &mlir_ty);
    }

    let merge_block = format!("match_merge_{}", ctx.next_id());
    
    // We track target labels for each arm to emit blocks in order
    let mut arm_targets = Vec::new();
    // We track explicit switch cases
    let mut switch_cases_map = Vec::new();
    // Default label (if Wild pattern exists)
    let mut explicit_default = None;
    
    let tag_val = format!("%tag_{}", ctx.next_id());
    let struct_ty = scrutinee_ty.to_mlir_type(ctx)?; // !llvm.struct<"EnumName", (i32, [u8...])>
    // Extract Tag (index 0)
    ctx.emit_extractvalue(out, &tag_val, &scrutinee_val, 0, &struct_ty);

    for (i, arm) in m.arms.iter().enumerate() {
        let block_label = format!("case_{}_{}", i, ctx.next_id());
        arm_targets.push(block_label.clone());
        
        // Resolve Pattern to Discriminant
        match &arm.pat {
            syn::Pat::Wild(_) => {
                // Catch-all
                explicit_default = Some(block_label.clone());
            },
            syn::Pat::Path(p) => {
                 // Enum::Variant
                 let last = p.path.segments.last().unwrap().ident.to_string();
                 if let Some((_, _, idx)) = info.variants.iter().find(|(n, _, _)| n == &last) {
                     switch_cases_map.push((*idx, block_label.clone()));
                 } else {
                     return Err(format!("Unknown variant {}", last));
                 }
            },
            syn::Pat::TupleStruct(ts) => {
                 // Enum::Variant(v)
                 let last = ts.path.segments.last().unwrap().ident.to_string();
                 if let Some((_, _ty_opt, idx)) = info.variants.iter().find(|(n, _, _)| n == &last) {
                     // Bindings are handled during block emission
                     switch_cases_map.push((*idx, block_label.clone()));
                 } else {
                     return Err(format!("Unknown variant {}", last));
                 }
            },
            syn::Pat::Ident(_) => {
                return Err("Catch-all Ident patterns not fully supported in MVP match selection (use _ or specific variant)".to_string());
            }
            _ => return Err(format!("Unsupported match pattern: {:?}", arm.pat)),
        };
    }
    
    // Emit Switch
    let fallback_label = format!("default_fallback_{}", ctx.next_id());
    let default_label = explicit_default.clone().unwrap_or_else(|| fallback_label.clone());
    
    let mut switch_args = Vec::new();
    for (val, label) in &switch_cases_map {
        switch_args.push(format!("{} : ^{}", val, label));
    }
    
    out.push_str(&format!("    llvm.switch {} : i32, ^{} [{}]\n", tag_val, default_label, switch_args.join(", ")));
    
    // Emit Cases
    for (i, arm) in m.arms.iter().enumerate() {
        let label = &arm_targets[i];
        out.push_str(&format!("  ^{}:\n", label));
        
        // Sub-scope for bindings
        let mut arm_scope = local_vars.clone();
        
        // Handle bindings if TupleStruct
        if let syn::Pat::TupleStruct(ts) = &arm.pat {
             let last = ts.path.segments.last().unwrap().ident.to_string();
              if let Some((_, Some(inner_ty), _)) = info.variants.iter().find(|(n, _, _)| n == &last) {
                  if let Some(syn::Pat::Ident(id)) = ts.elems.first() {
                        let payload_array = format!("%payload_raw_{}_{}", i, ctx.next_id());
                        ctx.emit_extractvalue(out, &payload_array, &scrutinee_val, 1, &struct_ty);
                        
                        let buffer_ptr = format!("%payload_buf_{}_{}", i, ctx.next_id());
                        let array_mlir_ty = format!("!llvm.array<{} x i8>", info.max_payload_size);
                        ctx.emit_alloca(out, &buffer_ptr, &array_mlir_ty);
                        ctx.emit_store(out, &payload_array, &buffer_ptr, &array_mlir_ty);
                        
                        let loaded_val = format!("%bound_{}_{}", id.ident, ctx.next_id());
                        let i_ty: Type = inner_ty.clone();
                        // Special handling for generics or promotion if needed?
                        // For now strict type.
                        
                        let inner_mlir = i_ty.to_mlir_type(ctx)?;
                        ctx.emit_load(out, &loaded_val, &buffer_ptr, &inner_mlir);
                        
                        arm_scope.insert(id.ident.to_string(), (inner_ty.clone(), LocalKind::SSA(loaded_val)));
                  }
              }
        }
        
        let (val, ty) = emit_expr(ctx, out, &arm.body, &mut arm_scope, Some(&ret_ty))?;
        
        // Handle Return vs Expression result
        if val != "%unreachable" {
             if ret_ty != Type::Unit {
                 let val_prom = crate::codegen::type_bridge::promote_numeric(ctx, out, &val, &ty, &ret_ty)?;
                 ctx.emit_store(out, &val_prom, &res_ptr, &mlir_ty);
             }
             out.push_str(&format!("    cf.br ^{}\n", merge_block));
        }
    }
    
    // Emit Fallback unreachable if no default was provided
    if explicit_default.is_none() {
        out.push_str(&format!("  ^{}:\n", fallback_label));
        out.push_str("    llvm.unreachable\n");
    }
    
    out.push_str(&format!("  ^{}:\n", merge_block));
    
    let res_val = if ret_ty != Type::Unit {
        let v = format!("%match_res_{}", ctx.next_id());
        ctx.emit_load(out, &v, &res_ptr, &mlir_ty);
        v
    } else {
        "%unit".to_string()
    };
    Ok((res_val, ret_ty))
}
