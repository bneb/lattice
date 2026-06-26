//! Integration Tests: Match Destructuring, Guards, and Let-Else
//!
//! Verifies that Salt's pattern matching features work end-to-end:
//! - Enum variant destructuring: `MyEnum::A(val) => { use val }`
//! - Match guards: `MyEnum::A(v) if v > 0 => ...`
//! - Let-else: `let MyEnum::A(val) = expr else { return 0; }`
//! - Wildcard with payload: `MyEnum::A(_) => ...`
//!
//! Uses inline enum definitions to avoid std module loader dependencies.

#[cfg(test)]
mod tests {
    /// Helper: Compile a Salt program through the full pipeline.
    fn compile_salt_program(source: &str) -> Result<String, String> {
        let processed = crate::preprocess(source);
        let mut file: crate::grammar::SaltFile = syn::parse_str(&processed)
            .map_err(|e| format!("Parse error: {}", e))?;

        let mut registry = crate::registry::Registry::new();
        registry.register(crate::registry::ModuleInfo::new("main"));
        crate::cli::load_imports(&file, &mut registry);

        crate::compile_ast(&mut file, true, Some(&registry), false, false, false, false, false, false, "<test>")
            .map_err(|e| format!("{}", e))
    }

    // =========================================================================
    // Test 1: Basic enum destructuring — bind payload in match arm
    // =========================================================================

    #[test]
    fn test_match_enum_destructuring_basic() {
        let source = r#"
            package main

            enum MyResult {
                Ok(i32),
                Err(i32)
            }

            fn check(r: MyResult) -> i32 {
                match r {
                    MyResult::Ok(val) => {
                        return val;
                    },
                    MyResult::Err(code) => {
                        return code;
                    }
                }
            }

            fn main() -> i32 {
                let x = MyResult::Ok(42);
                return check(x);
            }
        "#;

        let result = compile_salt_program(source);
        assert!(result.is_ok(),
            "Basic enum destructuring (MyResult::Ok(val)) should compile.\nError: {:?}",
            result.err());
    }

    // =========================================================================
    // Test 2: Match guard — `MyResult::Ok(v) if v > 0`
    // =========================================================================

    #[test]
    fn test_match_guard_basic() {
        let source = r#"
            package main

            enum MyResult {
                Ok(i32),
                Err(i32)
            }

            fn classify(r: MyResult) -> i32 {
                match r {
                    MyResult::Ok(v) if v > 0 => {
                        return 1;
                    },
                    MyResult::Ok(v) => {
                        return 0;
                    },
                    MyResult::Err(_) => {
                        return -1;
                    }
                }
            }

            fn main() -> i32 {
                let x = MyResult::Ok(42);
                return classify(x);
            }
        "#;

        let result = compile_salt_program(source);
        assert!(result.is_ok(),
            "Match guard (MyResult::Ok(v) if v > 0) should compile.\nError: {:?}",
            result.err());
    }

    // =========================================================================
    // Test 3: Let-else destructuring
    // =========================================================================

    #[test]
    fn test_let_else_destructuring() {
        let source = r#"
            package main

            enum MyOption {
                Some(i32),
                None
            }

            fn extract(opt: MyOption) -> i32 {
                let MyOption::Some(val) = opt else {
                    return -1;
                };
                return val;
            }

            fn main() -> i32 {
                let x = MyOption::Some(42);
                return extract(x);
            }
        "#;

        let result = compile_salt_program(source);
        assert!(result.is_ok(),
            "Let-else destructuring (let MyOption::Some(val) = opt else) should compile.\nError: {:?}",
            result.err());
    }

    // =========================================================================
    // Test 4: Wildcard payload — `MyResult::Err(_)`
    // =========================================================================

    #[test]
    fn test_match_wildcard_payload() {
        let source = r#"
            package main

            enum MyOption {
                Some(i32),
                None
            }

            fn is_some(opt: MyOption) -> i32 {
                match opt {
                    MyOption::Some(_) => {
                        return 1;
                    },
                    MyOption::None => {
                        return 0;
                    }
                }
            }

            fn main() -> i32 {
                let x = MyOption::Some(42);
                return is_some(x);
            }
        "#;

        let result = compile_salt_program(source);
        assert!(result.is_ok(),
            "Wildcard payload (MyOption::Some(_)) should compile.\nError: {:?}",
            result.err());
    }

    // =========================================================================
    // Test 5: Match guard with value use — guard + body both reference bound var
    // =========================================================================

    #[test]
    fn test_match_guard_with_value_use() {
        let source = r#"
            package main

            enum MyResult {
                Ok(i32),
                Err(i32)
            }

            fn transform(r: MyResult) -> i32 {
                match r {
                    MyResult::Ok(v) if v > 10 => {
                        return v * 2;
                    },
                    MyResult::Ok(v) => {
                        return v;
                    },
                    MyResult::Err(e) => {
                        return e;
                    }
                }
            }

            fn main() -> i32 {
                let x = MyResult::Ok(20);
                return transform(x);
            }
        "#;

        let result = compile_salt_program(source);
        assert!(result.is_ok(),
            "Match guard with value use (v > 10 => v * 2) should compile.\nError: {:?}",
            result.err());
    }

    // =========================================================================
    // Test 6: Integer literal pattern matching (non-enum)
    // =========================================================================

    #[test]
    fn test_match_integer_literal() {
        let source = r#"
            package main

            fn describe(x: i32) -> i32 {
                match x {
                    0 => {
                        return 100;
                    },
                    1 => {
                        return 200;
                    },
                    _ => {
                        return 300;
                    }
                }
            }

            fn main() -> i32 {
                return describe(1);
            }
        "#;

        let result = compile_salt_program(source);
        assert!(result.is_ok(),
            "Integer literal pattern matching should compile.\nError: {:?}",
            result.err());
    }
}
