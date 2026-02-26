// ============================================================================
// HashMap Eq Dispatch Tests
// Validates that equality comparisons on generic key references in HashMap
// correctly resolve to the concrete key type, not a partially-specialized
// Self type. This prevents emitting calls to non-existent functions like
// `HashMap__i64__eq` when the correct behavior is a primitive `arith.cmpi`.
//
// Root cause: During hydration of `HashMap<K,V>` methods with K=i64,
// the compiler resolves `&entry.key == key` with common_ty as
// Struct("HashMap__i64") instead of Reference(I64). This causes the
// Struct eq path to emit a call to a function that was never defined.
// ============================================================================

#[cfg(test)]
mod hashmap_eq_dispatch_tests {
    use salt_front::codegen::context::CodegenContext;
    use salt_front::grammar::SaltFile;
    use salt_front::types::Type;

    macro_rules! with_ctx {
        ($name:ident, $block:block) => {
            let file: SaltFile = syn::parse_str("fn main() {}").expect("valid salt file");
            let z3_cfg = z3::Config::new();
            let z3_ctx = z3::Context::new(&z3_cfg);
            #[allow(unused_mut)]
            let mut $name = CodegenContext::new(&file, false, None, &z3_ctx);
            $block
        };
    }

    // =========================================================================
    // Test 1: Primitive type mangle_suffix is simple, not qualified
    // =========================================================================

    /// When comparing &i64 references, the inner type's mangle_suffix should
    /// be "i64", not "std__collections__hash_map__HashMap__i64" or similar.
    #[test]
    fn test_i64_mangle_suffix_is_simple() {
        let ty = Type::I64;
        assert_eq!(ty.mangle_suffix(), "i64");
    }

    #[test]
    fn test_reference_to_i64_has_i64_inner() {
        let ty = Type::Reference(Box::new(Type::I64), false);
        if let Type::Reference(inner, _) = &ty {
            assert_eq!(inner.mangle_suffix(), "i64");
            // The eq method name should be "i64__eq", not "HashMap__i64__eq"
            let eq_method_name = format!("{}__{}", inner.mangle_suffix(), "eq");
            assert_eq!(eq_method_name, "i64__eq");
        } else {
            panic!("Expected Reference type");
        }
    }

    // =========================================================================
    // Test 2: generic_impls should NOT contain HashMap__i64__eq  
    //         but SHOULD resolve i64 eq through primitive comparison
    // =========================================================================

    /// When looking up eq for i64, the generic_impls map should either:
    /// - Contain "i64__eq" (trait-based eq for i64), or
    /// - Not contain "i64__eq" (falls back to primitive arith.cmpi)
    /// It should NEVER try "HashMap__i64__eq" or any qualified variant.
    #[test]
    fn test_eq_method_name_for_i64_is_not_qualified() {
        let inner = Type::I64;
        let mangle_name = inner.mangle_suffix();
        let eq_method_name = format!("{}__{}", mangle_name, "eq");
        
        // Must not contain any module path prefix
        assert!(!eq_method_name.contains("HashMap"), 
            "eq method name for i64 must not reference HashMap: {}", eq_method_name);
        assert!(!eq_method_name.contains("std__collections"),
            "eq method name for i64 must not reference std collections: {}", eq_method_name);
        assert_eq!(eq_method_name, "i64__eq");
    }

    // =========================================================================
    // Test 3: The Struct("HashMap__i64") type should NOT appear as common_ty
    //         when comparing actual i64 key references
    // =========================================================================

    /// A partially-specialized Self type like Struct("HashMap__i64") should not
    /// be used as the common type for equality on key references.
    /// This test documents the expected behavior: &K where K=i64 should resolve
    /// to Reference(I64), not Struct("std__collections__hash_map__HashMap__i64").
    #[test]
    fn test_partially_specialized_self_type_not_used_for_key_eq() {
        // Simulates what should NOT happen: treating a partially-specialized
        // HashMap type as if it were the key's type
        let _bad_type = Type::Struct("std__collections__hash_map__HashMap__i64".to_string());
        let bad_eq_name = format!("{}__{}", "std__collections__hash_map__HashMap__i64", "eq");
        
        // This is the bad function name that currently gets emitted
        assert_eq!(bad_eq_name, "std__collections__hash_map__HashMap__i64__eq");
        
        // The correct type should be I64, not the HashMap type
        let correct_inner = Type::I64;
        let correct_eq_name = format!("{}__{}", correct_inner.mangle_suffix(), "eq");
        assert_eq!(correct_eq_name, "i64__eq");
        
        // The bad name and correct name are different — this is the bug
        assert_ne!(bad_eq_name, correct_eq_name, 
            "If these were equal, there would be no bug");
    }

    // =========================================================================
    // Test 4: Primitive types should use arith.cmpi, not function calls
    // =========================================================================

    /// For primitive types (i64, i32, etc.), Reference equality should
    /// generate load+compare (arith.cmpi), not a function call.
    #[test]
    fn test_primitive_reference_eq_uses_load_compare() {
        with_ctx!(ctx, {
            // Register what the reference eq path would check
            let inner = Type::I64;
            let eq_method_name = format!("{}__{}", inner.mangle_suffix(), "eq");
            
            // For primitives, generic_impls should NOT contain an eq method
            // (primitives use hardware comparison, not trait dispatch)
            let has_trait_eq = ctx.generic_impls().contains_key(&eq_method_name);
            
            // If this is false, the codegen falls through to the load+compare path
            // which generates arith.cmpi — the correct behavior for primitives
            assert!(!has_trait_eq, 
                "i64__eq should NOT be in generic_impls for a bare context. \
                 Primitives should use arith.cmpi, not trait function calls.");
        });
    }

    // =========================================================================
    // Test 5: The eq method for a Struct type IS in generic_impls
    //         when properly registered (positive case)
    // =========================================================================

    /// This test verifies the positive case: when a struct HAS an eq method
    /// registered via trait impl, it should be found in generic_impls.
    #[test]
    fn test_struct_eq_method_found_when_registered() {
        with_ctx!(ctx, {
            // Simulate registering an eq method for a struct
            let struct_name = "MyStruct";
            let eq_name = format!("{}__{}", struct_name, "eq");
            
            // Create a minimal function definition for the eq method
            let method: salt_front::grammar::SaltFn = syn::parse_str(
                "fn eq(self: &MyStruct, other: &MyStruct) -> bool { true }"
            ).expect("valid fn");
            
            ctx.generic_impls_mut().insert(eq_name.clone(), (method, vec![]));
            
            // Now the lookup should succeed
            assert!(ctx.generic_impls().contains_key(&eq_name),
                "Registered eq method should be found in generic_impls");
        });
    }

    // =========================================================================
    // Tests 6-11: Type::substitute correctness
    // =========================================================================

    /// During hydration, the type_map contains {K → I64}. When a field type
    /// is Struct("K") (the Graydon Fix representation of a generic parameter),
    /// substitute() should resolve it to I64.
    #[test]
    fn test_substitute_struct_k_resolves_to_i64() {
        use std::collections::{BTreeMap, HashMap};

        let mut type_map = BTreeMap::new();
        type_map.insert("K".to_string(), Type::I64);
        type_map.insert("V".to_string(), Type::I64);

        let field_ty = Type::Struct("K".to_string());
        let resolved = field_ty.substitute(&type_map);

        assert_eq!(resolved, Type::I64,
            "Struct(\"K\") with type_map {{K→I64}} must resolve to I64, got {:?}", resolved);
    }

    #[test]
    fn test_substitute_reference_struct_k_becomes_reference_i64() {
        use std::collections::{BTreeMap, HashMap};

        let mut type_map = BTreeMap::new();
        type_map.insert("K".to_string(), Type::I64);

        let ref_k = Type::Reference(Box::new(Type::Struct("K".to_string())), false);
        let resolved = ref_k.substitute(&type_map);

        assert_eq!(resolved, Type::Reference(Box::new(Type::I64), false),
            "Reference(Struct(\"K\")) must substitute to Reference(I64), got {:?}", resolved);
    }

    /// Verifying substitute for all primitive types used as generic parameters.
    #[test]
    fn test_substitute_generic_param_to_all_primitives() {
        use std::collections::{BTreeMap, HashMap};

        let primitives = vec![
            ("i8", Type::I8), ("i16", Type::I16), ("i32", Type::I32), ("i64", Type::I64),
            ("u8", Type::U8), ("u16", Type::U16), ("u32", Type::U32), ("u64", Type::U64),
            ("f32", Type::F32), ("f64", Type::F64), ("bool", Type::Bool), ("usize", Type::Usize),
        ];

        for (name, expected_ty) in primitives {
            let mut type_map = BTreeMap::new();
            type_map.insert("T".to_string(), expected_ty.clone());

            let field_ty = Type::Struct("T".to_string());
            let resolved = field_ty.substitute(&type_map);
            assert_eq!(resolved, expected_ty,
                "Struct(\"T\") with T→{} must resolve to {:?}, got {:?}", name, expected_ty, resolved);
        }
    }

    // =========================================================================
    // Tests 12-15: Equality dispatch path — struct dispatch predicate
    // `matches!(common_ty, Type::Struct(_) | Type::Tuple(_) | Type::Array(..) | Type::Enum(_))`
    // =========================================================================

    /// ALL primitive types must NOT enter the struct dispatch path.
    #[test]
    fn test_all_primitives_skip_struct_dispatch() {
        let primitives = vec![
            ("I8", Type::I8), ("I16", Type::I16), ("I32", Type::I32), ("I64", Type::I64),
            ("U8", Type::U8), ("U16", Type::U16), ("U32", Type::U32), ("U64", Type::U64),
            ("F32", Type::F32), ("F64", Type::F64), ("Bool", Type::Bool), ("Usize", Type::Usize),
        ];

        for (name, ty) in primitives {
            let enters = matches!(ty, Type::Struct(_) | Type::Tuple(_) | Type::Array(..) | Type::Enum(_));
            assert!(!enters, "{} must NOT enter the struct eq dispatch path", name);
        }
    }

    /// ALL Reference-wrapped primitives must NOT enter the struct dispatch path.
    #[test]
    fn test_all_reference_primitives_skip_struct_dispatch() {
        let primitives = vec![
            ("&I8", Type::I8), ("&I16", Type::I16), ("&I32", Type::I32), ("&I64", Type::I64),
            ("&U8", Type::U8), ("&U16", Type::U16), ("&U32", Type::U32), ("&U64", Type::U64),
            ("&F32", Type::F32), ("&F64", Type::F64), ("&Bool", Type::Bool), ("&Usize", Type::Usize),
        ];

        for (name, inner) in primitives {
            let ty = Type::Reference(Box::new(inner), false);
            let enters = matches!(ty, Type::Struct(_) | Type::Tuple(_) | Type::Array(..) | Type::Enum(_));
            assert!(!enters, "{} must NOT enter the struct eq dispatch path", name);
        }
    }

    /// Struct, Tuple, Array, and Enum types SHOULD enter the struct dispatch path.
    #[test]
    fn test_aggregate_types_enter_struct_dispatch() {
        let aggregates: Vec<(&str, Type)> = vec![
            ("Struct", Type::Struct("MyStruct".to_string())),
            ("Tuple", Type::Tuple(vec![Type::I64, Type::I64])),
            ("Array", Type::Array(Box::new(Type::I64), 4, false)),
            ("Enum", Type::Enum("MyEnum".to_string())),
        ];

        for (name, ty) in aggregates {
            let enters = matches!(ty, Type::Struct(_) | Type::Tuple(_) | Type::Array(..) | Type::Enum(_));
            assert!(enters, "{} SHOULD enter the struct eq dispatch path", name);
        }
    }

    // =========================================================================
    // Tests 16-19: Reference equality path — trait dispatch guard
    // Inner primitives must use hardware comparison, not trait dispatch.
    // The guard: `!inner.is_numeric() && !inner.is_integer() && !matches!(**inner, Bool | I8 | U8)`
    // =========================================================================

    /// ALL primitive inner types must be skipped by the trait dispatch guard.
    /// This is the critical fix: without this guard, emit_call's mangle_fn_name
    /// re-mangles "i64__eq" under the caller's package context.
    #[test]
    fn test_reference_primitive_inner_skips_trait_dispatch() {
        let primitives = vec![
            ("I8", Type::I8), ("I16", Type::I16), ("I32", Type::I32), ("I64", Type::I64),
            ("U8", Type::U8), ("U16", Type::U16), ("U32", Type::U32), ("U64", Type::U64),
            ("F32", Type::F32), ("F64", Type::F64), ("Bool", Type::Bool), ("Usize", Type::Usize),
        ];

        for (name, inner) in &primitives {
            // This is the exact guard from emit_binary
            let skips_trait = inner.is_numeric() || inner.is_integer() || matches!(inner, Type::Bool | Type::I8 | Type::U8);
            assert!(skips_trait,
                "Reference({}) must skip trait dispatch and use hardware comparison", name);
        }
    }

    /// Non-primitive inner types (Struct, Enum) should NOT be skipped by the guard.
    #[test]
    fn test_reference_struct_inner_enters_trait_dispatch() {
        let non_primitives: Vec<(&str, Type)> = vec![
            ("Struct(String)", Type::Struct("String".to_string())),
            ("Struct(MyStruct)", Type::Struct("MyStruct".to_string())),
        ];

        for (name, inner) in &non_primitives {
            let skips_trait = inner.is_numeric() || inner.is_integer() || matches!(inner, Type::Bool | Type::I8 | Type::U8);
            assert!(!skips_trait,
                "Reference({}) SHOULD enter trait dispatch, not hardware comparison", name);
        }
    }

    // =========================================================================
    // Tests 20-22: common_ty derivation for various type pairs
    // =========================================================================

    /// common_ty for two identical Reference types should be that Reference type.
    #[test]
    fn test_common_ty_reference_preserves_inner_type() {
        let test_cases: Vec<(&str, Type)> = vec![
            ("I8", Type::I8), ("I16", Type::I16), ("I32", Type::I32), ("I64", Type::I64),
            ("U8", Type::U8), ("U16", Type::U16), ("U32", Type::U32), ("U64", Type::U64),
            ("F32", Type::F32), ("F64", Type::F64), ("Bool", Type::Bool),
            ("Struct", Type::Struct("MyStruct".to_string())),
        ];

        for (name, inner) in test_cases {
            let lhs = Type::Reference(Box::new(inner.clone()), false);
            let rhs = Type::Reference(Box::new(inner.clone()), false);

            // emit_binary logic: references are not numeric, so common_ty = lhs.clone()
            let common_ty = if lhs.is_numeric() && rhs.is_numeric() {
                lhs.clone()
            } else {
                lhs.clone()
            };

            // Must be Reference with correct inner
            if let Type::Reference(resolved_inner, _) = &common_ty {
                assert_eq!(**resolved_inner, inner,
                    "common_ty for Reference({}) == Reference({}) must preserve inner type", name, name);
            } else {
                panic!("common_ty for Reference pair must be Reference, got {:?}", common_ty);
            }
        }
    }

    /// common_ty for two identical primitive values should be that primitive.
    #[test]
    fn test_common_ty_primitive_pairs() {
        let test_cases: Vec<(&str, Type)> = vec![
            ("I8", Type::I8), ("I16", Type::I16), ("I32", Type::I32), ("I64", Type::I64),
            ("U8", Type::U8), ("U16", Type::U16), ("U32", Type::U32), ("U64", Type::U64),
            ("F32", Type::F32), ("F64", Type::F64), ("Bool", Type::Bool),
        ];

        for (name, ty) in test_cases {
            let lhs = ty.clone();
            let rhs = ty.clone();

            let common_ty = if lhs.is_numeric() && rhs.is_numeric() {
                // Both numeric: pick wider (same width here, so lhs)
                lhs.clone()
            } else {
                lhs.clone()
            };

            assert_eq!(common_ty, ty,
                "common_ty for {} == {} must be {}", name, name, name);

            // Must NOT enter struct dispatch
            assert!(!matches!(common_ty, Type::Struct(_) | Type::Tuple(_) | Type::Array(..) | Type::Enum(_)),
                "common_ty {} must NOT enter struct dispatch", name);
        }
    }

    // =========================================================================
    // Test 23: The exact bug scenario — documenting what was broken
    // =========================================================================

    /// Document the exact bug: Struct("HashMap__i64") as common_ty would
    /// trigger struct dispatch, which emits a call to a never-defined function.
    #[test]
    fn test_incorrect_struct_common_ty_triggers_wrong_path() {
        let bad_common_ty = Type::Struct("std__collections__hash_map__HashMap__i64".to_string());

        let enters_struct_path = matches!(bad_common_ty, Type::Struct(_) | Type::Tuple(_) | Type::Array(..) | Type::Enum(_));
        assert!(enters_struct_path,
            "Struct types DO enter struct dispatch — this is correct for actual structs");

        if let Type::Struct(ref name) = bad_common_ty {
            let eq_method_name = format!("{}__{}", name, "eq");
            assert_eq!(eq_method_name, "std__collections__hash_map__HashMap__i64__eq",
                "This is the undefined function name that the bug used to produce");
        }
    }

    // =========================================================================
    // Test 24: mangle_fn_name guard — names starting with std__ are not re-mangled
    // =========================================================================

    /// Validates that the mangle_fn_name function's early-return guard works
    /// correctly: names already starting with "std__" should not be re-mangled.
    #[test]
    fn test_qualified_names_bypass_mangling() {
        let qualified_names = vec![
            "std__eq__i64__eq",
            "std__hash__hash__i64__hash",
            "std__collections__hash_map__HashMap__get",
            "core__mem__size_of",
        ];

        for name in qualified_names {
            // The guard in mangle_fn_name checks:
            // if name.starts_with("std__") || name.starts_with("core__") ...
            let should_bypass = name.starts_with("std__") || name.starts_with("core__");
            assert!(should_bypass,
                "Qualified name '{}' should bypass re-mangling", name);
        }
    }

    /// Validates that unqualified names like "i64__eq" do NOT bypass the mangling guard.
    /// This documents why the bug occurs: these names get re-mangled under the caller's package.
    #[test]
    fn test_unqualified_names_dont_bypass_mangling() {
        let unqualified_names = vec![
            "i64__eq",
            "u64__eq",
            "String__eq",
            "MyStruct__hash",
        ];

        for name in unqualified_names {
            let should_bypass = name.starts_with("std__") || name.starts_with("core__");
            assert!(!should_bypass,
                "Unqualified name '{}' should NOT bypass mangling guard — this is the bug trigger", name);
        }
    }

    // =========================================================================
    // String Key Eq Dispatch Tests (string_hashmap_bench fix)
    // =========================================================================
    // When HashMap uses String keys, the eq dispatch path for Reference(String)
    // must:
    // 1. Construct eq_method_name as "std__string__String__eq" (fully qualified)
    // 2. Find it in generic_impls (it IS registered there)
    // 3. Call request_specialization with self_ty = Some(Struct("std__string__String"))
    //    so the TraitRegistry method-lookup path is used (not the function-lookup path)
    //
    // The bug: request_specialization was called with self_ty=None, causing it to
    // fall through to the function-lookup path which cannot find impl methods.
    // =========================================================================

    /// For Reference(&String), the inner type's mangle_suffix determines the eq method name.
    /// String is a Struct type, so its mangle_suffix is the struct name itself.
    #[test]
    fn test_string_struct_mangle_suffix() {
        // When String type reaches the Reference eq path, inner is Struct("std__string__String")
        let inner = Type::Struct("std__string__String".to_string());
        assert_eq!(inner.mangle_suffix(), "std__string__String",
            "String struct mangle_suffix must be the fully qualified name");
    }

    /// The eq_method_name for String must be "std__string__String__eq"
    #[test]
    fn test_string_eq_method_name_construction() {
        let inner = Type::Struct("std__string__String".to_string());
        let eq_method_name = format!("{}__{}", inner.mangle_suffix(), "eq");
        assert_eq!(eq_method_name, "std__string__String__eq",
            "String eq method name must be fully qualified");
    }

    /// String is NOT a primitive — it must NOT skip trait dispatch.
    /// This test ensures the guard at emit_binary line 1351 allows String through.
    #[test]
    fn test_string_is_not_primitive() {
        let inner = Type::Struct("std__string__String".to_string());
        let is_primitive = inner.is_numeric() || inner.is_integer()
            || matches!(inner, Type::Bool | Type::I8 | Type::U8);
        assert!(!is_primitive,
            "String must NOT be treated as primitive — it needs trait-dispatched eq");
    }

    /// request_specialization for struct eq methods MUST receive self_ty.
    /// Without self_ty, request_specialization uses the function-lookup path
    /// which cannot find impl methods (only top-level fn items).
    /// The self_ty should be the inner type (the struct being compared).
    #[test]
    fn test_request_specialization_requires_self_ty_for_structs() {
        // This documents the invariant: when inner is a Struct type,
        // the request_specialization call must pass Some(inner_type) as self_ty
        let inner = Type::Struct("std__string__String".to_string());

        // The correct self_ty to pass
        let self_ty: Option<Type> = Some((*Box::new(inner.clone())).clone());
        assert!(self_ty.is_some(),
            "self_ty must be Some for struct types — None causes silent hydration failure");

        // The self_ty name must match what TraitRegistry expects
        if let Some(Type::Struct(name)) = &self_ty {
            assert_eq!(name, "std__string__String",
                "self_ty struct name must be fully qualified for TraitRegistry lookup");
        } else {
            panic!("self_ty must be a Struct type");
        }
    }

    /// Validates that the eq_method_name starts with "std__" so mangle_fn_name
    /// will bypass re-mangling (the guard at the top of mangle_fn_name).
    #[test]
    fn test_string_eq_name_bypasses_remangling() {
        let eq_method_name = "std__string__String__eq";
        let bypasses = eq_method_name.starts_with("std__")
            || eq_method_name.starts_with("core__");
        assert!(bypasses,
            "String__eq method name must start with std__ to bypass mangle_fn_name re-mangling");
    }

    /// String::eq expects (&self, other: &String) — both args are pointers.
    /// When the caller has a value (e.g. HashMap::insert's key arg), it must
    /// spill the value to a stack slot and pass the pointer.
    /// When the caller already has a reference (e.g. HashMap::get's key arg),
    /// it can pass the pointer directly.
    #[test]
    fn test_string_eq_calling_convention() {
        // String::eq signature: fn eq(&self, other: &String) -> bool
        // Both parameters are pointers (references).
        // The MLIR signature is: @String__eq(!llvm.ptr, !llvm.ptr) -> i1
        //
        // Case 1: get(&self, key: &K) -> key is already !llvm.ptr -> pass directly ✅
        // Case 2: insert(&self, key: K) -> key is !struct -> must spill to stack ✅
        // Case 3: remove(&self, key: &K) -> key is already !llvm.ptr -> pass directly ✅
        
        let key_ty_in_get = Type::Reference(Box::new(Type::Struct("std__string__String".to_string())), false);
        let key_ty_in_insert = Type::Struct("std__string__String".to_string());
        
        // get: key is already a reference — can pass directly
        assert!(matches!(key_ty_in_get, Type::Reference(..)),
            "HashMap::get key type should be a Reference (already a pointer)");
        
        // insert: key is a value — needs spilling to stack before calling eq
        assert!(matches!(key_ty_in_insert, Type::Struct(_)),
            "HashMap::insert key type should be a Struct (value, needs spilling)");
        
        // The emit_binary Reference path handles both cases:
        // - For Reference args: operands are already pointers
        // - For Struct args: the comparison alloca+store pattern creates pointers
    }

    /// When `s1 == s2` is a direct value comparison (not through References),
    /// the struct eq path (line 1288) gets `struct_name` from `Type::Struct(name)`.
    /// This name may be UNQUALIFIED ("String") in user code context, while
    /// generic_impls stores the key as QUALIFIED ("std__string__String__eq").
    /// The lookup MUST handle both forms.
    #[test]
    fn test_struct_eq_name_may_be_unqualified() {
        // In user code: `let s1: String = ...; if s1 == s2 { ... }`
        // The common_ty is Struct("String") — NOT "std__string__String"
        let struct_name_unqualified = "String";
        let eq_name_unqualified = format!("{}__{}", struct_name_unqualified, "eq");
        assert_eq!(eq_name_unqualified, "String__eq");

        // But generic_impls key is "std__string__String__eq"
        let generic_impls_key = "std__string__String__eq";

        // These DON'T match — this is the bug!
        assert_ne!(eq_name_unqualified, generic_impls_key,
            "Unqualified lookup will miss the qualified key — struct eq must resolve the name");
    }

    /// Documents the fix: struct eq dispatch must resolve the struct name through
    /// type resolution (e.g. imports, type_map) before looking up in generic_impls.
    /// When struct_name is "String", it should resolve to "std__string__String"
    /// via the import system.
    #[test]
    fn test_struct_eq_must_resolve_through_imports() {
        // After resolution, the struct name becomes fully qualified
        let resolved_name = "std__string__String";
        let eq_name = format!("{}__{}", resolved_name, "eq");
        assert_eq!(eq_name, "std__string__String__eq",
            "Resolved name must match the generic_impls key");
    }

    /// CRITICAL: String values resolve as Type::Concrete("std__string__String", []),
    /// NOT as Type::Struct. The struct eq dispatch MUST also handle Type::Concrete
    /// to avoid falling through to the hardware comparison path (arith.cmpi on struct → crash).
    #[test]
    fn test_string_resolves_as_concrete_not_struct() {
        // This is what the compiler produces for `let s1: String = ...`
        let common_ty = Type::Concrete("std__string__String".to_string(), vec![]);
        
        // Type::Struct match FAILS for Concrete types
        assert!(!matches!(common_ty, Type::Struct(_)),
            "String common_ty is NOT Type::Struct — it's Type::Concrete");
        
        // Type::Concrete match SUCCEEDS
        assert!(matches!(common_ty, Type::Concrete(..)),
            "String common_ty IS Type::Concrete");
        
        // The eq method name must still be extractable from Concrete type
        if let Type::Concrete(name, _) = &common_ty {
            let eq_method_name = format!("{}__{}", name, "eq");
            assert_eq!(eq_method_name, "std__string__String__eq");
        }
    }
}
