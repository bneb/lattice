// ============================================================================
// Generic Receiver Specialization Tests
// Guards against regression of the Sieve benchmark linker issue (Jan 2026)
//
// Root Causes Fixed:
// 1. Suffix derivation from self_ty (not just concrete_tys)
// 2. current_self_ty override during method signature resolution
// 3. Effective receiver type for Self substitution after TYPE-OVERRIDE
// 4. Use expected_arg_tys[0] for self arg type in call emission
// ============================================================================

#[cfg(test)]
mod tests {
    use salt_front::codegen::context::CodegenContext;
    use salt_front::types::Type;
    use salt_front::grammar::SaltFile;

    // Helper macro for creating test contexts
    macro_rules! with_ctx {
        ($name:ident, $block:block) => {
            let file: SaltFile = syn::parse_str("fn main() {}").expect("valid salt file");
            let z3_cfg = z3::Config::new();
            let z3_ctx = z3::Context::new(&z3_cfg);
            #[allow(unused_mut)]
            let mut $name = CodegenContext::new(&file, false, None, &z3_ctx);
            $block
        }
    }

    // ============================================================================
    // Test 1: Suffix Derivation from self_ty
    // ============================================================================

    #[test]
    fn test_request_specialization_derives_suffix_from_self_ty() {
        with_ctx!(ctx, {
            // Setup a minimal method in method_registry that would be specialized
            // When called with self_ty=Concrete("Ptr", [U8]) and empty concrete_tys,
            // the mangled name should include "_u8" suffix
            
            let func_name = "std__core__ptr__Ptr__offset";
            let concrete_tys = vec![]; // Empty!
            let self_ty = Some(Type::Concrete("std__core__ptr__Ptr".to_string(), vec![Type::U8]));
            
            let mangled = ctx.request_specialization(func_name, concrete_tys, self_ty);
            
            // Critical assertion: suffix "_u8" should be derived from self_ty
            assert!(
                mangled.ends_with("_u8") || mangled.contains("_u8"),
                "Mangled name should include suffix from self_ty: expected suffix '_u8' in '{}', got: {}",
                func_name, mangled
            );
        });
    }

    #[test]
    fn test_request_specialization_prefers_concrete_tys_over_self_ty() {
        with_ctx!(ctx, {
            // When concrete_tys is non-empty, suffix should come from concrete_tys, not self_ty
            let func_name = "generic_fn";
            let concrete_tys = vec![Type::I32];
            let self_ty = Some(Type::Concrete("SomeType".to_string(), vec![Type::U8]));
            
            let mangled = ctx.request_specialization(func_name, concrete_tys, self_ty);
            
            // Should have "_i32" suffix from concrete_tys, not "_u8" from self_ty
            assert!(
                mangled.ends_with("_i32"),
                "Mangled name should prefer concrete_tys suffix: expected '*_i32', got: {}",
                mangled
            );
        });
    }

    #[test]
    fn test_request_specialization_no_suffix_when_both_empty() {
        with_ctx!(ctx, {
            // When both concrete_tys is empty and self_ty has no generics, no suffix should be added
            let func_name = "simple_fn";
            let concrete_tys = vec![];
            let self_ty = Some(Type::Struct("SimpleStruct".to_string()));
            
            let mangled = ctx.request_specialization(func_name, concrete_tys, self_ty);
            
            // Should not have any suffix
            assert_eq!(
                mangled, "simple_fn",
                "Mangled name should have no suffix when both are empty: {}",
                mangled
            );
        });
    }

    // ============================================================================  
    // Test 2: Type Conversion for Concrete Types with Generics
    // ============================================================================

    #[test]
    fn test_concrete_type_to_mlir_includes_specialization() {
        with_ctx!(ctx, {
            use salt_front::registry::StructInfo;
            use std::collections::{BTreeMap, HashMap};
            use salt_front::types::TypeKey;
            
            // Register a generic struct template
            let template_name = "std__core__ptr__Ptr".to_string();
            let info = StructInfo {
                name: format!("{}_u8", template_name),
                fields: HashMap::new(),
                field_order: vec![Type::I64, Type::I64], // Ptr has addr + len
                field_alignments: vec![],
                template_name: Some(template_name.clone()),
                specialization_args: vec![Type::U8],
            };
            let key = TypeKey { 
                path: vec!["std".into(), "core".into(), "ptr".into()], 
                name: "Ptr_u8".into(), 
                specialization: Some(vec![Type::U8]) 
            };
            ctx.struct_registry_mut().insert(key, info);
            
            // Concrete type with U8 specialization - Ptr is a pointer type
            // [V1.0 POINTER DECAY RULE] Pointer types emit as !llvm.ptr
            let ty = Type::Concrete("std__core__ptr__Ptr".to_string(), vec![Type::U8]);
            let mlir = ctx.with_lowering_ctx(|lctx| ty.to_mlir_type(lctx));
            
            assert!(mlir.is_ok(), "Should convert Concrete type to MLIR: {:?}", mlir);
            let mlir_str = mlir.unwrap();
            
            // Pointer types now emit directly as !llvm.ptr per Pointer Decay Rule
            assert_eq!(
                mlir_str, "!llvm.ptr",
                "Pointer types should emit as !llvm.ptr per Pointer Decay Rule: {}",
                mlir_str
            );
        });
    }

    // ============================================================================
    // Test 3: Reference vs Value Type in Method Signatures
    // ============================================================================

    #[test]
    fn test_reference_type_to_mlir_is_ptr() {
        with_ctx!(ctx, {
            // Type::Reference should produce !llvm.ptr in MLIR
            let ty = Type::Reference(Box::new(Type::I32), false);
            let mlir = ctx.with_lowering_ctx(|lctx| ty.to_mlir_type(lctx)).unwrap();
            
            assert_eq!(mlir, "!llvm.ptr", "Reference type should be !llvm.ptr: {}", mlir);
        });
    }

    #[test] 
    fn test_concrete_type_distinct_from_reference() {
        with_ctx!(ctx, {
            use salt_front::registry::StructInfo;
            use std::collections::{BTreeMap, HashMap};
            use salt_front::types::TypeKey;
            
            // Register a concrete struct
            let name = "TestStruct".to_string();
            let info = StructInfo {
                name: name.clone(),
                fields: HashMap::new(),
                field_order: vec![Type::I64],
                field_alignments: vec![],
                template_name: None,
                specialization_args: vec![],
            };
            let key = TypeKey { path: vec![], name: name.clone(), specialization: None };
            ctx.struct_registry_mut().insert(key, info);
            
            // Concrete struct type vs Reference to struct
            let struct_ty = Type::Struct(name.clone());
            let ref_ty = Type::Reference(Box::new(struct_ty.clone()), false);
            
            let struct_mlir = ctx.with_lowering_ctx(|lctx| struct_ty.to_mlir_type(lctx)).unwrap();
            let ref_mlir = ctx.with_lowering_ctx(|lctx| ref_ty.to_mlir_type(lctx)).unwrap();
            
            // They should be different!
            assert_ne!(
                struct_mlir, ref_mlir,
                "Struct type and Reference(Struct) should produce different MLIR: struct='{}', ref='{}'",
                struct_mlir, ref_mlir
            );
            
            // Reference should be ptr
            assert_eq!(ref_mlir, "!llvm.ptr", "Reference should be !llvm.ptr");
            
            // Struct should NOT be !llvm.ptr
            assert_ne!(struct_mlir, "!llvm.ptr", "Struct should not be !llvm.ptr: {}", struct_mlir);
        });
    }

    // ============================================================================
    // Test 4: Type Substitution for Self
    // ============================================================================

    #[test]
    fn test_type_substitute_self() {
        // Test that Type::substitute correctly replaces "Self"
        let input_ty = Type::Struct("Self".to_string());
        let target_ty = Type::Concrete("std__core__ptr__Ptr".to_string(), vec![Type::U8]);
        
        let mut subst_map = std::collections::BTreeMap::new();
        subst_map.insert("Self".to_string(), target_ty.clone());
        
        let result = input_ty.substitute(&subst_map);
        
        assert_eq!(
            result, target_ty,
            "Self should be substituted with target type: {:?}",
            result
        );
    }

    #[test]
    fn test_reference_self_substitute() {
        // Test that &Self is correctly substituted to &Ptr<u8>
        let input_ty = Type::Reference(Box::new(Type::Struct("Self".to_string())), false);
        let target_ty = Type::Concrete("Ptr".to_string(), vec![Type::U8]);
        
        let mut subst_map = std::collections::BTreeMap::new();
        subst_map.insert("Self".to_string(), target_ty.clone());
        
        let result = input_ty.substitute(&subst_map);
        
        // Should be Reference(Concrete("Ptr", [U8]))
        if let Type::Reference(inner, _) = result {
            assert_eq!(
                *inner, target_ty,
                "Inner of &Self should be substituted: {:?}",
                inner
            );
        } else {
            panic!("Result should be Reference type: {:?}", result);
        }
    }

    // ============================================================================
    // Test 5: Mangle Suffix Generation
    // ============================================================================

    #[test]
    fn test_mangle_suffix_primitives() {
        assert_eq!(Type::U8.mangle_suffix(), "u8");
        assert_eq!(Type::I32.mangle_suffix(), "i32");
        assert_eq!(Type::I64.mangle_suffix(), "i64");
        assert_eq!(Type::Bool.mangle_suffix(), "bool");
    }

    #[test]
    fn test_mangle_suffix_concrete() {
        let ty = Type::Concrete("Vec".to_string(), vec![Type::U8]);
        let suffix = ty.mangle_suffix();
        
        // Concrete types should have mangled inner types
        assert!(
            suffix.contains("Vec") && suffix.contains("u8"),
            "Concrete mangle suffix should include base and args: {}",
            suffix
        );
    }

    // ============================================================================
    // Test 6: TypeKey creation and lookup patterns
    // ============================================================================

    #[test]
    fn test_type_to_type_key_concrete() {
        let ty = Type::Concrete("std__core__ptr__Ptr".to_string(), vec![Type::U8]);
        let key = salt_front::codegen::type_bridge::type_to_type_key(&ty);
        
        // type_to_type_key stores the full FQN in name (needed for downstream registry lookups)
        assert_eq!(key.name, "std__core__ptr__Ptr", "Name should be full FQN for registry match: {:?}", key);
        assert_eq!(key.path, vec!["std", "core", "ptr"], "Path should be all but last: {:?}", key);
        assert_eq!(key.specialization, Some(vec![Type::U8]), "Specialization should include args");
    }

    #[test]
    fn test_type_key_without_specialization_for_template_lookup() {
        // When looking up templates, we create a key without specialization
        let ty = Type::Concrete("std__core__ptr__Ptr".to_string(), vec![Type::U8]);
        let key = salt_front::codegen::type_bridge::type_to_type_key(&ty);
        
        // Create base key for template lookup
        let base_key = salt_front::types::TypeKey {
            path: key.path.clone(),
            name: key.name.clone(),
            specialization: None,
        };
        
        assert!(base_key.specialization.is_none(), "Base key should have no specialization");
        // type_to_type_key stores full FQN in name for registry lookups
        assert_eq!(base_key.name, "std__core__ptr__Ptr", "Base key should preserve full FQN");
    }
}
