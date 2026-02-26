/// Pointer Indexing Unit Tests
/// 
/// These tests isolate the "Usize to Pointer" promotion issue into distinct,
/// testable behaviors to enable deterministic troubleshooting.
/// 
/// Key behaviors being tested:
/// 1. Loop induction variable registration as Type::Usize
/// 2. emit_lvalue for Pointer indexing returns element type (F32), not Pointer
/// 3. emit_assign uses peeled element type for RHS hint  
/// 4. promote_numeric rejects Integer→Pointer promotion

use salt_front::grammar::*;
use salt_front::codegen::context::{CodegenContext, LocalKind};
use salt_front::codegen::stmt::emit_stmt;
use salt_front::codegen::expr::emit_expr;
use salt_front::codegen::type_bridge::promote_numeric;
use salt_front::types::Type;
use std::collections::{BTreeMap, HashMap};

macro_rules! with_ctx {
    ($ctx:ident, $block:block) => {
        let code = "fn main() {}";
        let file: SaltFile = syn::parse_str(code).unwrap();
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        let $ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        $block
    };
}

// ============================================================================
// Test 1: Loop Induction Variable Registration
// Verifies that for-loop induction variables are registered as Type::Usize
// ============================================================================

#[test]
fn test_loop_iv_registered_as_usize() {
    // A simple for loop should register 'i' as Type::Usize in body_vars
    let code = r#"
        fn test() {
            for i in 0..10 {
                let x = i;
            }
        }
    "#;
    let file: SaltFile = syn::parse_str(code).unwrap();
    let z3_cfg = z3::Config::new();
    let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    
    let mut out = String::new();
    let mut locals = BTreeMap::new();
    
    let func = match &file.items[0] {
        Item::Fn(f) => f,
        _ => panic!("Expected function"),
    };
    
    for stmt in &func.body.stmts {
        emit_stmt(&ctx, &mut out, stmt, &mut locals).unwrap();
    }
    
    // The loop should compile without errors
    // If IV was registered as Pointer, we'd get a promotion error
    assert!(out.contains("affine.for"), "Expected affine.for: {}", out);
}

// ============================================================================
// Test 2: Pointer Indexing in Assignment Returns Element Type
// Verifies that w[i] = value assignments correctly resolve element type
// ============================================================================

#[test]
fn test_pointer_index_assignment_compiles() {
    // Pointer indexing in assignment should use element type (f32), not Pointer
    let code = r#"
        fn test(w: Ptr<f32>) {
            for i in 0..10 {
                w[i] = 0.05;
            }
        }
    "#;
    let file: SaltFile = syn::parse_str(code).unwrap();
    let z3_cfg = z3::Config::new();
    let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    
    let mut out = String::new();
    let mut locals = BTreeMap::new();
    
    // Register 'w' as Ptr<f32>
    locals.insert("w".to_string(), (
        Type::Pointer { 
            element: Box::new(Type::F32), 
            provenance: salt_front::types::Provenance::Naked,
            is_mutable: true 
        }, 
        LocalKind::SSA("%w".to_string())
    ));
    
    let func = match &file.items[0] {
        Item::Fn(f) => f,
        _ => panic!("Expected function"),
    };
    
    // This should NOT fail with "Usize to Pointer" error
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for stmt in &func.body.stmts {
            emit_stmt(&ctx, &mut out, stmt, &mut locals).unwrap();
        }
    }));
    
    assert!(result.is_ok(), "Pointer indexing assignment should compile without error");
}

// ============================================================================
// Test 3: promote_numeric Rejects Integer to Pointer
// Verifies the Constitutional Guard blocks Integer→Pointer promotion
// ============================================================================

#[test]
fn test_promote_numeric_rejects_integer_to_pointer() {
    with_ctx!(ctx, {
        let mut out = String::new();
        
        let pointer_type = Type::Pointer { 
            element: Box::new(Type::F32), 
            provenance: salt_front::types::Provenance::Naked,
            is_mutable: true 
        };
        
        // Attempting to promote Usize to Pointer should FAIL
        let result = promote_numeric(&ctx, &mut out, "%idx", &Type::Usize, &pointer_type);
        
        assert!(result.is_err(), "promote_numeric should reject Usize to Pointer promotion");
        
        let err_msg = result.unwrap_err();
        assert!(err_msg.contains("Sovereign Type Error") || err_msg.contains("Cannot promote"),
            "Error message should indicate type safety violation: {}", err_msg);
    });
}

#[test]
fn test_promote_numeric_allows_usize_to_i64() {
    with_ctx!(ctx, {
        let mut out = String::new();
        
        // Promoting Usize to I64 should succeed (valid index cast)
        let result = promote_numeric(&ctx, &mut out, "%idx", &Type::Usize, &Type::I64);
        
        assert!(result.is_ok(), "promote_numeric should allow Usize to I64: {:?}", result);
    });
}

// ============================================================================
// Test 4: Loop with Pointer Read (R-value indexing)
// Verifies that reading from pointer in loop doesn't contaminate IV
// ============================================================================

#[test]
fn test_pointer_read_in_loop_compiles() {
    // Reading from pointer (R-value) should also work without IV contamination
    let code = r#"
        fn test(arr: Ptr<f32>) -> f32 {
            let mut sum: f32 = 0.0;
            for i in 0..10 {
                sum = sum + arr[i];
            }
            return sum;
        }
    "#;
    let file: SaltFile = syn::parse_str(code).unwrap();
    let z3_cfg = z3::Config::new();
    let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    
    let mut out = String::new();
    let mut locals = BTreeMap::new();
    
    // Register 'arr' as Ptr<f32>
    locals.insert("arr".to_string(), (
        Type::Pointer { 
            element: Box::new(Type::F32), 
            provenance: salt_front::types::Provenance::Naked,
            is_mutable: false 
        }, 
        LocalKind::SSA("%arr".to_string())
    ));
    
    let func = match &file.items[0] {
        Item::Fn(f) => f,
        _ => panic!("Expected function"),
    };
    
    // This should NOT fail with "Usize to Pointer" error
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for stmt in &func.body.stmts {
            emit_stmt(&ctx, &mut out, stmt, &mut locals).unwrap();
        }
    }));
    
    assert!(result.is_ok(), "Pointer read in loop should compile without IV contamination");
}

// ============================================================================
// Test 5: Mixed Pointer Read/Write in Same Loop
// The exact pattern causing the failure in init_xavier
// ============================================================================

#[test]
fn test_init_xavier_pattern_compiles() {
    // This is the exact pattern from sovereign_train.salt init_xavier
    let code = r#"
        fn init_xavier(w: Ptr<f32>, size: i64) {
            for i in 0..size {
                w[i] = 0.05;
            }
        }
    "#;
    let file: SaltFile = syn::parse_str(code).unwrap();
    let z3_cfg = z3::Config::new();
    let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    
    let mut out = String::new();
    let mut locals = BTreeMap::new();
    
    let func = match &file.items[0] {
        Item::Fn(f) => f,
        _ => panic!("Expected function"),
    };
    
    // Register parameters
    locals.insert("w".to_string(), (
        Type::Pointer { 
            element: Box::new(Type::F32), 
            provenance: salt_front::types::Provenance::Naked,
            is_mutable: true 
        }, 
        LocalKind::SSA("%w".to_string())
    ));
    locals.insert("size".to_string(), (Type::I64, LocalKind::SSA("%size".to_string())));
    
    // This is the critical test - should NOT fail with "Usize to Pointer" error
    let mut compile_error: Option<String> = None;
    for stmt in &func.body.stmts {
        match emit_stmt(&ctx, &mut out, stmt, &mut locals) {
            Ok(_) => {},
            Err(e) => {
                compile_error = Some(e);
                break;
            }
        }
    }
    
    assert!(compile_error.is_none(), 
        "init_xavier pattern should compile without error. Got: {:?}", compile_error);
}
