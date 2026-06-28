use crate::grammar::pattern::Pattern;
use crate::grammar::SaltMatch;
use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use crate::codegen::expr::emit_expr;
use std::collections::HashMap;
use syn::spanned::Spanned;
use super::emit_block;
use super::pattern::{emit_pattern_bindings, emit_pattern_condition};

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
