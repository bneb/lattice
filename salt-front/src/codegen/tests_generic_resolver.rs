#[cfg(test)]
mod tests {
    use crate::codegen::generic_resolver::*;
    use crate::types::{Type, Provenance};
    use crate::grammar::GenericParam;
    use std::collections::BTreeMap;

    // ═══════════════════════════════════════════════════════════════════
    // unify_types tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_unify_generic_binds_name() {
        let mut map = BTreeMap::new();
        unify_types(&Type::Generic("T".into()), &Type::I64, &mut map);
        assert_eq!(map.get("T"), Some(&Type::I64));
    }

    #[test]
    fn test_unify_struct_single_char_uppercase_does_not_bind() {
        // After hack removal: Struct("T") is NOT treated as generic
        let mut map = BTreeMap::new();
        unify_types(&Type::Struct("T".into()), &Type::I64, &mut map);
        assert!(map.is_empty(), "Struct('T') should NOT bind — use Generic('T')");
    }

    #[test]
    fn test_unify_struct_multi_char_does_not_bind() {
        let mut map = BTreeMap::new();
        unify_types(&Type::Struct("Range".into()), &Type::I64, &mut map);
        assert!(map.is_empty(), "Multi-char Struct name should not be treated as generic");
    }

    #[test]
    fn test_unify_struct_single_char_does_not_bind() {
        // Hack removed: even Struct("T") is NOT treated as generic
        let mut map = BTreeMap::new();
        unify_types(&Type::Struct("T".into()), &Type::I64, &mut map);
        assert!(map.is_empty(), "Single-char Struct('T') should NOT be treated as generic — use Generic('T')");
    }

    #[test]
    fn test_unify_concrete_recurse() {
        let mut map = BTreeMap::new();
        let template = Type::Concrete("Vec".into(), vec![Type::Generic("T".into())]);
        let concrete = Type::Concrete("Vec".into(), vec![Type::I64]);
        unify_types(&template, &concrete, &mut map);
        assert_eq!(map.get("T"), Some(&Type::I64));
    }

    #[test]
    fn test_unify_fn_type() {
        let mut map = BTreeMap::new();
        let template = Type::Fn(
            vec![Type::Generic("A".into())],
            Box::new(Type::Generic("B".into())),
        );
        let concrete = Type::Fn(vec![Type::I64], Box::new(Type::F64));
        unify_types(&template, &concrete, &mut map);
        assert_eq!(map.get("A"), Some(&Type::I64));
        assert_eq!(map.get("B"), Some(&Type::F64));
    }

    #[test]
    fn test_unify_pointer_recurse() {
        let mut map = BTreeMap::new();
        let template = Type::Pointer {
            element: Box::new(Type::Generic("T".into())),
            provenance: Provenance::Naked,
            is_mutable: false,
        };
        let concrete = Type::Pointer {
            element: Box::new(Type::I32),
            provenance: Provenance::Naked,
            is_mutable: false,
        };
        unify_types(&template, &concrete, &mut map);
        assert_eq!(map.get("T"), Some(&Type::I32));
    }

    #[test]
    fn test_unify_pointer_vs_concrete_ptr() {
        let mut map = BTreeMap::new();
        let template = Type::Pointer {
            element: Box::new(Type::Generic("T".into())),
            provenance: Provenance::Naked,
            is_mutable: false,
        };
        let concrete = Type::Concrete("Ptr".into(), vec![Type::F32]);
        unify_types(&template, &concrete, &mut map);
        assert_eq!(map.get("T"), Some(&Type::F32));
    }

    #[test]
    fn test_unify_reference_auto_deref() {
        let mut map = BTreeMap::new();
        let template = Type::Generic("T".into());
        let concrete = Type::Reference(Box::new(Type::I64), false);
        unify_types(&template, &concrete, &mut map);
        // When pattern is Generic, it binds to the full Reference type
        assert_eq!(map.get("T"), Some(&Type::Reference(Box::new(Type::I64), false)));
    }

    #[test]
    fn test_unify_does_not_overwrite() {
        let mut map = BTreeMap::new();
        map.insert("T".into(), Type::I64);
        unify_types(&Type::Generic("T".into()), &Type::F64, &mut map);
        assert_eq!(map.get("T"), Some(&Type::I64), "Should not overwrite existing binding");
    }

    #[test]
    fn test_unify_nested_concrete() {
        let mut map = BTreeMap::new();
        let template = Type::Concrete(
            "Result".into(),
            vec![
                Type::Concrete("Ptr".into(), vec![Type::Generic("T".into())]),
                Type::Struct("IOError".into()),
            ],
        );
        let concrete = Type::Concrete(
            "Result".into(),
            vec![
                Type::Concrete("Ptr".into(), vec![Type::F32]),
                Type::Struct("IOError".into()),
            ],
        );
        unify_types(&template, &concrete, &mut map);
        assert_eq!(map.get("T"), Some(&Type::F32));
    }

    #[test]
    fn test_unify_qualified_name_matching() {
        let mut map = BTreeMap::new();
        let template = Type::Concrete("Result".into(), vec![Type::Generic("T".into())]);
        let concrete = Type::Concrete("std__core__result__Result".into(), vec![Type::I32]);
        // Should match because "std__core__result__Result" ends with "__Result"
        unify_types(&template, &concrete, &mut map);
        assert_eq!(map.get("T"), Some(&Type::I32));
    }

    // ═══════════════════════════════════════════════════════════════════
    // infer_phantom_generics tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_phantom_basic_fn_return() {
        // Map<I, F, T> with F = Fn(i64)->i64 => T = i64
        let mut map = BTreeMap::new();
        map.insert("I".into(), Type::Struct("Range".into()));
        map.insert("F".into(), Type::Fn(vec![Type::I64], Box::new(Type::I64)));
        let declared = vec!["I".into(), "F".into(), "T".into()];
        infer_phantom_generics(&declared, &mut map);
        assert_eq!(map.get("T"), Some(&Type::I64));
    }

    #[test]
    fn test_phantom_no_unresolved() {
        let mut map = BTreeMap::new();
        map.insert("T".into(), Type::I64);
        let declared = vec!["T".into()];
        infer_phantom_generics(&declared, &mut map);
        assert_eq!(map.get("T"), Some(&Type::I64), "Should not change existing binding");
    }

    #[test]
    fn test_phantom_no_fn_types() {
        let mut map = BTreeMap::new();
        map.insert("I".into(), Type::Struct("Range".into()));
        let declared = vec!["I".into(), "T".into()];
        infer_phantom_generics(&declared, &mut map);
        assert!(!map.contains_key("T"), "Cannot infer phantom without Fn types");
    }

    #[test]
    fn test_phantom_multiple_unresolved_no_infer() {
        // Ambiguous: two unresolved generics, one Fn type
        let mut map = BTreeMap::new();
        map.insert("F".into(), Type::Fn(vec![Type::I64], Box::new(Type::F64)));
        let declared = vec!["F".into(), "T".into(), "U".into()];
        infer_phantom_generics(&declared, &mut map);
        assert!(!map.contains_key("T"), "Should not infer with multiple unresolved");
        assert!(!map.contains_key("U"), "Should not infer with multiple unresolved");
    }

    #[test]
    fn test_phantom_fn_returning_struct() {
        let mut map = BTreeMap::new();
        map.insert("F".into(), Type::Fn(
            vec![Type::I64],
            Box::new(Type::Struct("MyStruct".into())),
        ));
        let declared = vec!["F".into(), "T".into()];
        infer_phantom_generics(&declared, &mut map);
        assert_eq!(map.get("T"), Some(&Type::Struct("MyStruct".into())));
    }

    // ═══════════════════════════════════════════════════════════════════
    // generic_param_name tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_generic_param_name_type() {
        let param = GenericParam::Type {
            name: syn::Ident::new("T", proc_macro2::Span::call_site()),
            constraint: None,
        };
        assert_eq!(generic_param_name(&param), "T");
    }

    #[test]
    fn test_generic_param_name_const() {
        use crate::grammar::SynType;
        let param = GenericParam::Const {
            name: syn::Ident::new("N", proc_macro2::Span::call_site()),
            ty: Box::new(SynType::from_std(syn::parse_quote!(i64)).unwrap()),
        };
        assert_eq!(generic_param_name(&param), "N");
    }

    // ═══════════════════════════════════════════════════════════════════
    // extract_generic_names_from_type tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_extract_generic_names_from_concrete() {
        let ty = Type::Concrete("Ptr".into(), vec![Type::Generic("T".into())]);
        let names = extract_generic_names_from_type(&ty);
        assert_eq!(names, vec!["T".to_string()]);
    }

    #[test]
    fn test_extract_generic_names_mixed() {
        let ty = Type::Concrete(
            "Map".into(),
            vec![Type::Struct("Range".into()), Type::Generic("F".into()), Type::Generic("T".into())],
        );
        let names = extract_generic_names_from_type(&ty);
        assert_eq!(names, vec!["F".to_string(), "T".to_string()]);
    }

    #[test]
    fn test_extract_generic_names_none() {
        let ty = Type::I64;
        let names = extract_generic_names_from_type(&ty);
        assert!(names.is_empty());
    }
}
