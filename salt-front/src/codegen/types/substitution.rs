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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn map_of(pairs: &[(&str, Type)]) -> BTreeMap<String, Type> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    #[test]
    fn test_substitute_generic_binds() {
        let m = map_of(&[("T", Type::I64)]);
        assert_eq!(substitute_generics(&m, &Type::Generic("T".into())), Type::I64);
    }

    #[test]
    fn test_substitute_struct_key() {
        let m = map_of(&[("T", Type::I64)]);
        assert_eq!(substitute_generics(&m, &Type::Struct("T".into())), Type::I64);
    }

    #[test]
    fn test_substitute_generic_not_in_map() {
        let m = BTreeMap::new();
        assert_eq!(substitute_generics(&m, &Type::Generic("T".into())), Type::Generic("T".into()));
    }

    #[test]
    fn test_substitute_concrete_args() {
        let m = map_of(&[("T", Type::I64)]);
        let ty = Type::Concrete("Vec".into(), vec![Type::Generic("T".into())]);
        let expected = Type::Concrete("Vec".into(), vec![Type::I64]);
        assert_eq!(substitute_generics(&m, &ty), expected);
    }

    #[test]
    fn test_substitute_self_type() {
        let m = map_of(&[("Self", Type::Struct("Foo".into()))]);
        assert_eq!(substitute_generics(&m, &Type::SelfType), Type::Struct("Foo".into()));
    }

    #[test]
    fn test_substitute_pointer_recurse() {
        let m = map_of(&[("T", Type::I32)]);
        let ty = Type::Pointer { element: Box::new(Type::Generic("T".into())), provenance: crate::types::Provenance::Naked, is_mutable: false };
        let expected = Type::Pointer { element: Box::new(Type::I32), provenance: crate::types::Provenance::Naked, is_mutable: false };
        assert_eq!(substitute_generics(&m, &ty), expected);
    }

    #[test]
    fn test_substitute_no_infinite_loop() {
        let m = map_of(&[("T", Type::Generic("T".into()))]);
        assert_eq!(substitute_generics(&m, &Type::Struct("T".into())), Type::Generic("T".into()));
    }

    #[test]
    fn test_substitute_fn_type() {
        let m = map_of(&[("T", Type::I64), ("R", Type::Bool)]);
        let ty = Type::Fn(
            vec![Type::Generic("T".into()), Type::I32],
            Box::new(Type::Generic("R".into())),
        );
        let expected = Type::Fn(
            vec![Type::I64, Type::I32],
            Box::new(Type::Bool),
        );
        assert_eq!(substitute_generics(&m, &ty), expected);
    }

    #[test]
    fn test_substitute_array_type() {
        let m = map_of(&[("T", Type::I32)]);
        let ty = Type::Array(Box::new(Type::Generic("T".into())), 10, false);
        let expected = Type::Array(Box::new(Type::I32), 10, false);
        assert_eq!(substitute_generics(&m, &ty), expected);
    }

    #[test]
    fn test_substitute_tuple_type() {
        let m = map_of(&[("A", Type::I32), ("B", Type::F64)]);
        let ty = Type::Tuple(vec![Type::Generic("A".into()), Type::Generic("B".into())]);
        let expected = Type::Tuple(vec![Type::I32, Type::F64]);
        assert_eq!(substitute_generics(&m, &ty), expected);
    }

    #[test]
    fn test_try_suffix_concrete_no_args() {
        // Concrete("foo__Bar", []) with no args triggers try_suffix:
        // last __ component "Bar" is looked up in the type_map
        let m = map_of(&[("Bar", Type::I64)]);
        let ty = Type::Concrete("foo__Bar".into(), vec![]);
        assert_eq!(substitute_generics(&m, &ty), Type::I64);
    }

    #[test]
    fn test_try_suffix_concrete_with_args_skips_suffix() {
        // Concrete with args bypasses try_suffix and substitutes args instead
        let m = map_of(&[("Bar", Type::I64)]);
        let ty = Type::Concrete("foo__Bar".into(), vec![Type::I32]);
        let expected = Type::Concrete("foo__Bar".into(), vec![Type::I32]);
        assert_eq!(substitute_generics(&m, &ty), expected);
    }

    #[test]
    fn test_substitute_reference_immutable() {
        let m = map_of(&[("T", Type::I64)]);
        let ty = Type::Reference(Box::new(Type::Generic("T".into())), false);
        let expected = Type::Reference(Box::new(Type::I64), false);
        assert_eq!(substitute_generics(&m, &ty), expected);
    }

    #[test]
    fn test_substitute_reference_mutable() {
        let m = map_of(&[("T", Type::I32)]);
        let ty = Type::Reference(Box::new(Type::Generic("T".into())), true);
        let expected = Type::Reference(Box::new(Type::I32), true);
        assert_eq!(substitute_generics(&m, &ty), expected);
    }

    #[test]
    fn test_substitute_self_type_not_in_map() {
        let m = BTreeMap::new();
        assert_eq!(substitute_generics(&m, &Type::SelfType), Type::SelfType);
    }
}
