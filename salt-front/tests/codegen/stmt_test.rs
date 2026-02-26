use salt_front::grammar::*;
use salt_front::codegen::context::{CodegenContext, LocalKind};
use salt_front::codegen::stmt::emit_stmt;
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

#[test]
fn test_label_resolution_stress() {
    with_ctx!(ctx, {
        let mut out = String::new();
        let mut locals = BTreeMap::new();
        
        // 11 levels of nested while loops (no labels since Salt parser/codegen doesn't support them yet)
        // This still exercises the break/continue label stack push/pop.
        let mut code = "while true { break; }".to_string();
        for _ in 0..10 {
            code = format!("while true {{ {} }}", code);
        }
        
        let file_code = format!("fn stress() {{ {} }}", code);
        let file: SaltFile = syn::parse_str(&file_code).unwrap();
        let func = match &file.items[0] {
            Item::Fn(f) => f,
            _ => panic!("Expected function"),
        };
        
        for stmt in &func.body.stmts {
            emit_stmt(&ctx, &mut out, stmt, &mut locals).unwrap();
        }
        
        assert!(out.contains("cf.br"));
    });
}

#[test]
fn test_early_return_matrix() {
    with_ctx!(ctx, {
        let mut out = String::new();
        let mut locals = BTreeMap::new();
        
        // Register 'x' so it's defined
        locals.insert("x".to_string(), (Type::I32, LocalKind::SSA("%x".to_string())));
        
        // Large if/else if/... chain where every branch returns
        let mut code = "if x == 0 { return 0; }".to_string();
        for i in 1..15 {
            code = format!("{} else if x == {} {{ return {}; }}", code, i, i);
        }
        code = format!("{} else {{ return 99; }}", code);
        
        let file_code = format!("fn matrix(x: i32) -> i32 {{ {} }}", code);
        let file: SaltFile = syn::parse_str(&file_code).unwrap();
        let func = match &file.items[0] {
            Item::Fn(f) => f,
            _ => panic!("Expected function"),
        };
        
        for stmt in &func.body.stmts {
            emit_stmt(&ctx, &mut out, stmt, &mut locals).unwrap();
        }
        
        // Check for return emission
        assert!(out.contains("func.return"));
    });
}

#[test]
fn test_nested_if_expr_stmt() {
    with_ctx!(ctx, {
        let mut out = String::new();
        let mut locals = BTreeMap::new();
        
        // Test an if expression as a statement
        let code = "let x = if true { 1 } else { 2 };";
        let syn_stmt: syn::Stmt = syn::parse_str(code).unwrap();
        let stmt = Stmt::Syn(syn_stmt);
        
    emit_stmt(&ctx, &mut out, &stmt, &mut locals).unwrap();
        assert!(out.contains("cf.cond_br"));
    });
}

// ============================================================================
// Control Flow Detection Tests  
// Guards against using affine.for with if-expressions (creates multiple blocks)
// ============================================================================

#[test]
fn test_affine_for_with_simple_body_allowed() {
    // Simple loop body without if should use affine.for
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
    
    // Should use affine.for for simple body
    assert!(out.contains("affine.for"), "Expected affine.for for simple loop body: {}", out);
}

#[test]
fn test_affine_for_with_if_expr_falls_back_to_cf() {
    // Loop with if expression in let binding should NOT use affine.for
    let code = r#"
        fn test() {
            for i in 0..10 {
                let x = if i > 5 { i } else { 0 };
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
    
    // Should NOT use affine.for when if expression is present
    assert!(!out.contains("affine.for"), "affine.for should not be used with if expressions: {}", out);
    // Should use cf.br loop instead
    assert!(out.contains("cf.br") || out.contains("cf.cond_br"), "Expected cf control flow: {}", out);
}

#[test]
fn test_affine_for_with_if_stmt_falls_back_to_cf() {
    // Loop with if statement should NOT use affine.for
    let code = r#"
        fn test() {
            for i in 0..10 {
                if i > 5 {
                    let x = i;
                }
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
    
    // Should NOT use affine.for when if statement is present
    assert!(!out.contains("affine.for"), "affine.for should not be used with if statements: {}", out);
}

// ============================================================================
// While Loop Detection Tests (Critical for merge_sorted_lists fix)
// Guards against using affine.for with while loops (creates cf.br blocks)
// ============================================================================

#[test]
fn test_while_loop_inside_for_falls_back_to_cf() {
    // For loop containing while loop should NOT use affine.for
    // This is the merge_sorted_lists pattern: for i in 0..n { while ptr != null { ... } }
    let code = r#"
        fn test() {
            for i in 0..10 {
                while true {
                    break;
                }
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
    
    // Should NOT use affine.for when while loop is present
    assert!(!out.contains("affine.for"), "affine.for should not be used with while loops inside: {}", out);
    // Should use cf.br loop for outer for
    assert!(out.contains("cf.br") || out.contains("cf.cond_br"), "Expected cf control flow: {}", out);
}

#[test]
fn test_nested_for_loops_stay_in_affine() {
    // Triple-nested for loops with simple body should ALL use affine.for
    // This is the MatMul pattern
    let code = r#"
        fn test() {
            for i in 0..10 {
                for j in 0..10 {
                    for k in 0..10 {
                        let x = 1;
                    }
                }
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
    
    // Should use affine.for for all three loops (count occurrences)
    let affine_count = out.matches("affine.for").count();
    assert!(affine_count >= 3, "Expected 3 affine.for loops for perfect nesting, got {}: {}", affine_count, out);
}

#[test]
fn test_nested_for_with_deep_while_falls_back() {
    // Nested for loops where innermost contains while should fall back ALL the way
    let code = r#"
        fn test() {
            for i in 0..10 {
                for j in 0..10 {
                    while true {
                        break;
                    }
                }
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
    
    // Should NOT use affine.for anywhere since while is nested deep inside
    assert!(!out.contains("affine.for"), "affine.for should not be used when while is nested inside: {}", out);
}

#[test]
fn test_standalone_while_emits_cf() {
    // Standalone while loop should emit cf.br/cf.cond_br
    let code = r#"
        fn test() {
            while true {
                break;
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
    
    // Should emit cf control flow for while loop
    assert!(out.contains("cf.br") || out.contains("cf.cond_br"), "Expected cf control flow for while: {}", out);
}

