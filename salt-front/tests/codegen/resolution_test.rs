use salt_front::codegen::context::CodegenContext;
use salt_front::grammar::SaltFile;
use salt_front::types::{Type, TypeKey};

#[test]
fn test_enum_result_resolution() {
    let src = r#"
        package std.core.ptr
        struct Ptr<T> { val: u64 }
        enum Result<T, E> {
            Ok(T),
            Err(E)
        }
        fn main() {
            let p = Ptr::<u64> { val: 0 };
            let r = Result::<Ptr<u64>, u8>::Ok(p);
        }
    "#;
    let file: SaltFile = syn::parse_str(src).unwrap();
    let res = salt_front::codegen::emit_mlir(&file, false, None, false, true, false, false, false, false, "");
    assert!(res.is_ok(), "Enum resolution failed via codegen_file entry point: {:?}", res.err());
    
    // Check for correct mangling in output
    let mlir = res.unwrap();
    // We expect the enum to be monomorphized/resolved. 
    // In this specific snippet `package std.core.ptr` is used.
    // The Enum `Result` should appear as check for existence.
    // We'll search for the type definition or usage.
    assert!(mlir.contains("std__core__ptr__Result"), "MLIR should contain monomorphized Result type. Got:\n{}", mlir);
}

#[test]
fn test_scope_merging_generic_leak() {
    let src = r#"
        struct Vec<T> { val: T }
        impl<T> Vec<T> {
            fn push(val: T) {
                let x: T = val;
            }
        }
        fn main() {
            let v = Vec::<bool> { val: true };
            v.push(false);
        }
    "#;
    let file: syn::File = syn::parse_str(src).expect("Failed to parse test source"); 
    // Wait, SaltFile is usually aliased or used? 
    // In resolution_tests.rs line 19: `let file: SaltFile = syn::parse_str(src).unwrap();`
    // SaltFile might be `crate::grammar::SaltFile`.
    let file: salt_front::grammar::SaltFile = syn::parse_str(src).expect("Failed to parse test source");

    let res = salt_front::codegen::emit_mlir(&file, false, None, false, true, false, false, false, false, "");
    
    // P0: MUST NOT PANIC and MUST NOT ERROR with "Generic Leak Detected"
    assert!(res.is_ok(), "Compilation failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    println!("DEBUG MLIR OUTPUT:\n{}", mlir); // For debugging if it fails

    // Check for "Vec_bool__push" or "Vec_bool__push_bool" ?
    // Method name mangling: SelfMangled__MethodName.
    // SelfMangled = Vec_bool.
    // MethodName = push.
    // Method generics? None.
    // So "Vec_bool__push".
    // Wait, does it include arguments?
    // seeker.rs: `mangled_name.push_str(&arg.mangle_suffix())` loop only for `generics` (turbofish).
    // It does NOT include argument *types*.
    // So `Vec_bool__push`.
    
    assert!(mlir.contains("Vec__push_bool"), "Output should contain Vec__push_bool. Got:\n{}", mlir);
    
    // Check for "Vec_T__push" (should NOT exist)
    assert!(!mlir.contains("Vec_T__push"), "Output should NOT contain Vec_T__push");
    assert!(!mlir.contains("_T"), "Output should not contain any _T leaks");
}

#[test]
fn test_self_hydration_invariant() {
    use salt_front::codegen::{CodegenContext, GenericContextGuard};
    use salt_front::types::Type;
    use std::collections::{BTreeMap, HashMap};

    // 1. Setup: Create a generic context
    let src = ""; 
    let file = syn::parse_str::<salt_front::grammar::SaltFile>(src).unwrap_or(salt_front::grammar::SaltFile { package: None, items: vec![], imports: vec![] });
    let z3_cfg = z3::Config::new();
    let z3_ctx = z3::Context::new(&z3_cfg);
    let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);

    // 2. Setup: Define a generic "Vec<T>"
    let struct_name = "std__collections__vec__Vec".to_string();
    let t_param = "T".to_string();
    
    // 3. Scenario: We are specializing Vec for u8
    // FIX: Populate struct_templates so hydration logic works
    {
        use salt_front::grammar::{Generics, GenericParam, StructDef};
        use syn::{Ident, Token};
        use proc_macro2::Span;
        use syn::punctuated::Punctuated;
        
        let mut params = Punctuated::new();
        params.push(GenericParam::Type { 
            name: Ident::new("T", Span::call_site()), 
            constraint: None 
        });
        
        let def = StructDef {
            name: Ident::new("Vec", Span::call_site()), // Inner name
            generics: Some(Generics { params }),
            fields: vec![],
            invariants: vec![],
        };
        ctx.struct_templates_mut().insert(struct_name.clone(), def);
    }

    let mut type_map = BTreeMap::new();
    type_map.insert(t_param.clone(), Type::U8);

    // This is the identity we EXPECT 'Self' to resolve to
    let expected_concrete_self = Type::Concrete(
        struct_name.clone(), 
        vec![Type::U8]
    );

    // 4. Execution: Enter the Specialized Context
    // We pass the generic Struct name as the initial self_ty.
    // The Guard and Tracer must "Hydrate" this using the type_map.
    {
        let _guard = GenericContextGuard::new(
            &ctx, 
            type_map, 
            Type::Struct(struct_name.clone()),
            vec![Type::U8] 
        );

        // 5. Test: Resolve 'Self'
        // We simulate the failure mode: resolve_Type::SelfType
        let resolved_self = salt_front::codegen::type_bridge::resolve_codegen_type(&ctx, &Type::SelfType);

        // ASSERTION: Self must be Vec<u8>, not just 'Vec' or 'T'
        assert_eq!(
            resolved_self, 
            expected_concrete_self,
            "Self-Type was not hydrated! Found generic residue: {:?}", resolved_self
        );

        // 6. Test: Resolve 'T'
        // T should resolve to U8 (This usually works, but verifying map presence)
        let resolved_t = salt_front::codegen::type_bridge::resolve_codegen_type(&ctx, &Type::Struct("T".to_string()));
         assert_eq!(resolved_t, Type::U8);
    }
}
