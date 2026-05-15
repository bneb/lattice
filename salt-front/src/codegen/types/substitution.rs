use crate::types::Type;
use crate::codegen::context::LoweringContext;

/// [GRAYDON FIX] Recursively substitute generic placeholders using current_type_map.
/// This is the "Secret of $i64$" - when HashMap<i64, i64> looks at Entry<K, V>,
/// this function transforms it to Entry<i64, i64> by consulting the active type context.
pub fn substitute_generics(type_map: &std::collections::BTreeMap<String, Type>, ty: &Type) -> Type {
    match ty {
        // Generics stored as Struct names (parser artifact) — check type_map
        Type::Struct(name) if type_map.contains_key(name) => {
            let concrete = &type_map[name].clone();
            // Guard against self-referential mappings that cause infinite loops
            if let Type::Generic(concrete_name) = concrete {
                if concrete_name == name {
                    return Type::Generic(name.clone());
                }
            }

            substitute_generics(type_map, concrete)
        }
        // Explicit Generic type
        Type::Generic(name) => {
            if let Some(concrete) = type_map.get(name) {
                substitute_generics(type_map, concrete)
            } else {
                ty.clone()
            }
        }
        // Concrete types with generic args (e.g., Entry<K, V>)
        Type::Concrete(name, args) => {
            let substituted_args: Vec<Type> = args.iter()
                .map(|a| substitute_generics(type_map, a))
                .collect();
            Type::Concrete(name.clone(), substituted_args)
        }
        // Pointer types
        Type::Pointer { element, provenance, is_mutable } => {
            Type::Pointer {
                element: Box::new(substitute_generics(type_map, element)),
                provenance: provenance.clone(),
                is_mutable: *is_mutable,
            }
        }
        // Reference types
        Type::Reference(inner, mutability) => {
            Type::Reference(Box::new(substitute_generics(type_map, inner)), *mutability)
        }
        // Array types
        Type::Array(inner, len, packed) => {
            Type::Array(Box::new(substitute_generics(type_map, inner)), *len, *packed)
        }
        // Tuple types
        Type::Tuple(elems) => {
            Type::Tuple(elems.iter().map(|e| substitute_generics(type_map, e)).collect())
        }
        // Function types: recursively substitute generics in arg and return types
        Type::Fn(args, ret) => {
            Type::Fn(
                args.iter().map(|a| substitute_generics(type_map, a)).collect(),
                Box::new(substitute_generics(type_map, ret)),
            )
        }
        // All other types pass through unchanged
        _ => ty.clone()
    }
}

/// Convenience wrapper: extracts type_map from CodegenContext.
pub fn substitute_generics_ctx(ctx: &mut LoweringContext, ty: &Type) -> Type {
    let type_map = ctx.current_type_map();
    substitute_generics(&type_map, ty)
}
