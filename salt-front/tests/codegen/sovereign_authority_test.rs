// ============================================================================
// Sovereign Authority Rule Tests
// Validates the Coherence Policing system that enforces trait implementation
// ownership rules, preventing "Orphan" implementations and ensuring
// deterministic symbol emission.
//
// Tests cover:
// - Positive: Legal trait implementations (type-home & trait-home)
// - Negative: Orphan hijacking, primitive hijacking, duplicate implementations
// - Discovery: Type and trait home registration
// ============================================================================

#[cfg(test)]
mod sovereign_authority_tests {
    use salt_front::codegen::phases::DiscoveryState;
    use salt_front::grammar::SaltFile;

    /// Helper to create a DiscoveryState with a dummy file for tests
    fn make_disc_state(file: &SaltFile) -> DiscoveryState {
        DiscoveryState::new(file)
    }

    // =========================================================================
    // Phase 1: DiscoveryState Unit Tests (Foundation)
    // =========================================================================

    #[test]
    fn test_register_type_home() {
        let file: SaltFile = syn::parse_str("fn main() {}").expect("valid salt file");
        let mut state = make_disc_state(&file);
        state.register_type_home("std__string__String".to_string(), "std.string".to_string());
        
        assert!(state.is_type_home("std__string__String", "std.string"));
        assert!(!state.is_type_home("std__string__String", "user.main"));
    }

    #[test]
    fn test_register_trait_home() {
        let file: SaltFile = syn::parse_str("fn main() {}").expect("valid salt file");
        let mut state = make_disc_state(&file);
        state.register_trait_home("std__eq__Eq".to_string(), "std.eq".to_string());
        
        assert!(state.is_trait_home("std__eq__Eq", "std.eq"));
        assert!(!state.is_trait_home("std__eq__Eq", "user.main"));
    }

    #[test]
    fn test_type_home_first_writer_wins() {
        let file: SaltFile = syn::parse_str("fn main() {}").expect("valid salt file");
        let mut state = make_disc_state(&file);
        state.register_type_home("Foo".to_string(), "module_a".to_string());
        // Second registration should NOT overwrite
        state.register_type_home("Foo".to_string(), "module_b".to_string());
        
        assert!(state.is_type_home("Foo", "module_a"));
        assert!(!state.is_type_home("Foo", "module_b"));
    }

    #[test]
    fn test_unknown_type_is_never_home() {
        let file: SaltFile = syn::parse_str("fn main() {}").expect("valid salt file");
        let state = make_disc_state(&file);
        assert!(!state.is_type_home("Unknown", "any_module"));
    }

    // =========================================================================
    // Phase 2: Coherence Validation Tests
    // =========================================================================

    /// Legal: Module owns the Type → can implement any trait for it
    #[test]
    fn test_legal_impl_type_home() {
        let file: SaltFile = syn::parse_str("fn main() {}").expect("valid salt file");
        let mut state = make_disc_state(&file);
        state.register_type_home("Foo".to_string(), "user.data".to_string());
        state.register_trait_home("Eq".to_string(), "std.eq".to_string());
        
        // user.data owns Foo → legal to impl Eq for Foo
        let result = state.register_trait_impl(
            "Foo".to_string(), "Eq".to_string(), "user.data".to_string()
        );
        assert!(result.is_ok());
        
        let coherence = state.validate_coherence();
        assert!(coherence.is_ok(), "Type-home impl should pass coherence: {:?}", coherence);
    }

    /// Legal: Module owns the Trait → can implement it for any type
    #[test]
    fn test_legal_impl_trait_home() {
        let file: SaltFile = syn::parse_str("fn main() {}").expect("valid salt file");
        let mut state = make_disc_state(&file);
        state.register_type_home("i64".to_string(), "std.primitives".to_string());
        state.register_trait_home("Inspector".to_string(), "user.inspect".to_string());
        
        // user.inspect owns Inspector → legal to impl Inspector for i64
        let result = state.register_trait_impl(
            "i64".to_string(), "Inspector".to_string(), "user.inspect".to_string()
        );
        assert!(result.is_ok());
        
        let coherence = state.validate_coherence();
        assert!(coherence.is_ok(), "Trait-home impl should pass coherence: {:?}", coherence);
    }

    // =========================================================================
    // Phase 3: Negative Tests (Orphan Detection)
    // =========================================================================

    /// ILLEGAL: Module owns neither the Type nor the Trait
    #[test]
    fn test_reject_orphan_hijack() {
        let file: SaltFile = syn::parse_str("fn main() {}").expect("valid salt file");
        let mut state = make_disc_state(&file);
        state.register_type_home("std__string__String".to_string(), "std.string".to_string());
        state.register_trait_home("std__eq__Eq".to_string(), "std.eq".to_string());
        
        // user.main owns NEITHER String NOR Eq → orphan
        let result = state.register_trait_impl(
            "std__string__String".to_string(), 
            "std__eq__Eq".to_string(), 
            "user.main".to_string()
        );
        assert!(result.is_ok(), "Registration should succeed (detection happens at validation)");
        
        let coherence = state.validate_coherence();
        assert!(coherence.is_err(), "Orphan implementation should be rejected");
        let error = coherence.unwrap_err();
        assert!(error.contains("Orphan Implementation"), "Error should mention 'Orphan Implementation': {}", error);
        assert!(error.contains("std__string__String"), "Error should mention the type: {}", error);
        assert!(error.contains("std__eq__Eq"), "Error should mention the trait: {}", error);
    }

    /// ILLEGAL: Implementing external trait for a primitive you don't own
    #[test]
    fn test_reject_primitive_hijack() {
        let file: SaltFile = syn::parse_str("fn main() {}").expect("valid salt file");
        let mut state = make_disc_state(&file);
        state.register_type_home("i64".to_string(), "std.primitives".to_string());
        state.register_trait_home("Eq".to_string(), "std.eq".to_string());
        
        // user.main owns neither i64 nor Eq → orphan
        let _ = state.register_trait_impl(
            "i64".to_string(), "Eq".to_string(), "user.main".to_string()
        );
        
        let coherence = state.validate_coherence();
        assert!(coherence.is_err(), "Primitive hijack should be rejected");
        assert!(coherence.unwrap_err().contains("Orphan Implementation"));
    }

    /// ILLEGAL: Duplicate implementation in the same module
    #[test]
    fn test_reject_duplicate_implementation() {
        let file: SaltFile = syn::parse_str("fn main() {}").expect("valid salt file");
        let mut state = make_disc_state(&file);
        state.register_type_home("Foo".to_string(), "user.data".to_string());
        state.register_trait_home("Eq".to_string(), "std.eq".to_string());
        
        // First impl: OK
        let result1 = state.register_trait_impl(
            "Foo".to_string(), "Eq".to_string(), "user.data".to_string()
        );
        assert!(result1.is_ok());
        
        // Second impl: should fail immediately (duplicate)
        let result2 = state.register_trait_impl(
            "Foo".to_string(), "Eq".to_string(), "user.data".to_string()
        );
        assert!(result2.is_err(), "Duplicate implementation should be rejected");
        assert!(result2.unwrap_err().contains("Duplicate Implementation"));
    }

    /// Legal: Same trait for DIFFERENT types in the same module
    #[test]
    fn test_allow_different_types_same_trait() {
        let file: SaltFile = syn::parse_str("fn main() {}").expect("valid salt file");
        let mut state = make_disc_state(&file);
        state.register_type_home("Foo".to_string(), "user.data".to_string());
        state.register_type_home("Bar".to_string(), "user.data".to_string());
        state.register_trait_home("Eq".to_string(), "std.eq".to_string());
        
        let r1 = state.register_trait_impl("Foo".to_string(), "Eq".to_string(), "user.data".to_string());
        let r2 = state.register_trait_impl("Bar".to_string(), "Eq".to_string(), "user.data".to_string());
        
        assert!(r1.is_ok());
        assert!(r2.is_ok());
        assert!(state.validate_coherence().is_ok());
    }

    /// Legal: Different traits for the same type in the same module
    #[test]
    fn test_allow_same_type_different_traits() {
        let file: SaltFile = syn::parse_str("fn main() {}").expect("valid salt file");
        let mut state = make_disc_state(&file);
        state.register_type_home("Foo".to_string(), "user.data".to_string());
        state.register_trait_home("Eq".to_string(), "std.eq".to_string());
        state.register_trait_home("Display".to_string(), "std.display".to_string());
        
        let r1 = state.register_trait_impl("Foo".to_string(), "Eq".to_string(), "user.data".to_string());
        let r2 = state.register_trait_impl("Foo".to_string(), "Display".to_string(), "user.data".to_string());
        
        assert!(r1.is_ok());
        assert!(r2.is_ok());
        assert!(state.validate_coherence().is_ok());
    }

    // =========================================================================
    // Phase 4: Trait Dispatch Integration Tests (via CodegenContext)
    // =========================================================================

    use salt_front::codegen::context::CodegenContext;

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

    /// Verify that register_type_home works through CodegenContext
    #[test]
    fn test_codegen_context_type_home_registration() {
        with_ctx!(ctx, {
            ctx.register_type_home("TestStruct".to_string(), "test.module".to_string());
            assert!(ctx.is_type_home("TestStruct", "test.module"));
            assert!(!ctx.is_type_home("TestStruct", "other.module"));
        });
    }

    /// Verify that register_trait_home works through CodegenContext
    #[test]
    fn test_codegen_context_trait_home_registration() {
        with_ctx!(ctx, {
            ctx.register_trait_home("TestTrait".to_string(), "test.module".to_string());
            assert!(ctx.is_trait_home("TestTrait", "test.module"));
            assert!(!ctx.is_trait_home("TestTrait", "other.module"));
        });
    }

    /// Verify that validate_coherence works through CodegenContext
    #[test]
    fn test_codegen_context_coherence_validation() {
        with_ctx!(ctx, {
            ctx.register_type_home("Foo".to_string(), "user.data".to_string());
            ctx.register_trait_home("Eq".to_string(), "std.eq".to_string());
            
            // Legal impl
            let result = ctx.register_trait_impl(
                "Foo".to_string(), "Eq".to_string(), "user.data".to_string()
            );
            assert!(result.is_ok());
            assert!(ctx.validate_coherence().is_ok());
        });
    }

    /// Verify that orphan detection works through CodegenContext
    #[test]
    fn test_codegen_context_orphan_detection() {
        with_ctx!(ctx, {
            ctx.register_type_home("String".to_string(), "std.string".to_string());
            ctx.register_trait_home("Eq".to_string(), "std.eq".to_string());
            
            // Orphan impl (user.main owns neither String nor Eq)
            let _ = ctx.register_trait_impl(
                "String".to_string(), "Eq".to_string(), "user.main".to_string()
            );
            
            let result = ctx.validate_coherence();
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("Orphan Implementation"));
        });
    }

    // =========================================================================
    // Phase 5: Edge Cases
    // =========================================================================

    /// Empty module (no types, no traits) should pass coherence trivially
    #[test]
    fn test_empty_state_passes_coherence() {
        let file: SaltFile = syn::parse_str("fn main() {}").expect("valid salt file");
        let state = make_disc_state(&file);
        assert!(state.validate_coherence().is_ok());
    }

    /// Implementing a trait for a type where neither is registered in origins
    /// (bootstrap edge case — should pass since we can't determine violation)
    #[test]
    fn test_unregistered_origins_default_behavior() {
        let file: SaltFile = syn::parse_str("fn main() {}").expect("valid salt file");
        let mut state = make_disc_state(&file);
        // Neither type nor trait registered in origins
        let _ = state.register_trait_impl(
            "Unknown".to_string(), "Unknown".to_string(), "some.module".to_string()
        );
        
        // Should fail coherence because neither home is verified
        let result = state.validate_coherence();
        assert!(result.is_err(), "Unregistered origins should fail coherence (can't prove ownership)");
    }
}
