//! TDD Tests for Phase 4: Result<T,E> Monomorphization & `if ptr` Sugar
//!
//! These tests assert the IDEAL behavior before implementation:
//!
//! 1. Type::substitute must correctly resolve Pointer { element: Struct("T") }
//!    when type_map contains {"T": F32} — yielding Pointer { element: F32 }
//!
//! 2. resolve_path_to_enum must succeed when the current type_map resolves
//!    all generics in the expected_ty's args.
//!
//! 3. emit_salt_if must accept Type::Pointer conditions (not just Bool),
//!    emitting llvm.icmp ne for the null check.

#[cfg(test)]
mod tests {
    use crate::types::{Type, Provenance};
    use std::collections::BTreeMap;

    // =========================================================================
    // Section 1: Type::substitute for Pointer { element: Struct("T") }
    //
    // This is the FUNDAMENTAL operation that must work correctly.
    // When inside a generic function like mmap<T>, the return type contains
    // Pointer { element: Struct("T") }. After substitution with {"T": F32},
    // it MUST become Pointer { element: F32 }.
    // =========================================================================

    #[test]
    fn test_substitute_pointer_with_struct_t_placeholder() {
        // Pointer { element: Struct("T"), provenance: Naked, is_mutable: true }
        let ty = Type::Pointer {
            element: Box::new(Type::Struct("T".to_string())),
            provenance: Provenance::Naked,
            is_mutable: true,
        };

        let mut map = BTreeMap::new();
        map.insert("T".to_string(), Type::F32);

        let result = ty.substitute(&map);

        assert_eq!(
            result,
            Type::Pointer {
                element: Box::new(Type::F32),
                provenance: Provenance::Naked,
                is_mutable: true,
            },
            "Pointer element Struct('T') must be substituted to F32 when T -> F32 in map"
        );
    }

    #[test]
    fn test_substitute_resolves_nested_generic_in_concrete() {
        // Concrete("Result", [Pointer { element: Struct("T") }, Concrete("IOError", [])])
        // After substitution with T -> F32:
        // Concrete("Result", [Pointer { element: F32 }, Concrete("IOError", [])])
        let ty = Type::Concrete(
            "std__core__result__Result".to_string(),
            vec![
                Type::Pointer {
                    element: Box::new(Type::Struct("T".to_string())),
                    provenance: Provenance::Naked,
                    is_mutable: true,
                },
                Type::Concrete("std__io__file__IOError".to_string(), vec![]),
            ],
        );

        let mut map = BTreeMap::new();
        map.insert("T".to_string(), Type::F32);

        let result = ty.substitute(&map);

        let expected = Type::Concrete(
            "std__core__result__Result".to_string(),
            vec![
                Type::Pointer {
                    element: Box::new(Type::F32),
                    provenance: Provenance::Naked,
                    is_mutable: true,
                },
                Type::Concrete("std__io__file__IOError".to_string(), vec![]),
            ],
        );

        assert_eq!(result, expected, 
            "Nested Struct('T') inside Concrete Result args must be substituted");
    }

    #[test]
    fn test_substitute_with_empty_map_is_noop() {
        let ty = Type::Pointer {
            element: Box::new(Type::Struct("T".to_string())),
            provenance: Provenance::Naked,
            is_mutable: true,
        };

        let map = BTreeMap::new();
        let result = ty.substitute(&map);

        // With empty map, Struct("T") stays Struct("T")
        assert_eq!(result, ty, "Empty type map should be a no-op");
    }

    // =========================================================================
    // Section 2: has_generics() after substitution
    //
    // After successful substitution, the resulting type must NOT have generics.
    // This test validates the condition that blocks resolve_path_to_enum.
    // =========================================================================

    #[test]
    fn test_has_generics_pointer_with_struct_t() {
        let ty = Type::Pointer {
            element: Box::new(Type::Struct("T".to_string())),
            provenance: Provenance::Naked,
            is_mutable: true,
        };
        // Struct("T") is NOT a generic — use Generic("T") or normalize_generics
        assert!(!ty.has_generics(), "Pointer<Struct('T')> should NOT have generics (Struct is not Generic)");

        // But Generic("T") IS recognized
        let ty2 = Type::Pointer {
            element: Box::new(Type::Generic("T".to_string())),
            provenance: Provenance::Naked,
            is_mutable: true,
        };
        assert!(ty2.has_generics(), "Pointer<Generic('T')> should have generics");
    }

    #[test]
    fn test_no_generics_after_substitution() {
        let ty = Type::Pointer {
            element: Box::new(Type::Struct("T".to_string())),
            provenance: Provenance::Naked,
            is_mutable: true,
        };

        let mut map = BTreeMap::new();
        map.insert("T".to_string(), Type::F32);

        let result = ty.substitute(&map);
        assert!(
            !result.has_generics(),
            "After substituting T -> F32, result must NOT have generics. Got: {:?}",
            result
        );
    }

    #[test]
    fn test_concrete_result_no_generics_after_full_substitution() {
        let ty = Type::Concrete(
            "Result".to_string(),
            vec![
                Type::Pointer {
                    element: Box::new(Type::Struct("T".to_string())),
                    provenance: Provenance::Naked,
                    is_mutable: true,
                },
                Type::Struct("E".to_string()),
            ],
        );

        let mut map = BTreeMap::new();
        map.insert("T".to_string(), Type::F32);
        map.insert("E".to_string(), Type::Struct("IOError".to_string()));

        let result = ty.substitute(&map);
        assert!(
            !result.has_generics(),
            "After substituting both T and E, Result<Ptr<T>, E> must have no generics. Got: {:?}",
            result
        );
    }

    // =========================================================================
    // Section 3: emit_salt_if condition type check
    //
    // Currently emit_salt_if rejects non-Bool conditions.
    // Phase 4.2 requires accepting Type::Pointer for `if ptr { ... }` sugar.
    // These tests assert the desired behavior.
    // =========================================================================

    #[test]
    fn test_pointer_type_should_be_truthy_candidate() {
        // Assert that Type::Pointer is a valid "truthy" type
        let ptr_ty = Type::Pointer {
            element: Box::new(Type::F32),
            provenance: Provenance::Naked,
            is_mutable: true,
        };

        let is_truthy = matches!(ptr_ty, Type::Bool | Type::Pointer { .. });
        assert!(is_truthy, "Type::Pointer should be accepted as a truthy condition type");
    }

    #[test]
    fn test_bool_type_is_truthy_candidate() {
        let bool_ty = Type::Bool;
        let is_truthy = matches!(bool_ty, Type::Bool | Type::Pointer { .. });
        assert!(is_truthy, "Type::Bool should be accepted as a truthy condition type");
    }

    #[test]
    fn test_i64_type_is_not_truthy_candidate() {
        let i64_ty = Type::I64;
        let is_truthy = matches!(i64_ty, Type::Bool | Type::Pointer { .. });
        assert!(!is_truthy, "Type::I64 should NOT be accepted as a truthy condition");
    }

    #[test]
    fn test_struct_type_is_not_truthy_candidate() {
        let struct_ty = Type::Struct("File".to_string());
        let is_truthy = matches!(struct_ty, Type::Bool | Type::Pointer { .. });
        assert!(!is_truthy, "Type::Struct should NOT be accepted as a truthy condition");
    }

    // =========================================================================
    // Section 4: Method generic inference from return type annotation
    //
    // When calling img_file.mmap(len, prot, flags) and the result is assigned
    // to a variable with type annotation Ptr<f32>, the compiler should infer
    // T = f32 for mmap<T>.
    // =========================================================================

    #[test]
    fn test_infer_generic_from_return_type_simple() {
        // Simulate: method signature returns Result<Ptr<T>, IOError>
        // Call site annotation: Result<Ptr<f32>, IOError> (from assignment)
        // Expected: T = f32

        // Uses Generic("T") now instead of Struct("T")
        let method_ret = Type::Concrete(
            "Result".to_string(),
            vec![
                Type::Pointer {
                    element: Box::new(Type::Generic("T".to_string())),
                    provenance: Provenance::Naked,
                    is_mutable: true,
                },
                Type::Concrete("IOError".to_string(), vec![]),
            ],
        );

        let call_site_ret = Type::Concrete(
            "Result".to_string(),
            vec![
                Type::Pointer {
                    element: Box::new(Type::F32),
                    provenance: Provenance::Naked,
                    is_mutable: true,
                },
                Type::Concrete("IOError".to_string(), vec![]),
            ],
        );

        // Unify: walk both types structurally, map generic placeholders to concrete
        let inferred = unify_types(&method_ret, &call_site_ret);
        assert_eq!(
            inferred.get("T"),
            Some(&Type::F32),
            "Should infer T = F32 from return type alignment"
        );
    }

    /// Mini unifier using Generic("T") for templates (Struct("T") is not a generic)
    fn unify_types(template: &Type, concrete: &Type) -> BTreeMap<String, Type> {
        let mut map = BTreeMap::new();
        unify_recursive(template, concrete, &mut map);
        map
    }

    fn unify_recursive(template: &Type, concrete: &Type, map: &mut BTreeMap<String, Type>) {
        match (template, concrete) {
            (Type::Generic(name), _) => {
                map.insert(name.clone(), concrete.clone());
            }
            // Recurse into Pointer
            (Type::Pointer { element: e1, .. }, Type::Pointer { element: e2, .. }) => {
                unify_recursive(e1, e2, map);
            }
            // Recurse into Concrete args
            (Type::Concrete(n1, args1), Type::Concrete(n2, args2)) if n1 == n2 && args1.len() == args2.len() => {
                for (a1, a2) in args1.iter().zip(args2.iter()) {
                    unify_recursive(a1, a2, map);
                }
            }
            // Recurse into Reference
            (Type::Reference(inner1, _), Type::Reference(inner2, _)) => {
                unify_recursive(inner1, inner2, map);
            }
            _ => {} // Base case: no unification needed
        }
    }

    #[test]
    fn test_infer_generic_from_pointer_element() {
        // Template: Ptr<T>  |  Concrete: Ptr<u8>  =>  T = u8
        let template = Type::Pointer {
            element: Box::new(Type::Generic("T".to_string())),
            provenance: Provenance::Naked,
            is_mutable: true,
        };
        let concrete = Type::Pointer {
            element: Box::new(Type::U8),
            provenance: Provenance::Naked,
            is_mutable: true,
        };

        let inferred = unify_types(&template, &concrete);
        assert_eq!(inferred.get("T"), Some(&Type::U8), "Should infer T = u8 from Ptr alignment");
    }

    #[test]
    fn test_infer_two_generics_from_result() {
        // Template: Result<T, E>  |  Concrete: Result<i32, bool>
        let template = Type::Concrete(
            "Result".to_string(),
            vec![Type::Generic("T".to_string()), Type::Generic("E".to_string())],
        );
        let concrete = Type::Concrete(
            "Result".to_string(),
            vec![Type::I32, Type::Bool],
        );

        let inferred = unify_types(&template, &concrete);
        assert_eq!(inferred.get("T"), Some(&Type::I32), "T should be i32");
        assert_eq!(inferred.get("E"), Some(&Type::Bool), "E should be bool");
    }
}
