// =============================================================================
// TDD Tests: SIR AST Extraction — Codegen Verification
// =============================================================================
// Verifies that the AST-to-SIR extraction correctly walks Salt source and
// produces SirModule with populated structs, functions, contracts, and attrs.
// =============================================================================

#[cfg(test)]
mod tests {
    use crate::codegen::sir::sir_emit::*;
    use crate::codegen::sir::types::*;

    fn extract_from_salt(salt_code: &str) -> SirModule {
        let full = format!("package main\n{}", salt_code);
        let processed = crate::preprocess(&full);
        let file: crate::grammar::SaltFile = syn::parse_str(&processed)
            .expect("Parse must succeed");
        extract_sir_from_ast(&file, "test_module")
    }

    // Test 1: Extract struct fields
    #[test]
    fn test_extracts_struct_fields() {
        let module = extract_from_salt(r#"
            struct Point {
                x: i32,
                y: i32,
            }
        "#);

        assert_eq!(module.structs.len(), 1);
        assert_eq!(module.structs[0].name, "Point");
        assert_eq!(module.structs[0].fields.len(), 2);
        assert_eq!(module.structs[0].fields[0].name, "x");
        assert_eq!(module.structs[0].fields[1].name, "y");
    }

    // Test 2: Extract function signature
    #[test]
    fn test_extracts_function_signature() {
        let module = extract_from_salt(r#"
            fn add(a: i32, b: i32) -> i32 {
                return a + b;
            }
        "#);

        assert_eq!(module.functions.len(), 1);
        assert_eq!(module.functions[0].name, "add");
        assert_eq!(module.functions[0].params.len(), 2);
        assert_eq!(module.functions[0].params[0].name, "a");
        assert_eq!(module.functions[0].params[1].name, "b");
        assert_eq!(module.functions[0].return_type, SirType::I32);
    }

    // Test 3: Extract requires contract
    #[test]
    fn test_extracts_requires_contract() {
        let module = extract_from_salt(r#"
            fn positive(x: i32) -> i32
                requires(x > 0)
            {
                return x;
            }
        "#);

        assert_eq!(module.functions.len(), 1);
        let contracts: Vec<_> = module.functions[0].contracts.iter()
            .filter(|c| c.kind == "requires")
            .collect();
        assert_eq!(contracts.len(), 1);
        assert!(contracts[0].expression.contains(">"));
    }

    // Test 4: Extract ensures contract
    #[test]
    fn test_extracts_ensures_contract() {
        let module = extract_from_salt(r#"
            fn nonzero(x: i32) -> i32
                ensures(result != 0)
            {
                if x == 0 { return 1; }
                return x;
            }
        "#);

        assert_eq!(module.functions.len(), 1);
        let contracts: Vec<_> = module.functions[0].contracts.iter()
            .filter(|c| c.kind == "ensures")
            .collect();
        assert_eq!(contracts.len(), 1);
        assert!(contracts[0].expression.contains("!=") || contracts[0].expression.contains("result"));
    }

    // Test 5: Extract pub visibility
    #[test]
    fn test_extracts_pub_visibility() {
        let module = extract_from_salt(r#"
            pub fn public_fn(x: i32) -> i32 { return x; }
            fn private_fn(x: i32) -> i32 { return x; }
        "#);

        assert_eq!(module.functions.len(), 2);
        let public_fn = module.functions.iter().find(|f| f.name == "public_fn").unwrap();
        let private_fn = module.functions.iter().find(|f| f.name == "private_fn").unwrap();
        assert!(public_fn.is_pub);
        assert!(!private_fn.is_pub);
    }

    // Test 6: Extract attributes
    #[test]
    fn test_extracts_attributes() {
        let module = extract_from_salt(r#"
            @no_mangle
            pub fn entry() -> i32 { return 0; }
        "#);

        assert_eq!(module.functions.len(), 1);
        assert!(module.functions[0].attributes.contains(&"no_mangle".to_string()));
    }
}
