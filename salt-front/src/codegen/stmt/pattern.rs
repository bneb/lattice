use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use crate::grammar::pattern::{Pattern, PatternField};
use std::collections::HashMap;

/// Emit condition for a variant pattern match (discriminant comparison).
pub(crate) fn emit_variant_pattern_condition(
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
pub(crate) fn emit_tuple_pattern_condition(
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
pub(crate) fn emit_struct_pattern_condition(
    ctx: &mut LoweringContext,
    out: &mut String,
    name: &syn::Ident,
    fields: &[PatternField],
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
pub(crate) fn emit_struct_field_condition(
    ctx: &mut LoweringContext,
    out: &mut String,
    pat_field: &PatternField,
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
pub(crate) fn emit_pattern_condition(
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
pub(crate) fn emit_variant_pattern_bindings(
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
pub(crate) fn emit_variant_payload_bindings(
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
pub(crate) fn emit_pattern_bindings(
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
