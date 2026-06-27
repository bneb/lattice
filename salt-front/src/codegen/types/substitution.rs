use crate::types::Type;
use crate::codegen::context::LoweringContext;

/// Returns true when a type_map entry maps `name` back to itself.
fn is_self_ref(n: &str, c: &Type) -> bool {
    matches!(c, Type::Struct(s) | Type::Generic(s) if s == n)
}

fn sub_through(m: &std::collections::BTreeMap<String, Type>, n: &str, ty: &Type) -> Type {
    let Some(c) = m.get(n) else { return ty.clone(); };
    if is_self_ref(n, c) { return Type::Generic(n.to_string()); }
    substitute_generics(m, c)
}

fn try_suffix(m: &std::collections::BTreeMap<String, Type>, n: &str) -> Option<Type> {
    let f = m.get(n);
    let s = if n.contains("__") { m.get(n.rsplit("__").next()?) } else { None };
    f.or(s).map(|c| substitute_generics(m, c))
}

/// Recursively substitute generic placeholders using current_type_map.
/// When HashMap<i64, i64> references Entry<K, V>, this function consults the
/// active type context to produce Entry<i64, i64>.
pub fn substitute_generics(type_map: &std::collections::BTreeMap<String, Type>, ty: &Type) -> Type {
    match ty {
        Type::Struct(name) if type_map.contains_key(name) => sub_through(type_map, name, ty),
        Type::Generic(name) => sub_through(type_map, name, ty),
        Type::Concrete(name, args) => {
            if args.is_empty() {
                if let Some(result) = try_suffix(type_map, name) {
                    return result;
                }
            }
            let substituted_args: Vec<Type> = args.iter()
                .map(|a| substitute_generics(type_map, a))
                .collect();
            Type::Concrete(name.clone(), substituted_args)
        }
        Type::SelfType => {
            let Some(concrete) = type_map.get("Self") else { return ty.clone(); };
            substitute_generics(type_map, concrete)
        }
        Type::Pointer { element, provenance, is_mutable } => {
            Type::Pointer {
                element: Box::new(substitute_generics(type_map, element)),
                provenance: provenance.clone(),
                is_mutable: *is_mutable,
            }
        }
        Type::Reference(inner, mutability) => {
            Type::Reference(Box::new(substitute_generics(type_map, inner)), *mutability)
        }
        Type::Array(inner, len, packed) => {
            Type::Array(Box::new(substitute_generics(type_map, inner)), *len, *packed)
        }
        Type::Tuple(elems) => {
            Type::Tuple(elems.iter().map(|e| substitute_generics(type_map, e)).collect())
        }
        Type::Fn(args, ret) => {
            Type::Fn(
                args.iter().map(|a| substitute_generics(type_map, a)).collect(),
                Box::new(substitute_generics(type_map, ret)),
            )
        }
        _ => ty.clone()
    }
}

/// Convenience wrapper: extracts type_map from CodegenContext.
pub fn substitute_generics_ctx(ctx: &mut LoweringContext, ty: &Type) -> Type {
    let type_map = ctx.current_type_map();
    substitute_generics(type_map, ty)
}
