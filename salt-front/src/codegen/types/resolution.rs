use crate::types::{Type, TypeKey};
use crate::codegen::context::LoweringContext;
use crate::codegen::types::layout::flatten_nested_ptr;
use std::collections::HashMap;

/// Deterministically pick the canonical template key for a bare `name` among
/// keys ending in `__name`. When several match — e.g. a stdlib type also
/// mangled under the entry package (`main__Slice` vs `std__core__slice__Slice`)
/// — prefer the one whose package path is a real loaded module; otherwise take
/// the lexicographically-first, so resolution never depends on HashMap order.
pub(crate) fn pick_canonical_key<'a>(
    keys: impl Iterator<Item = &'a String>,
    name: &str,
    registry: Option<&crate::registry::Registry>,
) -> Option<String> {
    let suffix = format!("__{}", name);
    let mut matches: Vec<&String> = keys.filter(|k| k.ends_with(&suffix)).collect();
    matches.sort();
    if matches.len() > 1 {
        if let Some(reg) = registry {
            let owned = matches.iter().find(|k| {
                reg.modules.contains_key(&k[..k.len() - suffix.len()].replace("__", "."))
            });
            if let Some(hit) = owned.copied() {
                return Some(hit.clone());
            }
        }
    }
    matches.first().map(|k| (*k).clone())
}
fn collect_self_concrete_args(ctx: &mut LoweringContext, struct_name: &str) -> Option<Vec<Type>> {
    let template = ctx.struct_templates().get(struct_name)?;
    let generics = template.generics.as_ref()?;
    let mut args = Vec::with_capacity(generics.params.len());
    for param in &generics.params {
        let p_name = match param {
            crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
            crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
        };
        let arg = ctx.current_type_map().get(&p_name).cloned()?;
        args.push(arg);
    }
    if args.is_empty() { None } else { Some(args) }
}

fn resolve_struct_self_opt(ctx: &mut LoweringContext, r: Type) -> Type {
    if let Type::Struct(name) = &r {
        if let Some(args) = collect_self_concrete_args(ctx, name) {
            return Type::Concrete(name.clone(), args);
        }
    }
    r
}

fn resolve_codegen_type_self(ctx: &mut LoweringContext, _flattened: &Type) -> Type {
    let mut res = None;
    if let Some(concrete_ty) = ctx.current_type_map().get("Self").cloned() {
        res = Some(concrete_ty);
    }
    if res.is_none() {
        if let Some(self_ty) = ctx.current_self_ty() {
            res = Some(self_ty.clone());
        }
    }

    if let Some(r) = res {
        resolve_struct_self_opt(ctx, r)
    } else {
        Type::Unit
    }
}

fn resolve_codegen_type_struct(ctx: &mut LoweringContext, ty: &Type, name: &str) -> Type {
    if name.chars().all(|c| c.is_ascii_digit()) {
        return ty.clone();
    }
    if name.contains("__") {
        let resolved_base = name.to_string();
        let requires_generics = ctx.struct_templates().get(&resolved_base)
            .map(|t| t.generics.as_ref().map(|g| !g.params.is_empty()).unwrap_or(false))
            .unwrap_or(false);
        if requires_generics {
            return Type::Struct(resolved_base);
        }
        let resolved_params = vec![]; 
        let is_enum = ctx.enum_templates().contains_key(&resolved_base);
        if !ctx.suppress_specialization.get() {
            let _ = ctx.specialize_template(&resolved_base, &resolved_params, is_enum);
        }
        if is_enum {
            return Type::Enum(resolved_base);
        } else {
            return Type::Struct(resolved_base);
        }
    }

    let concrete_opt = ctx.current_type_map().get(name).cloned();
    if let Some(concrete_ty) = concrete_opt {
        concrete_ty
    } else {
        let reg = ctx.config.registry;
        let canonical_candidate = pick_canonical_key(ctx.struct_templates().keys(), name, reg)
            .or_else(|| pick_canonical_key(ctx.enum_templates().keys(), name, reg));
        
        if let Some(ref candidate) = canonical_candidate {
            let resolved_base = candidate.clone();
            let requires_generics = ctx.struct_templates().get(&resolved_base)
                .map(|t| t.generics.as_ref().map(|g| !g.params.is_empty()).unwrap_or(false))
                .unwrap_or(false);
            if requires_generics {
                return Type::Struct(resolved_base);
            }
            let is_enum = ctx.enum_templates().contains_key(&resolved_base);
            if !ctx.suppress_specialization.get() {
                let _ = ctx.specialize_template(&resolved_base, &[], is_enum);
            }
            return if is_enum { Type::Enum(resolved_base) } else { Type::Struct(resolved_base) };
        }
        
        let segments: Vec<String> = name.split("::").map(|s| s.to_string()).collect();
        if let Some((pkg, item)) = crate::codegen::expr::utils::resolve_package_prefix_ctx(ctx, &segments) {
             let resolved_base = if item.is_empty() { pkg } else if pkg.is_empty() { item } else { format!("{}__{}", pkg, item) };
             let mut resolved_params = vec![];

             if resolved_params.is_empty() {
                  if let Some(template) = ctx.struct_templates().get(&resolved_base) {
                      if let Some(generics) = &template.generics {
                          let current_args = ctx.current_generic_args();
                           if current_args.len() == generics.params.len() {
                               resolved_params = current_args.clone();
                           } else {
                               let mut inferred = Vec::new();
                               let mut all_found = true;
                               for param in &generics.params {
                                   let p_name = match param {
                                       crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                                       crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                                   };
                                   let arg_opt = ctx.current_type_map().get(&p_name).cloned();
                                   if let Some(arg) = arg_opt {
                                       inferred.push(arg);
                                   } else {
                                       all_found = false;
                                       break;
                                   }
                               }
                               if all_found {
                                   resolved_params = inferred;
                               }
                           }
                      }
                  }
             }
             
             let is_enum = ctx.enum_templates().contains_key(&resolved_base);
             let requires_generics = ctx.struct_templates().get(&resolved_base)
                 .map(|t| t.generics.as_ref().map(|g| !g.params.is_empty()).unwrap_or(false))
                 .unwrap_or(false);
             
             if !ctx.suppress_specialization.get() && (!requires_generics || !resolved_params.is_empty()) {
                  let _ = ctx.specialize_template(&resolved_base, &resolved_params, is_enum);
             }
             
             if !resolved_params.is_empty() {
                 Type::Concrete(resolved_base, resolved_params)
             } else if is_enum {
                 Type::Enum(resolved_base)
             } else {
                 Type::Struct(resolved_base)
             }
        } else {
             Type::Struct(name.to_string())
        }
    }
}

fn resolve_codegen_type_concrete(ctx: &mut LoweringContext, base_name: &str, target_params: &[Type]) -> Type {
    if target_params.is_empty() {
        let concrete_opt = ctx.current_type_map().get(base_name).cloned();
        if let Some(concrete_ty) = concrete_opt {
            return resolve_codegen_type(ctx, &concrete_ty);
        }
    }
    if target_params.is_empty() && !ctx.current_type_map().is_empty() {
        if let Some(template) = ctx.struct_templates().get(base_name) {
            if let Some(generics) = &template.generics {
                let param_names: Vec<String> = generics.params.iter().map(|param| {
                    match param {
                        crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                        crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                    }
                }).collect();
                let type_map = ctx.current_type_map();
                let mut inferred_map = type_map.clone();
                crate::codegen::expr::infer_phantom_generics(&param_names, &mut inferred_map);
                let args: Vec<Type> = param_names.iter()
                    .filter_map(|pname| inferred_map.get(pname).cloned())
                    .collect();
                if args.len() == param_names.len() {
                    let resolved_args: Vec<Type> = args.iter()
                        .map(|a| resolve_codegen_type(ctx, a))
                        .collect();
                    return Type::Concrete(base_name.to_string(), resolved_args);
                }
            }
        }
    }
    let mut resolved_params = Vec::new();
    for param in target_params {
        resolved_params.push(resolve_codegen_type(ctx, param));
    }
    if base_name == "Owned" && !resolved_params.is_empty() {
        Type::Owned(Box::new(resolved_params[0].clone()))
    } else if !resolved_params.is_empty() && base_name == "Window" {
        let region = if resolved_params.len() >= 2 {
            if let Type::Struct(r) = &resolved_params[1] { r.clone() } else { "RAM".to_string() }
        } else { "RAM".to_string() };
        Type::Window(Box::new(resolved_params[0].clone()), region)
    } else if base_name == "Atomic" && !resolved_params.is_empty() {
        Type::Atomic(Box::new(resolved_params[0].clone()))
    } else {
        let mut resolved_base = base_name.to_string();
        if !resolved_base.contains("__") {
            let reg = ctx.config.registry;
            let canonical_candidate = pick_canonical_key(ctx.struct_templates().keys(), base_name, reg)
                .or_else(|| pick_canonical_key(ctx.enum_templates().keys(), base_name, reg));
            
            if let Some(candidate) = canonical_candidate {
                resolved_base = candidate;
            } else {
                let segments: Vec<String> = base_name.split("::").map(|s| s.to_string()).collect();
                if let Some((pkg, item)) = crate::codegen::expr::utils::resolve_package_prefix_ctx(ctx, &segments) {
                     resolved_base = if item.is_empty() { pkg } else if pkg.is_empty() { item } else { format!("{}__{}", pkg, item) };
                }
            }
        }

        let is_enum = ctx.enum_templates().contains_key(&resolved_base);
        if (ctx.struct_templates().contains_key(&resolved_base) || is_enum)
            && !ctx.suppress_specialization.get() {
                let _ = ctx.specialize_template(&resolved_base, &resolved_params, is_enum);
            }
        Type::Concrete(resolved_base, resolved_params)
    }
}

pub fn resolve_codegen_type(ctx: &mut LoweringContext, ty: &Type) -> Type {
    let flattened = flatten_nested_ptr(ty, 0, "codegen_resolve");
    match &flattened {
        Type::Enum(name) => Type::Enum(name.clone()),
        Type::Generic(name) => {
            let concrete_opt = ctx.current_type_map().get(name).cloned();
            if let Some(concrete_ty) = concrete_opt {
                 if let Type::Generic(ref n) = concrete_ty {
                     if n == name {
                         return concrete_ty;
                     }
                 }
                 resolve_codegen_type(ctx, &concrete_ty)
            } else if ctx.enum_registry().values().any(|i| i.name == *name) || ctx.enum_templates().contains_key(name) {
                Type::Enum(name.clone())
            } else {
                Type::Struct(name.clone())
            }
        }
        Type::SelfType => resolve_codegen_type_self(ctx, &flattened),
        Type::Struct(name) => resolve_codegen_type_struct(ctx, ty, name),
        Type::Concrete(base_name, target_params) => resolve_codegen_type_concrete(ctx, base_name, target_params),
        Type::Pointer { element, provenance, is_mutable } => Type::Pointer {
            element: Box::new(resolve_codegen_type(ctx, element)),
            provenance: provenance.clone(),
            is_mutable: *is_mutable,
        },
        Type::Reference(inner, mutability) => Type::Reference(Box::new(resolve_codegen_type(ctx, inner)), *mutability),
        Type::Owned(inner) => Type::Owned(Box::new(resolve_codegen_type(ctx, inner))),
        Type::Fn(args, ret) => Type::Fn(
            args.iter().map(|a| resolve_codegen_type(ctx, a)).collect(),
            Box::new(resolve_codegen_type(ctx, ret)),
        ),
        Type::Window(inner, region) => Type::Window(Box::new(resolve_codegen_type(ctx, inner)), region.clone()),
        Type::Array(inner, len, _) => Type::Array(Box::new(resolve_codegen_type(ctx, inner)), *len, false),
        Type::Tuple(elems) => Type::Tuple(elems.iter().map(|e| resolve_codegen_type(ctx, e)).collect()),
        Type::Tensor(inner, shape) => Type::Tensor(Box::new(resolve_codegen_type(ctx, inner)), shape.clone()),
        _ => ty.clone(),
    }
}



/// Bridges the gap between Rust's syn::Type (legacy/helper) and Salt's Type system.
pub fn resolve_type(ctx: &mut LoweringContext, ty: &crate::grammar::SynType) -> Type {
    // Handle context-dependent types (Array, Tensor) here.

    if let crate::grammar::SynType::Array(inner, len_expr) = ty {
        let inner_ty = resolve_type(ctx, inner);
        return match ctx.evaluator.eval_expr(len_expr) {
            Ok(crate::evaluator::ConstValue::Integer(val)) => Type::Array(Box::new(inner_ty), val as usize, false),
            Ok(_) => { crate::ice!("Array length must evaluate to an integer"); },
            Err(e) => { crate::ice!("Failed to evaluate array length: {:?}", e); }
        };
    }

    if let crate::grammar::SynType::Path(tp) = ty {
        if let Some(seg) = tp.segments.last() {
            if seg.ident == "Tensor"
                 && seg.args.len() >= 2 {
                     let inner_syn = &seg.args[0];
                     let inner = resolve_type(ctx, inner_syn);
                     let mut shape = Vec::new();
                     
                     // Check for __Shape_X_Y_Z__ marker (AUTO-RANK)
                     // Preprocessor prepends auto-computed rank: {128,784} -> __Shape_2_128_784__
                     // Format: __Shape_Rank_D1_D2_...__ where first element is auto-rank (skipped)
                     if let crate::grammar::SynType::Path(shape_path) = &seg.args[1] {
                         if let Some(shape_seg) = shape_path.segments.last() {
                             let shape_name = shape_seg.ident.to_string();
                             if shape_name.starts_with("__Shape_") && shape_name.ends_with("__") {
                                 // Parse __Shape_2_128_784__ -> skip auto-rank, dims = [128, 784]
                                 let shape_str = &shape_name[8..shape_name.len()-2]; // strip prefix/suffix
                                 let all_values: Vec<usize> = shape_str.split('_')
                                     .filter_map(|s| s.parse().ok())
                                     .collect();
                                 // Skip first value (rank indicator) and use rest as dimensions
                                 if all_values.len() > 1 {
                                     shape = all_values[1..].to_vec();
                                 } else if !all_values.is_empty() {
                                     // Single value: use as dimension (rank-1 tensor)
                                     shape = all_values;
                                 }
                                 return Type::Tensor(Box::new(inner), shape);
                             }
                         }
                     }
                     
                     // Legacy: Support old Tensor<f32, [128], [784]> syntax
                     for i in 1..seg.args.len() {
                         if let crate::grammar::SynType::Array(_dummy, len_expr) = &seg.args[i] {
                              if let Ok(crate::evaluator::ConstValue::Integer(val)) = ctx.evaluator.eval_expr(len_expr) {
                                  shape.push(val as usize);
                              }
                         }
                     }
                     return Type::Tensor(Box::new(inner), shape);
                 }
        }
    }

    // Default: Lower to Type and resolve imports/aliases (via resolve_codegen_type)
    // Note: Type::from_syn handles basic conversions (structs, primitives, etc.)
    if let Some(t) = Type::from_syn(ty) {
        resolve_codegen_type(ctx, &t)
    } else {
        Type::Unit
    }
}

/// Infers the type of a syn::Expr without emitting MLIR.
/// Used for receiver extraction in method call resolution.
pub fn infer_expr_type(
    ctx: &mut LoweringContext, 
    expr: &syn::Expr, 
    local_vars: &HashMap<String, (Type, crate::codegen::context::LocalKind)>
) -> Result<Type, String> {
    match expr {
        syn::Expr::Path(p) => {
            let name = p.path.segments.iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("__");
            
            // Check local vars first
            if let Some((ty, _)) = local_vars.get(&name) {
                return Ok(ty.clone());
            }
            
            // Check single-segment name in locals
            if p.path.segments.len() == 1 {
                let simple_name = p.path.segments[0].ident.to_string();
                if let Some((ty, _)) = local_vars.get(&simple_name) {
                    return Ok(ty.clone());
                }
            }
            
            // Check global variables/constants
            if let Some(ty) = ctx.globals().get(&name) {
                return Ok(ty.clone());
            }
            
            // Try canonical resolution with imports
            let canonical = crate::codegen::expr::utils::resolve_package_prefix_ctx(ctx, &p.path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>());
            if let Some((pkg, _)) = canonical {
                if let Some(ty) = ctx.globals().get(&pkg) {
                    return Ok(ty.clone());
                }
            }
            
            Err(format!("Cannot infer type for path expression: {:?}", name))
        }
        syn::Expr::Paren(p) => infer_expr_type(ctx, &p.expr, local_vars),
        syn::Expr::Field(f) => {
            let base_ty = infer_expr_type(ctx, &f.base, local_vars)?;
            // For field access, find the field type in the struct registry
            let _base_name = match &base_ty {
                Type::Struct(n) => n.clone(),
                Type::Concrete(n, _) => n.clone(),
                Type::Reference(inner, _) => {
                    match &**inner {
                        Type::Struct(n) => n.clone(),
                        Type::Concrete(n, _) => n.clone(),
                        _ => return Err(format!("Field access on non-struct reference: {:?}", base_ty)),
                    }
                }
                _ => return Err(format!("Field access on non-struct type: {:?}", base_ty)),
            };
            
            // Find the struct in the registry using TypeKey
            let type_key = type_to_type_key(&base_ty);
            if let Some(info) = ctx.struct_registry().get(&type_key) {
                if let syn::Member::Named(field_name) = &f.member {
                    // StructInfo.fields is HashMap<String, (usize, Type)>
                    if let Some((_, ft)) = info.fields.get(&field_name.to_string()) {
                        return Ok(ft.clone());
                    }
                }
            }
            Err(format!("Unknown field on type {:?}: {:?}", base_ty, f.member))
        }
        syn::Expr::Reference(r) => {
            let inner = infer_expr_type(ctx, &r.expr, local_vars)?;
            Ok(Type::Reference(Box::new(inner), r.mutability.is_some()))
        }
        syn::Expr::Unary(u) if matches!(u.op, syn::UnOp::Deref(_)) => {
            let inner = infer_expr_type(ctx, &u.expr, local_vars)?;
            match inner {
                Type::Reference(inner_ty, _) => Ok(*inner_ty),
                Type::Owned(inner_ty) => Ok(*inner_ty),
                _ => Err(format!("Dereference on non-reference type: {:?}", inner)),
            }
        }
        _ => Err(format!("Cannot infer type for expression: {:?}", expr)),
    }
}

pub fn type_to_type_key(ty: &Type) -> TypeKey {
    match ty {
        Type::Struct(name) => {
            let parts: Vec<&str> = name.split("__").collect();
            if parts.len() > 1 {
                TypeKey {
                    path: parts[..parts.len()-1].iter().map(|s| s.to_string()).collect(),
                    name: name.clone(),
                    specialization: Some(vec![]),
                }
            } else {
                TypeKey { path: vec![], name: name.clone(), specialization: Some(vec![]) }
            }
        }
        Type::Concrete(name, args) => {
            let parts: Vec<&str> = name.split("__").collect();
            if parts.len() > 1 {
                TypeKey {
                    path: parts[..parts.len()-1].iter().map(|s| s.to_string()).collect(),
                    name: name.clone(),
                    specialization: Some(args.clone()),
                }
            } else {
                TypeKey { path: vec![], name: name.clone(), specialization: Some(args.clone()) }
            }
        }
        Type::Reference(inner, _) => type_to_type_key(inner),
        Type::Owned(inner) => type_to_type_key(inner),
        _ => TypeKey { path: vec![], name: format!("{:?}", ty), specialization: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_to_type_key_struct() {
        let k = type_to_type_key(&Type::Struct("foo".into()));
        assert_eq!(k.name, "foo");
        assert_eq!(k.path, vec![] as Vec<String>);
    }

    #[test]
    fn test_type_to_type_key_pkg_path() {
        let k = type_to_type_key(&Type::Struct("std__core__Vec".into()));
        assert_eq!(k.name, "std__core__Vec");
        assert_eq!(k.path, vec!["std", "core"]);
    }

    #[test]
    fn test_type_to_type_key_reference() {
        let k = type_to_type_key(&Type::Reference(Box::new(Type::Struct("foo".into())), true));
        assert_eq!(k.name, "foo");
    }
}
