use crate::types::{Type, TypeKey};
use crate::codegen::context::LoweringContext;
use crate::common::mangling::Mangler;
use std::collections::HashMap;

pub fn resolve_codegen_type(ctx: &mut LoweringContext, ty: &Type) -> Type {
    let flattened = super::layout::flatten_inception_recursive(ty, 0, "codegen_resolve");
    let res = match &flattened {
        Type::Enum(name) => {
            let mut resolved_name = name.clone();
            {
                let imports = ctx.imports();
                for imp in &*imports {
                    if let Some(group) = &imp.group {
                        if group.iter().any(|id| id.to_string() == *name) {
                            let base = Mangler::mangle(&imp.name.iter().map(|id| id.to_string()).collect::<Vec<_>>());
                            resolved_name = format!("{}__{}", base, name);
                            break;
                        }
                    }
                    if let Some(last) = imp.name.last() {
                        if let Some(alias) = &imp.alias {
                            if alias.to_string() == *name {
                                resolved_name = Mangler::mangle(&imp.name.iter().map(|id| id.to_string()).collect::<Vec<_>>());
                                break;
                            }
                        } else if last.to_string() == *name {
                            resolved_name = Mangler::mangle(&imp.name.iter().map(|id| id.to_string()).collect::<Vec<_>>());
                            break;
                        }
                    }
                }
            }
            Type::Enum(resolved_name)
        }
        Type::Generic(name) => {
            let concrete_opt = {
                ctx.current_type_map().get(name).cloned()
            };

            if let Some(concrete_ty) = concrete_opt {
                 resolve_codegen_type(ctx, &concrete_ty)
            } else if ctx.enum_registry().values().any(|i| i.name == *name) || ctx.enum_templates().contains_key(name) {
                Type::Enum(name.clone())
            } else {
                let mut resolved_name = name.clone();
                {
                    let imports = ctx.imports();
                    for imp in &*imports {
                        if let Some(group) = &imp.group {
                            if group.iter().any(|id| id.to_string() == *name) {
                                let base = Mangler::mangle(&imp.name.iter().map(|id| id.to_string()).collect::<Vec<_>>());
                                resolved_name = Mangler::mangle(&[&base, name]);
                                break;
                            }
                        }
                    }
                }
                Type::Struct(resolved_name)
            }
        }
        Type::Concrete(base, args) => {
            let res_args: Vec<Type> = args.iter().map(|a| resolve_codegen_type(ctx, a)).collect();
            Type::Concrete(base.clone(), res_args)
        }
        Type::Pointer { element, provenance, is_mutable } => {
            Type::Pointer {
                element: Box::new(resolve_codegen_type(ctx, element)),
                provenance: provenance.clone(),
                is_mutable: *is_mutable,
            }
        }
        Type::Reference(inner, m) => {
            Type::Reference(Box::new(resolve_codegen_type(ctx, inner)), *m)
        }
        Type::Owned(inner) => Type::Owned(Box::new(resolve_codegen_type(ctx, inner))),
        _ => flattened.clone(),
    };
    res
}

pub fn resolve_type(ctx: &mut LoweringContext, ty: &crate::grammar::SynType) -> Type {
    if let crate::grammar::SynType::Array(inner, len_expr) = ty {
        let inner_ty = resolve_type(ctx, inner);
        return match ctx.evaluator.eval_expr(len_expr) {
            Ok(crate::evaluator::ConstValue::Integer(val)) => Type::Array(Box::new(inner_ty), val as usize, false),
            _ => Type::I32, // Fallback
        };
    }

    if let crate::grammar::SynType::Path(tp) = ty {
        if let Some(seg) = tp.segments.last() {
            if seg.ident == "Tensor" {
                 if seg.args.len() >= 2 {
                     let inner_syn = &seg.args[0];
                     let inner = resolve_type(ctx, inner_syn);
                     let mut shape = Vec::new();
                     
                     if let crate::grammar::SynType::Path(shape_path) = &seg.args[1] {
                         if let Some(shape_seg) = shape_path.segments.last() {
                             let shape_name = shape_seg.ident.to_string();
                             if shape_name.starts_with("__Shape_") && shape_name.ends_with("__") {
                                 let shape_str = &shape_name[8..shape_name.len()-2];
                                 let all_values: Vec<usize> = shape_str.split('_')
                                     .filter_map(|s| s.parse().ok())
                                     .collect();
                                 if all_values.len() > 1 {
                                     shape = all_values[1..].to_vec();
                                 } else if !all_values.is_empty() {
                                     shape = all_values;
                                 }
                                 return Type::Tensor(Box::new(inner), shape);
                             }
                         }
                     }
                     
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
    }

    if let Some(t) = Type::from_syn(ty) {
        resolve_codegen_type(ctx, &t)
    } else {
        Type::Unit
    }
}

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
            
            if let Some((ty, _)) = local_vars.get(&name) {
                return Ok(ty.clone());
            }
            
            if p.path.segments.len() == 1 {
                let simple_name = p.path.segments[0].ident.to_string();
                if let Some((ty, _)) = local_vars.get(&simple_name) {
                    return Ok(ty.clone());
                }
            }
            
            if let Some(ty) = ctx.globals().get(&name) {
                return Ok(ty.clone());
            }
            
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
            let type_key = type_to_type_key(&base_ty);
            if let Some(info) = ctx.struct_registry().get(&type_key) {
                if let syn::Member::Named(field_name) = &f.member {
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
        Type::Pointer { .. } => {
            TypeKey { path: vec!["std".to_string(), "core".to_string(), "ptr".to_string()], name: "Ptr".to_string(), specialization: None }
        }
        Type::Reference(inner, _) => type_to_type_key(inner),
        Type::Owned(inner) => type_to_type_key(inner),
        _ => TypeKey { path: vec![], name: ty.mangle_suffix(), specialization: None },
    }
}
