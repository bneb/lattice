use crate::types::{Type, TypeKey};
use crate::grammar::{SaltFn, ImportDecl};
use crate::codegen::context::{CodegenContext, LoweringContext};

pub fn resolve_method_impl(ctx: &CodegenContext, receiver_ty: &Type, method_name: &str) -> Result<(SaltFn, Option<Type>, Vec<ImportDecl>), String> {
    let mut current_ty = receiver_ty.clone();
    let mut depth = 0;
    
    loop {
        if depth > 10 { break; }
        depth += 1;

        if let Some(key) = current_ty.to_key() {
            if let Some(result) = ctx.discovery.borrow().trait_registry.get_legacy(&key, method_name) {
                return Ok(result);
            }
            let template_key = key.to_template();
            if let Some(result) = ctx.discovery.borrow().trait_registry.get_legacy(&template_key, method_name) {
                return Ok(result);
            }
            if let Some(result) = ctx.discovery.borrow().trait_registry.find_method_by_name(&key.name, method_name, &current_ty) {
                return Ok(result);
            }
        }

        if let Some(next_ty) = current_ty.peel_reference() {
            current_ty = next_ty.clone();
        } else {
            break;
        }
    }
    
    Err(format!("Method '{}' not found for type {:?}", method_name, receiver_ty))
}

pub fn resolve_method_lctx_impl(ctx: &LoweringContext, receiver_ty: &Type, method_name: &str) -> Result<(SaltFn, Option<Type>, Vec<ImportDecl>), String> {
    let mut current_ty = receiver_ty.clone();
    let mut depth = 0;
    
    loop {
        if depth > 10 { break; }
        depth += 1;

        if let Some(key) = current_ty.to_key() {
            if let Some(result) = ctx.discovery.trait_registry.get_legacy(&key, method_name) {
                return Ok(result);
            }
            let template_key = key.to_template();
            if let Some(result) = ctx.discovery.trait_registry.get_legacy(&template_key, method_name) {
                return Ok(result);
            }
            if let Some(result) = ctx.discovery.trait_registry.find_method_by_name(&key.name, method_name, &current_ty) {
                return Ok(result);
            }
        }

        if let Some(next_ty) = current_ty.peel_reference() {
            current_ty = next_ty.clone();
        } else {
            break;
        }
    }
    
    Err(format!("Method '{}' not found for type {:?}", method_name, receiver_ty))
}

pub fn resolve_method_to_task_impl(ctx: &CodegenContext, receiver_ty: &Type, method_name: &str, generics: Vec<Type>) -> Result<crate::codegen::collector::MonomorphizationTask, String> {
    let (func, trait_ty, imports) = resolve_method_impl(ctx, receiver_ty, method_name)?;
    
    let mut type_map = std::collections::BTreeMap::new();
    if let Some(Type::Concrete(name, args)) = &trait_ty {
            if let Some(template) = ctx.discovery.borrow().struct_templates.get(name) {
                if let Some(t_generics) = &template.generics {
                    for (i, param) in t_generics.params.iter().enumerate() {
                        let param_name = match param {
                            crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                            crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                        };
                        if i < args.len() {
                            type_map.insert(param_name, args[i].clone());
                        }
                    }
                }
            }
    }

    let mangled_name = if let Some(self_ty) = &trait_ty {
        format!("{}__{}", self_ty.mangle_suffix(), method_name)
    } else {
        method_name.to_string()
    };

    Ok(crate::codegen::collector::MonomorphizationTask {
        identity: TypeKey { path: vec![], name: mangled_name.clone(), specialization: None },
        mangled_name,
        func,
        concrete_tys: generics,
        self_ty: trait_ty,
        imports,
        type_map,
    })
}
