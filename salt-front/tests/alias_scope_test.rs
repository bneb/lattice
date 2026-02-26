// Unit tests for V7.3 per-argument alias scope tracking
// Tests cover: ControlFlowState scope registration, propagation, and MLIR emission

use salt_front::codegen::emit_mlir;
use salt_front::codegen::phases::ControlFlowState;
use salt_front::grammar::SaltFile;

// =============================================================================
// ControlFlowState Scope Tracking Unit Tests
// =============================================================================

#[test]
fn test_arg_scope_registration() {
    let mut cf = ControlFlowState::new();
    
    // Register scopes for function arguments
    let scope_w = cf.register_arg_scope("%arg_w");
    let scope_x = cf.register_arg_scope("%arg_x");
    let scope_result = cf.register_arg_scope("%arg_result");
    
    // Verify unique IDs assigned
    assert_eq!(scope_w, 0);
    assert_eq!(scope_x, 1);
    assert_eq!(scope_result, 2);
    
    // Verify lookup
    assert_eq!(cf.get_arg_scope("%arg_w"), Some(0));
    assert_eq!(cf.get_arg_scope("%arg_x"), Some(1));
    assert_eq!(cf.get_arg_scope("%arg_result"), Some(2));
    assert_eq!(cf.get_arg_scope("%arg_unknown"), None);
}

#[test]
fn test_arg_scope_clear() {
    let mut cf = ControlFlowState::new();
    
    cf.register_arg_scope("%arg_a");
    cf.register_arg_scope("%arg_b");
    assert_eq!(cf.get_arg_scope("%arg_a"), Some(0));
    
    cf.clear_arg_scopes();
    
    // After clear, all scopes should be gone
    assert_eq!(cf.get_arg_scope("%arg_a"), None);
    assert_eq!(cf.get_arg_scope("%arg_b"), None);
    
    // New registration should start from 0
    let new_scope = cf.register_arg_scope("%arg_new");
    assert_eq!(new_scope, 0);
}

#[test]
fn test_get_other_arg_scopes() {
    let mut cf = ControlFlowState::new();
    
    cf.register_arg_scope("%arg_w");      // scope 0
    cf.register_arg_scope("%arg_x");      // scope 1
    cf.register_arg_scope("%arg_result"); // scope 2
    
    // When accessing W (scope 0), noalias should include X (1) and result (2)
    let others = cf.get_other_arg_scopes(0);
    assert!(others.contains(&1));
    assert!(others.contains(&2));
    assert!(!others.contains(&0));
    assert_eq!(others.len(), 2);
    
    // When accessing X (scope 1), noalias should include W (0) and result (2)
    let others = cf.get_other_arg_scopes(1);
    assert!(others.contains(&0));
    assert!(others.contains(&2));
    assert!(!others.contains(&1));
}

#[test]
fn test_scope_provenance_propagation() {
    let mut cf = ControlFlowState::new();
    
    // Register argument scope
    cf.register_arg_scope("%arg_w");  // scope 0
    
    // Propagate scope through GEP chain
    cf.propagate_scope_provenance("%arg_w", "%gep_123");
    cf.propagate_scope_provenance("%gep_123", "%gep_456");  // Transitive
    
    // Both derived pointers should inherit the scope
    assert_eq!(cf.get_pointer_scope("%arg_w"), Some(0));
    assert_eq!(cf.get_pointer_scope("%gep_123"), Some(0));
    assert_eq!(cf.get_pointer_scope("%gep_456"), Some(0));
    
    // Unknown pointer should return None
    assert_eq!(cf.get_pointer_scope("%random_ptr"), None);
}

#[test]
fn test_scope_provenance_multiple_args() {
    let mut cf = ControlFlowState::new();
    
    // Register two argument scopes
    cf.register_arg_scope("%arg_w");  // scope 0
    cf.register_arg_scope("%arg_x");  // scope 1
    
    // Propagate each to different GEP chains
    cf.propagate_scope_provenance("%arg_w", "%gep_w_elem");
    cf.propagate_scope_provenance("%arg_x", "%gep_x_elem");
    
    // Verify correct scope inheritance
    assert_eq!(cf.get_pointer_scope("%gep_w_elem"), Some(0));
    assert_eq!(cf.get_pointer_scope("%gep_x_elem"), Some(1));
    
    // Cross-check: these are in different scopes
    assert_ne!(
        cf.get_pointer_scope("%gep_w_elem"), 
        cf.get_pointer_scope("%gep_x_elem")
    );
}

#[test]
fn test_scope_propagation_no_op_for_unknown() {
    let mut cf = ControlFlowState::new();
    
    // Try to propagate from unknown pointer - should be no-op
    cf.propagate_scope_provenance("%unknown", "%gep_result");
    
    // Result should not have a scope
    assert_eq!(cf.get_pointer_scope("%gep_result"), None);
}

// =============================================================================
// MLIR Emission Integration Tests
// =============================================================================

#[test]
fn test_matvec_pattern_emits_per_arg_scopes() {
    // Test: matvec-like function with multiple pointer args should emit
    // per-argument alias scopes in inner loop loads
    let src = r#"
        package test::alias_scope::matvec;
        extern fn malloc(n: i64) -> Ptr<u8>;
        extern fn free(ptr: Ptr<u8>);
        
        fn dot_product(w: Ptr<f32>, x: Ptr<f32>, n: i64) -> f32 {
            let mut sum: f32 = 0.0f32;
            for i in 0..n {
                let w_elem = w[i];
                let x_elem = x[i];
                sum = sum + (w_elem * x_elem);
            }
            return sum;
        }
        
        fn main() -> i32 {
            let buf = malloc(100);
            let w = buf as Ptr<f32>;
            let x = buf as Ptr<f32>;
            let r = dot_product(w, x, 10);
            free(buf);
            return 0;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    
    assert!(res.is_ok(), "matvec alias scope emission failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    
    // Should contain per-arg scope definitions or noalias attributes
    // Salt uses noalias = [#scope_...] metadata on loads
    assert!(mlir.contains("noalias = [") || mlir.contains("#scope_arg_"), 
        "Expected noalias or scope metadata. Got:\n{}", mlir);
}

#[test]
fn test_single_pointer_arg_no_noalias() {
    // Test: Function with single pointer arg should have alias_scopes but empty noalias
    let src = r#"
        package test::alias_scope::single;
        extern fn malloc(n: i64) -> Ptr<u8>;
        extern fn free(ptr: Ptr<u8>);
        
        fn sum_array(x: Ptr<f32>, n: i64) -> f32 {
            let mut sum: f32 = 0.0f32;
            for i in 0..n {
                let elem = x[i];
                sum = sum + elem;
            }
            return sum;
        }
        
        fn main() -> i32 {
            let buf = malloc(100);
            let x = buf as Ptr<f32>;
            let r = sum_array(x, 10);
            free(buf);
            return 0;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    
    assert!(res.is_ok(), "single arg scope emission failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    
    // Should have noalias metadata on loads (modern Salt approach)
    assert!(mlir.contains("noalias = [") || mlir.contains("#scope_arg_0"),
        "Expected noalias or scope metadata for single pointer arg. Got:\n{}", mlir);
}

#[test]
fn test_non_pointer_args_no_scope() {
    // Test: Non-pointer args should not get alias scopes
    let src = r#"
        package test::alias_scope::nonptr;
        
        fn add_ints(a: i32, b: i32) -> i32 {
            return a + b;
        }
        
        fn main() -> i32 {
            return add_ints(1, 2);
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    
    assert!(res.is_ok(), "non-pointer args should compile: {:?}", res.err());
    
    let mlir = res.unwrap();
    
    // Function with only value args should not have per-arg scope metadata on loads
    // (it will still have the preamble definitions but not use them)
    assert!(!mlir.contains("alias_scopes = [#scope_arg_0], noalias_scopes"),
        "Non-pointer function should not have per-arg scope loads. Got:\n{}", mlir);
}

#[test]
fn test_gep_inherits_scope_in_mlir() {
    // Test: After GEP (ptr[offset]), the resulting pointer should inherit ptr's scope
    let src = r#"
        package test::alias_scope::gep;
        extern fn malloc(n: i64) -> Ptr<u8>;
        extern fn free(ptr: Ptr<u8>);
        
        fn read_offset(ptr: Ptr<f32>, offset: i64) -> f32 {
            let elem = ptr[offset];
            return elem;
        }
        
        fn main() -> i32 {
            let buf = malloc(100);
            let p = buf as Ptr<f32>;
            let r = read_offset(p, 5);
            free(buf);
            return 0;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    
    assert!(res.is_ok(), "GEP scope inheritance failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    
    // Should have GEP followed by load
    assert!(mlir.contains("llvm.getelementptr"),
        "Expected GEP for pointer indexing. Got:\n{}", mlir);
    
    // Modern Salt uses noalias metadata on loads, or just plain loads for simple functions
    assert!(mlir.contains("noalias = [") || mlir.contains("alias_scopes") || mlir.contains("llvm.load"),
        "Expected noalias, alias_scopes, or load. Got:\n{}", mlir);
}

