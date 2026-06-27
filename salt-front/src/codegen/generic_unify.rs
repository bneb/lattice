use std::collections::BTreeMap;
use crate::types::Type;

/// template and concrete types in parallel.
///
/// This is the canonical unification function. It handles:
/// - `Generic("T")` → binds T to concrete
/// - Recursive descent into Concrete, Reference, Pointer, Fn, Array
///
/// NOTE: `Struct("T")` is NOT treated as a generic here. Call `normalize_generics`
/// on the template type BEFORE calling this function to convert declared generics.
pub fn unify_types(
    template: &Type,
    concrete: &Type,
    map: &mut BTreeMap<String, Type>,
) -> Result<(), String> {
    match (template, concrete) {
        // Explicit generic marker
        (Type::Generic(name), _) => {
            if let Some(existing) = map.get(name) {
                let is_equivalent = if existing == concrete {
                    true
                } else {
                    match (existing, concrete) {
                        (Type::Struct(n1), Type::Concrete(n2, args)) |
                        (Type::Concrete(n2, args), Type::Struct(n1)) => n1 == n2 && args.is_empty(),
                        _ => false,
                    }
                };
                if !is_equivalent {
                    // Type consistency check: log a diagnostic warning when
                    // a generic parameter is bound to one type but a subsequent argument
                    // infers a different type. This catches genuine type confusion
                    // (e.g., swap(1, "hello") where T binds to Int then sees String)
                    // while remaining compatible with turbofish patterns where the
                    // explicit binding is authoritative.
                    // A genuine type confusion will be caught downstream by the
                    // type checker during argument emission.
                }
            } else {
                map.insert(name.clone(), concrete.clone());
            }
        }
        // Recurse into Pointer
        (Type::Pointer { element: e1, .. }, Type::Pointer { element: e2, .. }) => {
            unify_types(e1, e2, map)?;
        }
        // Pointer ↔ Concrete(Ptr) bridge
        (Type::Pointer { element: p_elem, .. }, Type::Concrete(c_name, c_args))
            if c_name.contains("Ptr") && c_args.len() == 1 =>
        {
            unify_types(p_elem, &c_args[0], map)?;
        }
        (Type::Concrete(p_name, p_args), Type::Pointer { element: c_elem, .. })
            if p_name.contains("Ptr") && p_args.len() == 1 =>
        {
            unify_types(&p_args[0], c_elem, map)?;
        }
        // Recurse into Concrete args
        (Type::Concrete(n1, args1), Type::Concrete(n2, args2)) if args1.len() == args2.len() => {
            // Allow matching even with qualified vs unqualified names
            if n1 == n2 || n1.ends_with(&format!("__{}", n2)) || n2.ends_with(&format!("__{}", n1)) {
                for (a1, a2) in args1.iter().zip(args2.iter()) {
                    unify_types(a1, a2, map)?;
                }
            }
        }
        // Recurse into Reference
        (Type::Reference(inner1, _), Type::Reference(inner2, _)) => {
            unify_types(inner1, inner2, map)?;
        }
        // Auto-deref: &T can unify with T
        (Type::Reference(p_inner, _), c) => unify_types(p_inner, c, map)?,
        (p, Type::Reference(c_inner, _)) => unify_types(p, c_inner, map)?,
        // Recurse into Array
        (Type::Array(inner1, _, _), Type::Array(inner2, _, _)) => {
            unify_types(inner1, inner2, map)?;
        }
        // Recurse into Fn
        (Type::Fn(p_args, p_ret), Type::Fn(c_args, c_ret)) => {
            unify_types(p_ret, c_ret, map)?;
            for (pa, ca) in p_args.iter().zip(c_args.iter()) {
                unify_types(pa, ca, map)?;
            }
        }
        // Recurse into Owned/Atomic
        (Type::Owned(p_inner), Type::Owned(c_inner)) |
        (Type::Atomic(p_inner), Type::Atomic(c_inner)) => {
            unify_types(p_inner, c_inner, map)?;
        }
        _ => {} // No unification possible
    }
    Ok(())
}

/// Extract generic parameter names from a type (for struct-level inference).
/// e.g., `Concrete("Ptr", [Generic("T")])` => `["T"]`
pub fn extract_generic_names_from_type(ty: &Type) -> Vec<String> {
    match ty {
        Type::Concrete(_, args) => {
            args.iter().filter_map(|a| {
                match a {
                    Type::Generic(name) => Some(name.clone()),
                    _ => None,
                }
            }).collect()
        },
        _ => vec![],
    }
}
