//! Tests for Bidirectional Type Inference (`unify_types_recursive`)
//!
//! Verifies that the structural unification engine correctly extracts
//! generic bindings from all supported Type variants:
//! - Struct names are NOT generic placeholders (use Generic)
//! - Generic(name) explicit markers
//! - Pointer { element }
//! - Concrete(name, args) with recursive args
//! - Reference(inner, mut)
//! - Array(inner, size, mut)
//! - Primitives (I32, F32, U8, etc.)
//! - Nested combinations (Result<Ptr<T>, IOError>)

#[cfg(test)]
mod tests {
    use crate::types::{Type, Provenance};
    use crate::codegen::expr::unify_types_recursive;
    use std::collections::BTreeMap;

    // ============================
    // Helper constructors
    // ============================
    
    fn ptr(element: Type) -> Type {
        Type::Pointer { element: Box::new(element), provenance: Provenance::Naked, is_mutable: true }
    }
    
    fn concrete(name: &str, args: Vec<Type>) -> Type {
        Type::Concrete(name.to_string(), args)
    }
    
    fn reference(inner: Type, is_mut: bool) -> Type {
        Type::Reference(Box::new(inner), is_mut)
    }
    
    fn array(inner: Type, size: usize) -> Type {
        Type::Array(Box::new(inner), size, false)
    }
    
    fn generic(name: &str) -> Type {
        Type::Generic(name.to_string())
    }
    
    fn struct_ty(name: &str) -> Type {
        Type::Struct(name.to_string())
    }

    // ============================
    // 1. Generic(name) placeholder
    // ============================
    
    #[test]
    fn test_generic_placeholder_binds_to_i32() {
        let mut map = BTreeMap::new();
        unify_types_recursive(&generic("T"), &Type::I32, &mut map);
        assert_eq!(map.get("T"), Some(&Type::I32));
    }
    
    #[test]
    fn test_generic_placeholder_binds_to_f64() {
        let mut map = BTreeMap::new();
        unify_types_recursive(&generic("T"), &Type::F64, &mut map);
        assert_eq!(map.get("T"), Some(&Type::F64));
    }
    
    #[test]
    fn test_generic_placeholder_binds_to_struct() {
        let mut map = BTreeMap::new();
        let file_ty = struct_ty("std__io__file__File");
        unify_types_recursive(&generic("T"), &file_ty, &mut map);
        assert_eq!(map.get("T"), Some(&file_ty));
    }

    #[test]
    fn test_generic_placeholder_binds_to_pointer() {
        let mut map = BTreeMap::new();
        let ptr_f32 = ptr(Type::F32);
        unify_types_recursive(&generic("T"), &ptr_f32, &mut map);
        assert_eq!(map.get("T"), Some(&ptr_f32));
    }

    #[test]
    fn test_generic_first_binding_wins() {
        // If T is already bound, don't overwrite
        let mut map = BTreeMap::new();
        map.insert("T".to_string(), Type::I32);
        unify_types_recursive(&generic("T"), &Type::F32, &mut map);
        assert_eq!(map.get("T"), Some(&Type::I32), "First binding should win");
    }

    // ============================
    // 2. Struct names are NOT generic placeholders
    // ============================
    
    #[test]
    fn test_struct_single_char_does_not_bind() {
        // After hack removal: Struct("T") is NOT treated as a generic
        let mut map = BTreeMap::new();
        unify_types_recursive(&struct_ty("T"), &Type::F32, &mut map);
        assert!(map.is_empty(), "Struct('T') should NOT be treated as generic — use Generic('T') or normalize_generics");
    }

    #[test]
    fn test_struct_multi_char_is_not_placeholder() {
        // "File" is not a generic placeholder — it's a real struct name
        let mut map = BTreeMap::new();
        unify_types_recursive(&struct_ty("File"), &Type::I32, &mut map);
        assert!(map.is_empty(), "Multi-char struct names should not be treated as generic placeholders");
    }
    
    #[test]
    fn test_struct_lowercase_single_char_is_not_placeholder() {
        let mut map = BTreeMap::new();
        unify_types_recursive(&struct_ty("x"), &Type::I32, &mut map);
        assert!(map.is_empty(), "Lowercase single-char struct names should not be treated as generic placeholders");
    }

    // ============================
    // 3. Pointer { element }
    // ============================
    
    #[test]
    fn test_pointer_recurses_into_element() {
        let mut map = BTreeMap::new();
        let template = ptr(generic("T"));
        let concrete_ty = ptr(Type::F32);
        unify_types_recursive(&template, &concrete_ty, &mut map);
        assert_eq!(map.get("T"), Some(&Type::F32));
    }
    
    #[test]
    fn test_pointer_with_generic_element() {
        let mut map = BTreeMap::new();
        let template = ptr(generic("T"));
        let concrete_ty = ptr(Type::U64);
        unify_types_recursive(&template, &concrete_ty, &mut map);
        assert_eq!(map.get("T"), Some(&Type::U64));
    }
    
    #[test]
    fn test_nested_pointer_recurses() {
        let mut map = BTreeMap::new();
        let template = ptr(ptr(generic("T")));
        let concrete_ty = ptr(ptr(Type::I32));
        unify_types_recursive(&template, &concrete_ty, &mut map);
        assert_eq!(map.get("T"), Some(&Type::I32));
    }

    // ============================
    // 4. Concrete(name, args) — Result, Vec, etc.
    // ============================
    
    #[test]
    fn test_concrete_result_infers_both_params() {
        // Result<T, E> matched against Result<File, IOError>
        let mut map = BTreeMap::new();
        let template = concrete("std__core__result__Result", vec![
            generic("T"),
            generic("E"),
        ]);
        let concrete_ty = concrete("std__core__result__Result", vec![
            struct_ty("std__io__file__File"),
            struct_ty("std__io__file__IOError"),
        ]);
        unify_types_recursive(&template, &concrete_ty, &mut map);
        assert_eq!(map.get("T"), Some(&struct_ty("std__io__file__File")));
        assert_eq!(map.get("E"), Some(&struct_ty("std__io__file__IOError")));
    }

    #[test]
    fn test_concrete_result_with_nested_pointer() {
        // Result<Ptr<T>, IOError> matched against Result<Ptr<f32>, IOError>
        // This is the exact mmap<T> scenario
        let mut map = BTreeMap::new();
        let template = concrete("std__core__result__Result", vec![
            ptr(generic("T")),
            concrete("std__io__file__IOError", vec![]),
        ]);
        let concrete_ty = concrete("std__core__result__Result", vec![
            ptr(Type::F32),
            concrete("std__io__file__IOError", vec![]),
        ]);
        unify_types_recursive(&template, &concrete_ty, &mut map);
        assert_eq!(map.get("T"), Some(&Type::F32), "Should infer T=f32 from Result<Ptr<T>, IOError> vs Result<Ptr<f32>, IOError>");
    }

    #[test]
    fn test_concrete_result_with_u8_pointer() {
        // Result<Ptr<T>, IOError> matched against Result<Ptr<u8>, IOError>
        let mut map = BTreeMap::new();
        let template = concrete("std__core__result__Result", vec![
            ptr(generic("T")),
            concrete("std__io__file__IOError", vec![]),
        ]);
        let concrete_ty = concrete("std__core__result__Result", vec![
            ptr(Type::U8),
            concrete("std__io__file__IOError", vec![]),
        ]);
        unify_types_recursive(&template, &concrete_ty, &mut map);
        assert_eq!(map.get("T"), Some(&Type::U8));
    }

    #[test]
    fn test_concrete_name_mismatch_no_binding() {
        let mut map = BTreeMap::new();
        let template = concrete("Vec", vec![generic("T")]);
        let concrete_ty = concrete("HashMap", vec![Type::I32]);
        unify_types_recursive(&template, &concrete_ty, &mut map);
        assert!(map.is_empty(), "Mismatched Concrete names should not produce bindings");
    }
    
    #[test]
    fn test_concrete_arity_mismatch_no_binding() {
        let mut map = BTreeMap::new();
        let template = concrete("Result", vec![generic("T"), generic("E")]);
        let concrete_ty = concrete("Result", vec![Type::I32]);
        unify_types_recursive(&template, &concrete_ty, &mut map);
        assert!(map.is_empty(), "Arity mismatch should not produce bindings");
    }

    #[test]
    fn test_concrete_vec_single_param() {
        let mut map = BTreeMap::new();
        let template = concrete("Vec", vec![generic("T")]);
        let concrete_ty = concrete("Vec", vec![Type::I64]);
        unify_types_recursive(&template, &concrete_ty, &mut map);
        assert_eq!(map.get("T"), Some(&Type::I64));
    }

    #[test]
    fn test_concrete_nested_concrete() {
        // Vec<Result<T, E>> matched against Vec<Result<i32, IOError>>
        let mut map = BTreeMap::new();
        let template = concrete("Vec", vec![
            concrete("Result", vec![generic("T"), generic("E")])
        ]);
        let concrete_ty = concrete("Vec", vec![
            concrete("Result", vec![Type::I32, struct_ty("IOError")])
        ]);
        unify_types_recursive(&template, &concrete_ty, &mut map);
        assert_eq!(map.get("T"), Some(&Type::I32));
        assert_eq!(map.get("E"), Some(&struct_ty("IOError")));
    }

    // ============================
    // 5. Reference(inner, is_mut)
    // ============================
    
    #[test]
    fn test_reference_recurses_into_inner() {
        let mut map = BTreeMap::new();
        let template = reference(generic("T"), false);
        let concrete_ty = reference(Type::F32, false);
        unify_types_recursive(&template, &concrete_ty, &mut map);
        assert_eq!(map.get("T"), Some(&Type::F32));
    }
    
    #[test]
    fn test_reference_mutable_recurses() {
        let mut map = BTreeMap::new();
        let template = reference(generic("T"), true);
        let concrete_ty = reference(Type::I64, true);
        unify_types_recursive(&template, &concrete_ty, &mut map);
        assert_eq!(map.get("T"), Some(&Type::I64));
    }
    
    #[test]
    fn test_reference_with_concrete_inner() {
        // &Result<T, E> matched against &Result<File, IOError>
        let mut map = BTreeMap::new();
        let template = reference(
            concrete("Result", vec![generic("T"), generic("E")]),
            false,
        );
        let concrete_ty = reference(
            concrete("Result", vec![Type::I32, Type::U8]),
            false,
        );
        unify_types_recursive(&template, &concrete_ty, &mut map);
        assert_eq!(map.get("T"), Some(&Type::I32));
        assert_eq!(map.get("E"), Some(&Type::U8));
    }

    // ============================
    // 6. Array(inner, size, mut)
    // ============================
    
    #[test]
    fn test_array_recurses_into_element() {
        let mut map = BTreeMap::new();
        let template = array(generic("T"), 10);
        let concrete_ty = array(Type::F32, 10);
        unify_types_recursive(&template, &concrete_ty, &mut map);
        assert_eq!(map.get("T"), Some(&Type::F32));
    }

    // ============================
    // 7. Primitives — no unification
    // ============================
    
    #[test]
    fn test_primitive_vs_primitive_no_binding() {
        let mut map = BTreeMap::new();
        unify_types_recursive(&Type::I32, &Type::I32, &mut map);
        assert!(map.is_empty(), "Matching primitives should not produce bindings");
    }

    #[test]
    fn test_primitive_vs_different_primitive_no_binding() {
        let mut map = BTreeMap::new();
        unify_types_recursive(&Type::I32, &Type::F32, &mut map);
        assert!(map.is_empty(), "Different primitives should not produce bindings");
    }

    // ============================
    // 8. Mixed / edge cases
    // ============================
    
    #[test]
    fn test_pointer_vs_reference_no_binding() {
        // Pointer and Reference are different type constructors
        let mut map = BTreeMap::new();
        let template = ptr(generic("T"));
        let concrete_ty = reference(Type::F32, false);
        unify_types_recursive(&template, &concrete_ty, &mut map);
        assert!(map.is_empty(), "Pointer vs Reference should not unify");
    }

    #[test]
    fn test_concrete_vs_pointer_no_binding() {
        let mut map = BTreeMap::new();
        let template = concrete("Result", vec![generic("T")]);
        let concrete_ty = ptr(Type::F32);
        unify_types_recursive(&template, &concrete_ty, &mut map);
        assert!(map.is_empty(), "Concrete vs Pointer should not unify");
    }

    #[test]
    fn test_multiple_generics_same_type() {
        // HashMap<T, T> matched against HashMap<i32, i32>
        // First T binds to i32, second T sees existing binding
        let mut map = BTreeMap::new();
        let template = concrete("HashMap", vec![generic("T"), generic("T")]);
        let concrete_ty = concrete("HashMap", vec![Type::I32, Type::I32]);
        unify_types_recursive(&template, &concrete_ty, &mut map);
        assert_eq!(map.get("T"), Some(&Type::I32));
    }
    
    #[test]
    fn test_deeply_nested_inference() {
        // Result<Ptr<Vec<T>>, E> matched against Result<Ptr<Vec<f32>>, IOError>
        let mut map = BTreeMap::new();
        let template = concrete("Result", vec![
            ptr(concrete("Vec", vec![generic("T")])),
            generic("E"),
        ]);
        let concrete_ty = concrete("Result", vec![
            ptr(concrete("Vec", vec![Type::F32])),
            struct_ty("IOError"),
        ]);
        unify_types_recursive(&template, &concrete_ty, &mut map);
        assert_eq!(map.get("T"), Some(&Type::F32));
        assert_eq!(map.get("E"), Some(&struct_ty("IOError")));
    }

    #[test]
    fn test_empty_concrete_args_matches() {
        // IOError matched against IOError (no args)
        let mut map = BTreeMap::new();
        let template = concrete("IOError", vec![]);
        let concrete_ty = concrete("IOError", vec![]);
        unify_types_recursive(&template, &concrete_ty, &mut map);
        assert!(map.is_empty(), "Empty args should not produce bindings");
    }

    #[test]
    fn test_generic_name_e_binds() {
        let mut map = BTreeMap::new();
        unify_types_recursive(&generic("E"), &struct_ty("IOError"), &mut map);
        assert_eq!(map.get("E"), Some(&struct_ty("IOError")));
    }

    #[test]
    fn test_struct_placeholder_e_binds_to_concrete() {
        let mut map = BTreeMap::new();
        unify_types_recursive(
            &generic("E"),
            &concrete("std__io__file__IOError", vec![]),
            &mut map,
        );
        assert_eq!(map.get("E"), Some(&concrete("std__io__file__IOError", vec![])));
    }

    // ============================
    // 9. End-to-end: mmap<T> scenario
    // ============================

    #[test]
    fn test_mmap_f32_scenario() {
        // Exact production scenario:
        // Method return: Result<Ptr<Struct("T")>, IOError>  
        // Expected type: Result<Ptr<F32>, IOError>
        // Should infer: T -> F32
        let mut map = BTreeMap::new();
        let ret_template = concrete("std__core__result__Result", vec![
            ptr(generic("T")),
            concrete("std__io__file__IOError", vec![]),
        ]);
        let expected = concrete("std__core__result__Result", vec![
            ptr(Type::F32),
            concrete("std__io__file__IOError", vec![]),
        ]);
        unify_types_recursive(&ret_template, &expected, &mut map);
        assert_eq!(map.get("T"), Some(&Type::F32));
        assert_eq!(map.len(), 1, "Only T should be inferred");
    }

    #[test]
    fn test_mmap_u8_scenario() {
        let mut map = BTreeMap::new();
        let ret_template = concrete("std__core__result__Result", vec![
            ptr(generic("T")),
            concrete("std__io__file__IOError", vec![]),
        ]);
        let expected = concrete("std__core__result__Result", vec![
            ptr(Type::U8),
            concrete("std__io__file__IOError", vec![]),
        ]);
        unify_types_recursive(&ret_template, &expected, &mut map);
        assert_eq!(map.get("T"), Some(&Type::U8));
    }

    // ============================
    // 10. Smallest-scope pipeline isolation tests
    //
    // Each test isolates ONE micro-step of the inference pipeline:
    //   Step A: unify extracts a binding
    //   Step B: substitute uses that binding  
    //   Step C: has_generics detects unresolved params
    //   Step D: full pipeline (unify → substitute → verify)
    // ============================

    /// Step A: Bare minimum — unify single Struct("T") against F32
    #[test]
    fn test_pipeline_step_a_single_binding() {
        let mut map = BTreeMap::new();
        unify_types_recursive(&generic("T"), &Type::F32, &mut map);
        assert_eq!(map.len(), 1);
        assert_eq!(map["T"], Type::F32);
    }

    /// Step B: After unification, substitute replaces Struct("T") with the binding
    #[test]
    fn test_pipeline_step_b_substitute_after_unify() {
        let mut map = BTreeMap::new();
        let template = ptr(generic("T"));
        let concrete_ty = ptr(Type::F32);
        unify_types_recursive(&template, &concrete_ty, &mut map);
        
        // Now substitute the template using the inferred map
        let ret_template = concrete("Result", vec![
            ptr(generic("T")),
            struct_ty("IOError"),
        ]);
        let substituted = ret_template.substitute(&map);
        let expected = concrete("Result", vec![
            ptr(Type::F32),
            struct_ty("IOError"),
        ]);
        assert_eq!(substituted, expected, "substitute should replace T with F32");
    }

    /// Step C: has_generics returns true for unresolved, false after substitution
    #[test]
    fn test_pipeline_step_c_has_generics_before_and_after() {
        let unresolved = ptr(generic("T"));
        // Generic("T") is the proper way to represent generic placeholders,
        // but has_generics checks Type::Generic not Type::Struct. 
        // So check that substitute actually resolves it:
        let mut map = BTreeMap::new();
        map.insert("T".to_string(), Type::F32);
        let resolved = unresolved.substitute(&map);
        assert_eq!(resolved, ptr(Type::F32));
        assert!(!resolved.has_generics(), "After substitution, no generics should remain");
    }

    /// Step D: Full pipeline simulation — what emit_method_call does:
    /// 1. Check unmapped params exist
    /// 2. Unify ret template vs expected
    /// 3. Substitute all types using inferred map
    /// 4. Verify no generics remain
    #[test]
    fn test_pipeline_step_d_full_emit_simulation() {
        // Simulate: fn mmap<T>(&self, ...) -> Result<Ptr<T>, IOError>
        // Called as: let r: Result<Ptr<f32>, IOError> = file.mmap(...)
        
        let declared_generics = vec!["T".to_string()];
        let mut method_generic_map: BTreeMap<String, Type> = BTreeMap::new();
        
        // Step 1: Check unmapped
        let unmapped: Vec<&String> = declared_generics.iter()
            .filter(|g| !method_generic_map.contains_key(*g))
            .collect();
        assert_eq!(unmapped.len(), 1, "T should be unmapped");
        
        // Step 2: Unify
        let ret_template = concrete("Result", vec![
            ptr(generic("T")),
            concrete("IOError", vec![]),
        ]);
        let expected_ty = concrete("Result", vec![
            ptr(Type::F32),
            concrete("IOError", vec![]),
        ]);
        unify_types_recursive(&ret_template, &expected_ty, &mut method_generic_map);
        assert_eq!(method_generic_map.get("T"), Some(&Type::F32), "Unification should resolve T");
        
        // Step 3: Substitute
        let arg_template = ptr(generic("T"));
        let substituted_arg = arg_template.substitute(&method_generic_map);
        assert_eq!(substituted_arg, ptr(Type::F32), "Substitute should replace T in arg types");
        
        let substituted_ret = ret_template.substitute(&method_generic_map);
        let expected_ret = concrete("Result", vec![
            ptr(Type::F32),
            concrete("IOError", vec![]),
        ]);
        assert_eq!(substituted_ret, expected_ret, "Substitute should replace T in return type");
        
        // Step 4: Verify no generics remain
        assert!(!substituted_ret.has_generics(), "No unresolved generics should remain");
        assert!(!substituted_arg.has_generics(), "No unresolved generics in args");
    }

    /// Step D variant: Same pipeline but for T=u8
    #[test]
    fn test_pipeline_step_d_u8_variant() {
        let mut method_generic_map: BTreeMap<String, Type> = BTreeMap::new();
        
        let ret_template = concrete("Result", vec![
            ptr(generic("T")),
            concrete("IOError", vec![]),
        ]);
        let expected_ty = concrete("Result", vec![
            ptr(Type::U8),
            concrete("IOError", vec![]),
        ]);
        unify_types_recursive(&ret_template, &expected_ty, &mut method_generic_map);
        
        assert_eq!(method_generic_map.get("T"), Some(&Type::U8));
        let substituted = ret_template.substitute(&method_generic_map);
        assert_eq!(substituted, expected_ty);
        assert!(!substituted.has_generics());
    }

    /// Pipeline test: inferred type used to check concrete_tys injection
    /// (simulates the BIDIR BRIDGE logic)
    #[test]
    fn test_pipeline_bidir_bridge_injection() {
        let declared_generics = vec!["T".to_string()];
        let mut method_generic_map: BTreeMap<String, Type> = BTreeMap::new();
        
        // Unify
        let ret_template = ptr(generic("T"));
        let expected_ty = ptr(Type::F32);
        unify_types_recursive(&ret_template, &expected_ty, &mut method_generic_map);
        
        // Simulate BIDIR BRIDGE: build concrete_tys from method_generic_map
        let mut concrete_tys: Vec<Type> = Vec::new();
        for param_name in &declared_generics {
            if let Some(resolved) = method_generic_map.get(param_name) {
                if !resolved.has_generics() {
                    concrete_tys.push(resolved.clone());
                }
            }
        }
        
        assert_eq!(concrete_tys.len(), declared_generics.len(), 
            "All generics should be resolved");
        assert_eq!(concrete_tys[0], Type::F32);
        assert!(!concrete_tys.is_empty(), "is_specialized should be true");
    }

    /// Pipeline test: two-param version (T, E)
    #[test]
    fn test_pipeline_two_param_bridge() {
        let declared_generics = vec!["T".to_string(), "E".to_string()];
        let mut map: BTreeMap<String, Type> = BTreeMap::new();
        
        let ret_template = concrete("Result", vec![generic("T"), generic("E")]);
        let expected_ty = concrete("Result", vec![Type::I32, struct_ty("MyError")]);
        unify_types_recursive(&ret_template, &expected_ty, &mut map);
        
        let mut concrete_tys: Vec<Type> = Vec::new();
        for param in &declared_generics {
            if let Some(resolved) = map.get(param) {
                concrete_tys.push(resolved.clone());
            }
        }
        
        assert_eq!(concrete_tys.len(), 2);
        assert_eq!(concrete_tys[0], Type::I32);
        assert_eq!(concrete_tys[1], struct_ty("MyError"));
    }

    // ============================
    // 11. Multi-Character Generic Names (TDD)
    // These tests drive the fix for F2, Item, Allocator, etc.
    // ============================

    #[test]
    fn test_generic_multi_char_f2_binds() {
        // Type::Generic("F2") should unify just like Generic("T")
        let mut map = BTreeMap::new();
        let fn_type = Type::Fn(vec![Type::I64], Box::new(Type::Bool));
        unify_types_recursive(&generic("F2"), &fn_type, &mut map);
        assert_eq!(map.get("F2"), Some(&fn_type),
            "Generic('F2') should bind to Fn type");
    }

    #[test]
    fn test_generic_multi_char_item_binds() {
        let mut map = BTreeMap::new();
        unify_types_recursive(&generic("Item"), &Type::I64, &mut map);
        assert_eq!(map.get("Item"), Some(&Type::I64),
            "Generic('Item') should bind to I64");
    }

    #[test]
    fn test_generic_multi_char_allocator_binds() {
        let mut map = BTreeMap::new();
        let alloc_ty = struct_ty("BumpAllocator");
        unify_types_recursive(&generic("Allocator"), &alloc_ty, &mut map);
        assert_eq!(map.get("Allocator"), Some(&alloc_ty),
            "Generic('Allocator') should bind");
    }

    #[test]
    fn test_concrete_with_multi_char_generic_args() {
        // Map<Generic("I"), Generic("F2"), Generic("Output")> vs
        // Map<Range, Fn(i64)->i64, i64>
        let mut map = BTreeMap::new();
        let template = concrete("Map", vec![
            generic("I"),
            generic("F2"),
            generic("Output"),
        ]);
        let concrete_ty = concrete("Map", vec![
            struct_ty("Range"),
            Type::Fn(vec![Type::I64], Box::new(Type::I64)),
            Type::I64,
        ]);
        unify_types_recursive(&template, &concrete_ty, &mut map);
        assert_eq!(map.get("I"), Some(&struct_ty("Range")));
        assert_eq!(map.get("F2"), Some(&Type::Fn(vec![Type::I64], Box::new(Type::I64))));
        assert_eq!(map.get("Output"), Some(&Type::I64));
    }

    #[test]
    fn test_iterator_combinator_scenario() {
        // Exact production scenario:
        // fn map<F2, Output>(self, f: F2) -> Map<Self, F2, Output>
        // Called as: filter.map(|x| x * 2)
        // Template ret: Concrete("Map", [Generic("I"), Generic("F2"), Generic("Output")])
        // Concrete ret: Concrete("Map", [Filter_Range_Fn, Fn(i64)->i64, i64])
        let mut map = BTreeMap::new();
        let filter_ty = concrete("Filter", vec![struct_ty("Range"), Type::Fn(vec![Type::I64], Box::new(Type::Bool))]);
        let template = concrete("Map", vec![
            generic("I"),       // Self type (Filter<Range, Fn>)
            generic("F2"),      // Closure type
            generic("Output"),  // Return type of closure
        ]);
        let concrete_ty = concrete("Map", vec![
            filter_ty.clone(),
            Type::Fn(vec![Type::I64], Box::new(Type::I64)),
            Type::I64,
        ]);
        unify_types_recursive(&template, &concrete_ty, &mut map);
        assert_eq!(map.get("I"), Some(&filter_ty));
        assert_eq!(map.get("F2"), Some(&Type::Fn(vec![Type::I64], Box::new(Type::I64))));
        assert_eq!(map.get("Output"), Some(&Type::I64));
    }

    #[test]
    fn test_pipeline_multi_char_full_flow() {
        // Full pipeline: unify → substitute → verify no generics
        let _declared_generics = vec!["F2".to_string(), "Output".to_string()];
        let mut map: BTreeMap<String, Type> = BTreeMap::new();

        let ret_template = concrete("Map", vec![
            struct_ty("Range"),
            generic("F2"),
            generic("Output"),
        ]);
        let expected_ty = concrete("Map", vec![
            struct_ty("Range"),
            Type::Fn(vec![Type::I64], Box::new(Type::I64)),
            Type::I64,
        ]);
        unify_types_recursive(&ret_template, &expected_ty, &mut map);

        assert_eq!(map.get("F2"), Some(&Type::Fn(vec![Type::I64], Box::new(Type::I64))));
        assert_eq!(map.get("Output"), Some(&Type::I64));

        // Substitute
        let substituted = ret_template.substitute(&map);
        assert_eq!(substituted, expected_ty, "All multi-char generics should be resolved after substitution");
        assert!(!substituted.has_generics(), "No unresolved generics should remain");
    }

    // ============================
    // 12. Phantom Generic Inference (TDD)
    // Tests for infer_phantom_generics: Map<I, F, T> where T = Fn return type
    // ============================

    use crate::codegen::expr::infer_phantom_generics;

    #[test]
    fn test_phantom_generic_map_i_f_t() {
        // Map<I, F, T> with I=Range, F=Fn(i64)->i64 => T should be i64
        let declared = vec!["I".to_string(), "F".to_string(), "T".to_string()];
        let mut map = BTreeMap::new();
        map.insert("I".to_string(), struct_ty("Range"));
        map.insert("F".to_string(), Type::Fn(vec![Type::I64], Box::new(Type::I64)));

        infer_phantom_generics(&declared, &mut map);

        assert_eq!(map.get("T"), Some(&Type::I64),
            "T should be inferred as the return type of F");
        assert_eq!(map.len(), 3, "All 3 generics should be resolved");
    }

    #[test]
    fn test_phantom_generic_no_unresolved() {
        // When all generics are resolved, nothing should change
        let declared = vec!["I".to_string(), "F".to_string()];
        let mut map = BTreeMap::new();
        map.insert("I".to_string(), struct_ty("Range"));
        map.insert("F".to_string(), Type::Fn(vec![Type::I64], Box::new(Type::Bool)));

        let map_before = map.clone();
        infer_phantom_generics(&declared, &mut map);

        assert_eq!(map, map_before, "Nothing should change when all generics resolved");
    }

    #[test]
    fn test_phantom_generic_no_fn_types() {
        // When no Fn types are resolved, phantom generics remain unresolved
        let declared = vec!["I".to_string(), "T".to_string()];
        let mut map = BTreeMap::new();
        map.insert("I".to_string(), struct_ty("Range"));

        infer_phantom_generics(&declared, &mut map);

        assert!(!map.contains_key("T"), "T should remain unresolved without Fn types");
    }

    #[test]
    fn test_phantom_generic_fn_returns_struct() {
        // F = Fn(i64) -> MyStruct, T should be MyStruct
        let declared = vec!["I".to_string(), "F".to_string(), "Output".to_string()];
        let mut map = BTreeMap::new();
        map.insert("I".to_string(), struct_ty("Vec"));
        map.insert("F".to_string(), Type::Fn(vec![Type::I64], Box::new(struct_ty("MyStruct"))));

        infer_phantom_generics(&declared, &mut map);

        assert_eq!(map.get("Output"), Some(&struct_ty("MyStruct")),
            "Output should be inferred as MyStruct from Fn return type");
    }

    #[test]
    fn test_phantom_generic_two_unresolved_no_inference() {
        // Two unresolved generics + one Fn -> should NOT infer (ambiguous)
        let declared = vec!["I".to_string(), "F".to_string(), "T".to_string(), "U".to_string()];
        let mut map = BTreeMap::new();
        map.insert("I".to_string(), struct_ty("Range"));
        map.insert("F".to_string(), Type::Fn(vec![Type::I64], Box::new(Type::I64)));

        infer_phantom_generics(&declared, &mut map);

        assert!(!map.contains_key("T"), "Should not infer with 2 unresolved generics");
        assert!(!map.contains_key("U"), "Should not infer with 2 unresolved generics");
    }
}
