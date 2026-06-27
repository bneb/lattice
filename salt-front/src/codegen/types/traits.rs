use crate::types::Type;
use crate::codegen::context::LoweringContext;
use crate::codegen::types::resolution::type_to_type_key;

pub fn check_trait_constraint(
    ctx: &mut LoweringContext,
    concrete_type: &Type,
    trait_name: &str,
) -> Result<(), String> {
    let type_key = type_to_type_key(concrete_type);

    let trait_exists = ctx.trait_registry().get_trait(trait_name).is_some();
    if !trait_exists {
        return Ok(());
    }

    if ctx.trait_registry().get_trait_impl(&type_key, trait_name).is_some() {
        return Ok(());
    }

    if let Some(trait_def) = ctx.trait_registry().get_trait(trait_name) {
        let required_methods: Vec<String> = trait_def.method_signatures.iter()
            .map(|m| m.name.clone())
            .collect();

        for method_name in &required_methods {
            if !ctx.trait_registry().contains_method(&type_key, method_name) {
                return Err(format!(
                    "Type '{}' does not satisfy trait '{}': missing method '{}'",
                    concrete_type.mangle_suffix(),
                    trait_name,
                    method_name
                ));
            }
        }

        return Ok(());
    }

    Err(format!(
        "Type '{}' does not implement trait '{}'",
        concrete_type.mangle_suffix(),
        trait_name
    ))
}

pub fn validate_trait_constraints(
    ctx: &mut LoweringContext,
    generics: &Option<crate::grammar::Generics>,
    concrete_types: &[Type],
) -> Result<(), String> {
    let generics = match generics {
        Some(g) => g,
        None => return Ok(()),
    };

    let type_params: Vec<_> = generics.params.iter()
        .filter_map(|p| {
            if let crate::grammar::GenericParam::Type { name, constraint } = p {
                Some((name.to_string(), constraint.as_ref().map(|c| c.to_string())))
            } else {
                None
            }
        })
        .collect();

    for (i, (param_name, constraint)) in type_params.iter().enumerate() {
        if let Some(trait_name) = constraint {
            if let Some(concrete_ty) = concrete_types.get(i) {
                check_trait_constraint(ctx, concrete_ty, trait_name)
                    .map_err(|e| format!(
                        "Constraint violation for type parameter '{}': {}",
                        param_name, e
                    ))?;
            }
        }
    }

    Ok(())
}

pub fn has_unresolved_type_params(ctx: &mut LoweringContext, ty: &Type) -> bool {
    match ty {
        Type::Generic(_) => true,
        Type::Struct(name) => {
            if let Some(Type::Struct(mapped_name)) = ctx.current_type_map().get(name) {
                if mapped_name == name { return true; }
            }
            let is_known = ctx.struct_registry().keys().any(|k| k.name.ends_with(name))
                || ctx.enum_templates().contains_key(name);
            !is_known && !name.contains("__")
        }
        Type::Concrete(name, args) => {
            let base_unresolved = if args.is_empty() {
                !ctx.struct_registry().keys().any(|k| k.name.ends_with(name))
                    && !ctx.enum_templates().contains_key(name)
                    && !name.contains("__")
            } else { false };
            base_unresolved || args.iter().any(|a| has_unresolved_type_params(ctx, a))
        }
        Type::Pointer { element, .. } => has_unresolved_type_params(ctx, element),
        Type::Reference(inner, _) | Type::Owned(inner) | Type::Atomic(inner) => has_unresolved_type_params(ctx, inner),
        Type::Array(inner, _, _) => has_unresolved_type_params(ctx, inner),
        Type::Fn(args, ret) => args.iter().any(|a| has_unresolved_type_params(ctx, a)) || has_unresolved_type_params(ctx, ret),
        Type::Tuple(elems) => elems.iter().any(|e| has_unresolved_type_params(ctx, e)),
        _ => false,
    }
}
