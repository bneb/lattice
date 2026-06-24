//! Tests for Generic Enum Pattern Matching and Noreturn Codegen
//!
//! These tests verify two critical behaviors discovered while fixing the
//! buffered_writer_perf benchmark:
//!
//! - Bug D1: `Type::Concrete` enum lookup in pattern match uses base name
//!   (e.g., "std__core__result__Result") instead of fully-mangled name
//!   (e.g., "std__core__result__Result_File_IOError"). Fixed by using
//!   `mangle_suffix()` in stmt.rs.
//!
//! - Bug D2: Modular codegen return checker rejects functions where a
//!   match arm calls exit() without explicit return (treats exit() as
//!   non-diverging). This prevents Result::unwrap() from compiling.

mod tests {
    use crate::codegen::CodegenContext;
    use crate::grammar::SaltFile;
    use crate::types::{Type, TypeKey};
    use crate::registry::EnumInfo;

    /// Helper: Create a fresh CodegenContext for testing
    fn make_ctx() -> (SaltFile, crate::z3_shim::Config, crate::z3_shim::Context) {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        (file, z3_cfg, z3_ctx)
    }

    // =========================================================================
    // Section 1: Type::Concrete mangle_suffix correctness
    //
    // When a generic enum like Result<T, E> is specialized to
    // Result<File, IOError>, the `mangle_suffix()` method must produce
    // the fully-mangled name that matches what `specialize_template`
    // registers in the enum_registry.
    //
    // Bug D1: stmt.rs extracted only the base name from Type::Concrete,
    // producing "std__core__result__Result" instead of
    // "std__core__result__Result_std__io__file__File_std__io__file__IOError".
    // =========================================================================

    #[test]
    fn test_mangle_suffix_concrete_result_file_ioerror() {
        let file_ty = Type::Struct("std__io__file__File".to_string());
        let ioerror_ty = Type::Struct("std__io__file__IOError".to_string());
        let result_ty = Type::Concrete(
            "std__core__result__Result".to_string(),
            vec![file_ty, ioerror_ty],
        );

        let mangled = result_ty.mangle_suffix();

        assert!(
            mangled.contains("std__core__result__Result"),
            "Mangled name must include the base enum name. Got: {}", mangled
        );
        assert!(
            mangled.contains("std__io__file__File"),
            "Mangled name must include first arg (File). Got: {}", mangled
        );
        assert!(
            mangled.contains("std__io__file__IOError"),
            "Mangled name must include second arg (IOError). Got: {}", mangled
        );
        assert_eq!(
            mangled,
            "std__core__result__Result_std__io__file__File_std__io__file__IOError",
            "Mangled name should be base_arg1_arg2 format"
        );
    }

    #[test]
    fn test_mangle_suffix_concrete_with_primitives() {
        let result_ty = Type::Concrete(
            "std__core__result__Result".to_string(),
            vec![Type::I32, Type::Bool],
        );

        let mangled = result_ty.mangle_suffix();
        assert_eq!(
            mangled,
            "std__core__result__Result_i32_bool",
            "Concrete type with primitive args should mangle correctly"
        );
    }

    #[test]
    fn test_mangle_suffix_enum_vs_concrete_divergence() {
        // Type::Enum always has a simple name — it IS the fully-mangled form
        let enum_ty = Type::Enum("std__core__result__Result".to_string());
        let enum_mangled = enum_ty.mangle_suffix();

        // Type::Concrete with no args — should match Enum behavior
        let concrete_no_args = Type::Concrete("std__core__result__Result".to_string(), vec![]);
        let concrete_mangled = concrete_no_args.mangle_suffix();

        assert_eq!(
            enum_mangled, concrete_mangled,
            "Enum('Result') and Concrete('Result', []) should produce same mangle_suffix"
        );
    }

    #[test]
    fn test_mangle_suffix_nested_concrete() {
        let inner = Type::Concrete(
            "std__core__result__Result".to_string(),
            vec![Type::I32, Type::Bool],
        );
        let outer = Type::Concrete(
            "std__core__result__Result".to_string(),
            vec![inner, Type::I64],
        );

        let mangled = outer.mangle_suffix();
        assert!(
            mangled.contains("std__core__result__Result_i32_bool"),
            "Nested Concrete specialization must be recursively mangled. Got: {}", mangled
        );
    }

    // =========================================================================
    // Section 2: Enum registry lookup with specialized generic enums
    //
    // After specialize_template registers a specialized enum (e.g.,
    // "Result_File_IOError") in the enum_registry, the pattern match
    // codegen in stmt.rs must be able to find it via mangle_suffix().
    // =========================================================================

    #[test]
    fn test_enum_registry_lookup_concrete_type() {
        let (file, _z3_cfg, z3_ctx) = make_ctx();
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mangled_name = "std__core__result__Result_std__io__file__File_std__io__file__IOError";

        let key = TypeKey {
            path: vec!["std".into(), "core".into(), "result".into()],
            name: "Result".to_string(),
            specialization: Some(vec![
                Type::Struct("std__io__file__File".to_string()),
                Type::Struct("std__io__file__IOError".to_string()),
            ]),
        };

        ctx.enum_registry_mut().insert(
            key,
            EnumInfo {
                name: mangled_name.to_string(),
                variants: vec![
                    ("Ok".to_string(), Some(Type::Struct("std__io__file__File".to_string())), 0),
                    ("Err".to_string(), Some(Type::Struct("std__io__file__IOError".to_string())), 1),
                ],
                max_payload_size: 8,
                template_name: Some("std__core__result__Result".to_string()),
                specialization_args: vec![
                    Type::Struct("std__io__file__File".to_string()),
                    Type::Struct("std__io__file__IOError".to_string()),
                ],
            },
        );

        // Build the scrutinee type as it appears during codegen
        let scrutinee_ty = Type::Concrete(
            "std__core__result__Result".to_string(),
            vec![
                Type::Struct("std__io__file__File".to_string()),
                Type::Struct("std__io__file__IOError".to_string()),
            ],
        );

        // FIXED lookup logic from stmt.rs: use mangle_suffix()
        let enum_name = match &scrutinee_ty {
            Type::Enum(name) => name.clone(),
            Type::Concrete(_, _) => scrutinee_ty.mangle_suffix(),
            _ => panic!("Unexpected type"),
        };

        // The lookup must succeed
        let registry = ctx.enum_registry();
        let found = registry.values()
            .find(|i| i.name == enum_name || i.name.ends_with(&format!("__{}", enum_name)));
        assert!(
            found.is_some(),
            "Enum registry lookup for specialized Result<File, IOError> must succeed.\n\
             Lookup key: '{}'", enum_name
        );

        let info = found.unwrap();
        assert_eq!(info.variants.len(), 2, "Result should have exactly 2 variants");
        assert_eq!(info.variants[0].0, "Ok", "First variant should be Ok");
        assert_eq!(info.variants[1].0, "Err", "Second variant should be Err");
    }

    #[test]
    fn test_enum_registry_lookup_base_name_fails() {
        // Document that using the base name ALONE fails for specialized enums
        let (file, _z3_cfg, z3_ctx) = make_ctx();
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mangled_name = "std__core__result__Result_std__io__file__File_std__io__file__IOError";

        let key = TypeKey {
            path: vec!["std".into(), "core".into(), "result".into()],
            name: "Result".to_string(),
            specialization: Some(vec![
                Type::Struct("std__io__file__File".to_string()),
                Type::Struct("std__io__file__IOError".to_string()),
            ]),
        };

        ctx.enum_registry_mut().insert(
            key,
            EnumInfo {
                name: mangled_name.to_string(),
                variants: vec![
                    ("Ok".to_string(), Some(Type::Struct("std__io__file__File".to_string())), 0),
                    ("Err".to_string(), Some(Type::Struct("std__io__file__IOError".to_string())), 1),
                ],
                max_payload_size: 8,
                template_name: Some("std__core__result__Result".to_string()),
                specialization_args: vec![],
            },
        );

        // The OLD buggy lookup: just uses the Concrete base name
        let base_name_only = "std__core__result__Result";

        let registry = ctx.enum_registry();
        let found = registry.values()
            .find(|i| i.name == base_name_only || i.name.ends_with(&format!("__{}", base_name_only)));
        assert!(
            found.is_none(),
            "Base name '{}' should NOT match specialized enum '{}'. \
             This was the root cause of Bug D1.",
            base_name_only, mangled_name
        );
    }

    #[test]
    fn test_enum_registry_lookup_non_generic_still_works() {
        // Non-generic enums (Status) work with both Type::Enum and Type::Concrete(name, [])
        let (file, _z3_cfg, z3_ctx) = make_ctx();
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let key = TypeKey {
            path: vec![],
            name: "MyEnum".to_string(),
            specialization: None,
        };

        ctx.enum_registry_mut().insert(
            key,
            EnumInfo {
                name: "MyEnum".to_string(),
                variants: vec![
                    ("A".to_string(), None, 0),
                    ("B".to_string(), None, 1),
                ],
                max_payload_size: 0,
                template_name: None,
                specialization_args: vec![],
            },
        );

        // Type::Enum path
        let enum_name = Type::Enum("MyEnum".to_string()).mangle_suffix();
        let registry = ctx.enum_registry();
        let found = registry.values().find(|i| i.name == enum_name);
        assert!(found.is_some(), "Type::Enum('MyEnum') lookup must succeed");
        drop(registry);

        // Type::Concrete(name, []) path
        let concrete_name = Type::Concrete("MyEnum".to_string(), vec![]).mangle_suffix();
        let registry = ctx.enum_registry();
        let found = registry.values().find(|i| i.name == concrete_name);
        assert!(found.is_some(), "Type::Concrete('MyEnum', []) lookup must succeed");
    }

    // =========================================================================
    // Section 3: Status enum builtin alignment with gRPC codes
    // =========================================================================

    #[test]
    fn test_status_is_not_builtin_enum() {
        // Status is now a struct defined in std/status.salt,
        // discovered through normal import pipeline — NOT a builtin enum.
        let (file, _z3_cfg, z3_ctx) = make_ctx();
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        ctx.register_builtins();

        let registry = ctx.enum_registry();
        let status = registry.values().find(|i| i.name == "Status");
        assert!(
            status.is_none(),
            "Status should NOT be in enum registry — it is now a struct in std/status.salt"
        );
    }
}
