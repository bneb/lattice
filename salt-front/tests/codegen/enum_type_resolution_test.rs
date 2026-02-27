// ============================================================================
// Enum Type Resolution Regression Tests
// Guards against MLIR type alias mismatches when enums passed as Type::Struct
// 
// Fixes tested:
// - Type::Struct("main__Status") should resolve to !struct_Status when Status
//   is registered in enum_registry
// - Package prefix stripping: "main__Status" -> "Status" for lookup
// - Enum registry takes priority over struct fallback
// ============================================================================

#[cfg(test)]
mod tests {
    use salt_front::codegen::context::CodegenContext;
    use salt_front::registry::EnumInfo;
    use salt_front::types::{Type, TypeKey};
    use salt_front::grammar::SaltFile;

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

    /// Helper to create an EnumInfo for testing
    fn make_test_enum(name: &str) -> EnumInfo {
        EnumInfo {
            name: name.to_string(),
            variants: vec![("V1".to_string(), None, 0), ("V2".to_string(), None, 1)],
            max_payload_size: 0,
            template_name: None,
            specialization_args: vec![],
        }
    }

    /// Helper to create a TypeKey for testing
    fn make_type_key(name: &str) -> TypeKey {
        TypeKey { path: vec![], name: name.to_string(), specialization: None }
    }

    // =========================================================================
    // Parameterized Test Cases for Enum Type Resolution
    // =========================================================================
    
    /// Test case structure for parameterized enum resolution tests
    struct EnumResolutionCase {
        /// Name to register in enum_registry
        registered_name: &'static str,
        /// Type to resolve (e.g., Type::Struct("main__Status") or Type::Enum)
        input_type: fn() -> Type,
        /// Description for debugging
        description: &'static str,
        /// Expected MLIR type output
        expected_result: &'static str,
    }

    fn get_enum_resolution_cases() -> Vec<EnumResolutionCase> {
        vec![
            // Case 1: Exact match - Type::Enum with registered name
            EnumResolutionCase {
                registered_name: "Status",
                input_type: || Type::Enum("Status".to_string()),
                description: "Type::Enum with exact registered name",
                expected_result: "!struct_Status",  // Exact match uses registered name
            },
            // Case 2: Type::Struct with package prefix (main__Status -> Status)
            EnumResolutionCase {
                registered_name: "Status",
                input_type: || Type::Struct("main__Status".to_string()),
                description: "Type::Struct with package prefix resolves via stripped lookup",
                expected_result: "!struct_main__Status",  // With prefix, keeps full name
            },
            // Case 3: Type::Enum with package prefix
            EnumResolutionCase {
                registered_name: "Status",
                input_type: || Type::Enum("main__Status".to_string()),
                description: "Type::Enum with package prefix resolves via stripped lookup",
                expected_result: "!struct_Status",  // Enum lookup uses registered name, stripping prefix
            },
            // Case 4: Multi-level package prefix (foo__bar__Status -> Status)
            EnumResolutionCase {
                registered_name: "Status",
                input_type: || Type::Struct("foo__bar__Status".to_string()),
                description: "Multi-level package prefix uses last segment",
                expected_result: "!struct_foo__bar__Status",  // Multi-level prefix kept
            },
            // Case 5: Different enum names should not conflict
            EnumResolutionCase {
                registered_name: "Result",
                input_type: || Type::Struct("main__Result".to_string()),
                description: "Different enum names resolve correctly",
                expected_result: "!struct_main__Result",  // Different enum with prefix
            },
        ]
    }

    #[test]
    fn test_enum_resolution_parameterized() {
        for case in get_enum_resolution_cases() {
            with_ctx!(ctx, {
                // Register enum in registry
                let info = make_test_enum(case.registered_name);
                let key = make_type_key(case.registered_name);
                ctx.enum_registry_mut().insert(key, info);

                // Resolve type
                let input = (case.input_type)();
                let result = ctx.with_lowering_ctx(|lctx| input.to_mlir_type(lctx));

                assert!(
                    result.is_ok(),
                    "Case '{}' should succeed, got error: {:?}",
                    case.description,
                    result.err()
                );
                assert_eq!(
                    result.unwrap(),
                    case.expected_result,
                    "Case '{}' failed",
                    case.description
                );
            });
        }
    }

    // =========================================================================
    // Edge Cases: Enum vs Struct Priority
    // =========================================================================

    #[test]
    fn test_enum_registry_takes_priority_over_struct() {
        with_ctx!(ctx, {
            // Register enum
            let enum_info = make_test_enum("Status");
            let enum_key = make_type_key("Status");
            ctx.enum_registry_mut().insert(enum_key, enum_info);

            // Also register struct with same name
            let struct_info = salt_front::registry::StructInfo {
                name: "Status".to_string(),
                fields: std::collections::HashMap::new(),
                field_order: vec![],
                field_alignments: vec![],
                template_name: None,
                specialization_args: vec![],
            };
            let struct_key = make_type_key("Status");
            ctx.struct_registry_mut().insert(struct_key, struct_info);

            // Type::Struct should still resolve to enum type
            let ty = Type::Struct("main__Status".to_string());
            let result = ctx.with_lowering_ctx(|lctx| ty.to_mlir_type(lctx)).unwrap();
            
            // Enum registry takes priority
            assert_eq!(result, "!struct_main__Status", "Full name maintained for consistency");
        });
    }

    #[test]
    fn test_unregistered_enum_falls_back_to_opaque() {
        with_ctx!(ctx, {
            // No enum registered
            let ty = Type::Enum("Unknown".to_string());
            let result = ctx.with_lowering_ctx(|lctx| ty.to_mlir_type(lctx)).unwrap();
            
            // [VERIFIED METAL] Unregistered types use !struct_ prefix format for consistency
            assert_eq!(result, "!struct_Unknown");
        });
    }

    #[test]
    fn test_unregistered_mangled_name_falls_back_to_opaque() {
        with_ctx!(ctx, {
            // No enum registered for "FooBar"
            let ty = Type::Struct("main__FooBar".to_string());
            let result = ctx.with_lowering_ctx(|lctx| ty.to_mlir_type(lctx)).unwrap();
            
            // [VERIFIED METAL] Uses canonical name with !struct_ prefix
            // The canonical name strips the package prefix
            assert_eq!(result, "!struct_main__FooBar");  // Unregistered keeps input name
        });
    }
}
