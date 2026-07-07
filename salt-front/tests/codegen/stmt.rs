use saltc::compile;

#[test]
fn test_release_mode() {
    let code = r#"
fn main() -> i32 { return 42; }
"#;
    // Test both debug and release modes
    let debug_result = compile(code, false, None, true);
    let release_result = compile(code, true, None, true);
    assert!(debug_result.is_ok());
    assert!(release_result.is_ok());
}

#[test]
fn test_codegen_error_paths() {
    // Test cases that should trigger codegen errors
    // (Note: Some may be caught by parser first)
    
    // Unknown variable - may be caught by codegen
    let result = compile("fn main() -> i32 { return unknown_var; }", false, None, true);
    assert!(result.is_err());
    
    // Field on non-struct
    let result = compile("fn main() -> i32 { let x = 42; return x.field; }", false, None, true);
    assert!(result.is_err());
}

#[test]
fn test_alloca_hoisting_invariant() {
    let code = r#"
        fn main() -> i32 {
            let mut i: i32 = 0;
            let mut sum: i32 = 0;
            while i < 10 {
                let local_val: i32 = i * 2;
                sum = sum + local_val;
                i = i + 1;
            }
            return sum;
        }
    "#;

    let result = compile(code, false, None, true).expect("Compilation failed");
    
    // The invariant: No llvm.alloca between loop header and loop exit.
    // We search for ^while_header and ^while_exit and ensure no "llvm.alloca" exists between them.
    
    let lines: Vec<&str> = result.lines().collect();
    let mut in_loop = false;
    let mut alloca_in_loop = false;

    for line in lines {
        if line.contains("^while_header_") {
            in_loop = true;
        }
        if line.contains("^while_exit_") {
            in_loop = false;
        }
        if in_loop && line.contains("llvm.alloca") {
            alloca_in_loop = true;
            eprintln!("Violation: alloca found in loop: {}", line);
        }
    }

    assert!(!alloca_in_loop, "Alloca Hoisting Invariant violated: llvm.alloca found inside while loop body");
}

#[test]
fn test_nested_alloca_hoisting() {
    let code = r#"
        fn main() -> i32 {
            let mut i: i32 = 0;
            while i < 5 {
                let mut j: i32 = 0;
                while j < 5 {
                    let nested_val: i32 = i + j;
                    j = j + 1;
                }
                i = i + 1;
            }
            return 0;
        }
    "#;

    let result = compile(code, false, None, true).expect("Compilation failed");
    
    let lines: Vec<&str> = result.lines().collect();
    let mut in_loop_depth = 0;
    let mut alloca_in_loop = false;

    for line in lines {
        if line.contains("^while_header_") {
            in_loop_depth += 1;
        }
        if line.contains("^while_exit_") {
            in_loop_depth -= 1;
        }
        if in_loop_depth > 0 && line.contains("llvm.alloca") {
            alloca_in_loop = true;
            eprintln!("Violation: alloca found in nested loop: {}", line);
        }
    }

    assert!(!alloca_in_loop, "Alloca Hoisting Invariant violated in nested loops");
}

#[test]
fn test_block_expression() {
    let code = r#"
        fn main() -> i32 {
            let x: i32 = {
                let a: i32 = 10;
                let b: i32 = 20;
                a + b
            };
            return x;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Block expr failed: {:?}", result.err());
}

#[test]
fn test_if_else() {
    let code = r#"
        fn main() -> i32 {
            let x: i32 = 5;
            if x > 3 {
                return 1;
            } else {
                return 0;
            }
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "If-else failed: {:?}", result.err());
}

#[test]
fn test_while_loop() {
    let code = r#"
        fn main() -> i32 {
            let mut x: i32 = 0;
            while x < 10 {
                x = x + 1;
            }
            return x;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "While loop failed: {:?}", result.err());
}

#[test]
fn test_for_loop() {
    let code = r#"
        fn main() -> i32 {
            let mut sum: i32 = 0;
            for i in 0..10 {
                sum = sum + 1;
            }
            return sum;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "For loop failed: {:?}", result.err());
}

#[test]
fn test_break_in_loop() {
    let code = r#"
        fn main() -> i32 {
            let mut i: i32 = 0;
            while i < 100 {
                i = i + 1;
                if i == 10 {
                    break;
                }
            }
            return i;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Break failed: {:?}", result.err());
}

#[test]
fn test_region_block() {
    let code = r#"
        fn main() -> i32 {
            region("critical") {
                let x: i32 = 42;
            }
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "region block failed: {:?}", result.err());
}

#[test]
fn test_deeply_nested_if() {
    let code = r#"
        fn main() -> i32 {
            let x: i32 = 5;
            if x > 0 {
                if x > 2 {
                    if x > 4 {
                        return 3;
                    }
                    return 2;
                }
                return 1;
            }
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "nested if failed: {:?}", result.err());
}

#[test]
fn test_nested_while_loops() {
    let code = r#"
        fn main() -> i32 {
            let mut sum: i32 = 0;
            let mut i: i32 = 0;
            while i < 3 {
                let mut j: i32 = 0;
                while j < 3 {
                    sum = sum + 1;
                    j = j + 1;
                }
                i = i + 1;
            }
            return sum;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "nested while failed: {:?}", result.err());
}

// ============================================================================
// Dialect Routing Tests (Critical for MatMul 3.4x and merge_sorted_lists fix)
// Tests that while loops trigger cf.br fallback, nested for stays affine
// ============================================================================

#[test]
fn test_while_inside_for_uses_cf_fallback() {
    // The merge_sorted_lists pattern: for i in 0..n { while ptr != null { ... } }
    // Should NOT use affine.for since while creates cf.br blocks
    let code = r#"
        fn main() -> i32 {
            for i in 0..10 {
                while true {
                    break;
                }
            }
            return 0;
        }
    "#;
    let result = compile(code, false, None, true).expect("Compilation failed");
    
    // Outer for should NOT be affine.for (while loop inside)
    assert!(!result.contains("affine.for"), 
        "For loop with while inside should NOT use affine.for: {}", 
        result.lines().take(50).collect::<Vec<_>>().join("\n"));
}

#[test]
fn test_triple_nested_for_uses_affine() {
    // The MatMul pattern: pure nested for loops with no control flow
    // All three loops should use affine.for
    let code = r#"
        fn main() -> i32 {
            let mut sum: i32 = 0;
            for i in 0..10 {
                for j in 0..10 {
                    for k in 0..10 {
                        sum = sum + 1;
                    }
                }
            }
            return sum;
        }
    "#;
    let result = compile(code, false, None, true).expect("Compilation failed");
    
    // All three loops should be affine.for
    let affine_count = result.matches("affine.for").count();
    assert!(affine_count >= 3, 
        "Expected 3 affine.for for triple-nested loops, got {}: {}",
        affine_count, result.lines().take(50).collect::<Vec<_>>().join("\n"));
}

#[test]
fn test_deep_while_propagates_fallback() {
    // Nested for loops where innermost contains while - ALL should fall back
    let code = r#"
        fn main() -> i32 {
            for i in 0..5 {
                for j in 0..5 {
                    while true {
                        break;
                    }
                }
            }
            return 0;
        }
    "#;
    let result = compile(code, false, None, true).expect("Compilation failed");
    
    // No affine.for should be present
    assert!(!result.contains("affine.for"), 
        "No affine.for when while is nested deep inside: {}",
        result.lines().take(50).collect::<Vec<_>>().join("\n"));
}

#[test]
fn test_for_with_if_uses_cf_fallback() {
    // For loop with if statement should use cf.br fallback
    let code = r#"
        fn main() -> i32 {
            let mut sum: i32 = 0;
            for i in 0..10 {
                if i > 5 {
                    sum = sum + 1;
                }
            }
            return sum;
        }
    "#;
    let result = compile(code, false, None, true).expect("Compilation failed");
    
    // Should NOT use affine.for (if statement creates blocks)
    assert!(!result.contains("affine.for"), 
        "For loop with if inside should NOT use affine.for");
}

