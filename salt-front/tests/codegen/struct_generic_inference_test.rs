// ============================================================================
// Struct-Level Generic Inference Tests
// Guards against regression: Ptr::empty() without turbofish must infer T
// from return type context.
//
// Root Cause Fixed:
// - unify_generics only checked function-level generics, missing struct-level
//   generics from impl<T> blocks. Static methods like Ptr::empty() have T
//   as a struct-level generic, requiring inference from return type context.
// ============================================================================

#[cfg(test)]
mod tests {
    use salt_front::codegen::context::CodegenContext;
    use salt_front::types::Type;
    use salt_front::grammar::SaltFile;
    use salt_front::codegen::expr::resolver::CallSiteResolver;
    use std::collections::{BTreeMap, HashMap};

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

    // ========================================================================
    // Test 1: unify_types correctly maps T from Ptr<T> ↔ Ptr<Node>
    // ========================================================================

    /// The core building block: unifying Concrete("Ptr", [Generic("T")]) with
    /// Concrete("Ptr", [Struct("Node")]) should produce map: {"T" => Struct("Node")}
    #[test]
    fn test_unify_types_extracts_struct_generic_from_concrete() {
        with_ctx!(ctx, {
            ctx.with_lowering_ctx(|lctx| {
                let mut resolver = CallSiteResolver::new(lctx);
            
                // Pattern: Ptr<T> (the template return type)
                let pattern = Type::Concrete(
                    "std__core__ptr__Ptr".to_string(),
                    vec![Type::Generic("T".to_string())]
                );
            
                // Concrete: Ptr<Node> (the expected return type from context)
                let concrete = Type::Concrete(
                    "std__core__ptr__Ptr".to_string(),
                    vec![Type::Struct("main__Node".to_string())]
                );
            
                let mut map = BTreeMap::new();
                let result = resolver.unify_types(&pattern, &concrete, &mut map);
            
                assert!(result.is_ok(), "Unification should succeed: {:?}", result.err());
                assert_eq!(
                    map.get("T"),
                    Some(&Type::Struct("main__Node".to_string())),
                    "T should be bound to Node. Map: {:?}",
                    map
                );
                Ok::<(), String>(())
            }).unwrap();
        });
    }

    // ========================================================================
    // Test 2: infer_from_return_context solves struct-level T
    // ========================================================================

    /// When template return type is Ptr<T> and expected is Ptr<i32>,
    /// infer_from_return_context should return Some(i32) for generic "T".
    #[test]
    fn test_infer_from_return_context_solves_struct_generic() {
        with_ctx!(ctx, {
            ctx.with_lowering_ctx(|lctx| {
                let mut resolver = CallSiteResolver::new(lctx);
            
                // Build a mock template for empty(): fn empty() -> Ptr<T>
                let template: salt_front::grammar::SaltFn = syn::parse_str(
                    "fn empty() -> Ptr<T> {}"
                ).expect("valid fn");
            
                let expected = Type::Concrete(
                    "std__core__ptr__Ptr".to_string(),
                    vec![Type::I32]
                );
            
                let result = resolver.infer_from_return_context("T", &template, Some(&expected));
            
                assert!(
                    result.is_some(),
                    "Should infer T from return type context. Got None."
                );
                assert_eq!(
                    result.unwrap(),
                    Type::I32,
                    "T should be inferred as i32"
                );
                Ok::<(), String>(())
            }).unwrap();
        });
    }

    // ========================================================================
    // Test 3: verify_completeness handles struct-level generics
    // ========================================================================

    /// The critical gap: verify_completeness must check struct-level generics
    /// (from self_ty) AND infer them from return type, not just function-level.
    #[test]
    fn test_verify_completeness_infers_struct_generics_from_return_type() {
        with_ctx!(ctx, {
            ctx.with_lowering_ctx(|lctx| {
                let mut resolver = CallSiteResolver::new(lctx);
            
                // Template: fn empty() -> Ptr<T> (NO function-level generics)
                let template: salt_front::grammar::SaltFn = syn::parse_str(
                    "fn empty() -> Ptr<T> {}"
                ).expect("valid fn");
            
                // self_ty: Concrete("Ptr", [Generic("T")]) — the struct-level generic
                let self_ty = Some(Type::Concrete(
                    "std__core__ptr__Ptr".to_string(),
                    vec![Type::Generic("T".to_string())]
                ));
            
                // Expected return type from context: Ptr<i32>
                let expected = Type::Concrete(
                    "std__core__ptr__Ptr".to_string(),
                    vec![Type::I32]
                );
            
                let mut map = BTreeMap::new();
                let result = resolver.verify_completeness_with_struct_generics(
                    &template,
                    &mut map,
                    Some(&expected),
                    self_ty.as_ref()
                );
            
                assert!(result.is_ok(), "Should resolve T from return context. Error: {:?}", result.err());
                assert_eq!(
                    map.get("T"),
                    Some(&Type::I32),
                    "T should be bound to i32. Map: {:?}",
                    map
                );
                Ok::<(), String>(())
            }).unwrap();
        });
    }

    // ========================================================================
    // Test 4: Turbofish still works — explicit generics take priority
    // ========================================================================

    #[test]
    fn test_explicit_turbofish_takes_priority_over_inference() {
        with_ctx!(ctx, {
            ctx.with_lowering_ctx(|lctx| {
                let mut resolver = CallSiteResolver::new(lctx);
            
                let template: salt_front::grammar::SaltFn = syn::parse_str(
                    "fn empty() -> Ptr<T> {}"
                ).expect("valid fn");
            
                let self_ty = Some(Type::Concrete(
                    "std__core__ptr__Ptr".to_string(),
                    vec![Type::Generic("T".to_string())]
                ));
            
                let expected = Type::Concrete(
                    "std__core__ptr__Ptr".to_string(),
                    vec![Type::I64]
                );
            
                // Pre-populate map as if turbofish was used: T = i32
                let mut map = BTreeMap::new();
                map.insert("T".to_string(), Type::I32);
            
                let result = resolver.verify_completeness_with_struct_generics(
                    &template,
                    &mut map,
                    Some(&expected),
                    self_ty.as_ref()
                );
            
                assert!(result.is_ok(), "Should succeed with explicit binding");
                // Turbofish binding should remain
                assert_eq!(
                    map.get("T"),
                    Some(&Type::I32),
                    "Explicit turbofish T=i32 should take priority over inferred T=i64"
                );
                Ok::<(), String>(())
            }).unwrap();
        });
    }

    // ========================================================================
    // Test 5: No context = still errors (won't silently produce broken code)
    // ========================================================================

    #[test]
    fn test_no_context_no_turbofish_still_errors() {
        with_ctx!(ctx, {
            ctx.with_lowering_ctx(|lctx| {
                let mut resolver = CallSiteResolver::new(lctx);
            
                let template: salt_front::grammar::SaltFn = syn::parse_str(
                    "fn empty() -> Ptr<T> {}"
                ).expect("valid fn");
            
                let self_ty = Some(Type::Concrete(
                    "std__core__ptr__Ptr".to_string(),
                    vec![Type::Generic("T".to_string())]
                ));
            
                let mut map = BTreeMap::new();
                let result = resolver.verify_completeness_with_struct_generics(
                    &template,
                    &mut map,
                    None,  // No expected type!
                    self_ty.as_ref()
                );
            
                assert!(
                    result.is_err(),
                    "Should error when T is completely unconstrained (no turbofish, no context)"
                );
                Ok::<(), String>(())
            }).unwrap();
        });
    }
}
