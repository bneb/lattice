// =============================================================================
// TDD Test Suite: DWARF Line-Level Debug Info (P3)
//
// These tests verify that statement-level `loc("file":line:col)` annotations
// are emitted on terminal MLIR operations when debug info is enabled (-g).
//
// Pillar 1: Correctness — loc annotations must carry accurate source positions.
// Pillar 2: Precision — only terminal ops (return, branch, store) get locs,
//           not intermediate SSA computations.
// Pillar 3: Zero-cost — when debug_info is false, no loc annotations appear.
// =============================================================================

use salt_front::codegen::emit_mlir;
use salt_front::grammar::SaltFile;

/// Helper: compile Salt source with debug info enabled.
/// Returns the MLIR string on success.
fn compile_with_debug(src: &str) -> String {
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse test source");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, true, "test.salt");
    assert!(res.is_ok(), "Compilation failed: {:?}", res.err());
    res.unwrap()
}

/// Helper: compile Salt source WITHOUT debug info.
/// Returns the MLIR string on success.
fn compile_without_debug(src: &str) -> String {
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse test source");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    assert!(res.is_ok(), "Compilation failed: {:?}", res.err());
    res.unwrap()
}

// =============================================================================
// Test 1: Zero-cost principle — no locs when debug_info is false
// =============================================================================
#[test]
fn test_no_locs_without_debug_flag() {
    let src = r#"
        package test::no_debug;
        fn main() -> i32 {
            let x: i32 = 42;
            return x;
        }
    "#;
    let mlir = compile_without_debug(src);

    // No loc annotations should appear anywhere in the output
    assert!(!mlir.contains("loc(\""),
        "ZERO-COST VIOLATION: loc annotations present without -g flag.\nMLIR:\n{}", mlir);
}

// =============================================================================
// Test 2: Return statement gets loc annotation
// =============================================================================
#[test]
fn test_return_stmt_has_loc() {
    let src = r#"
        package test::ret;
        fn main() -> i32 {
            return 42;
        }
    "#;
    let mlir = compile_with_debug(src);

    // func.return must carry a loc annotation
    let return_lines: Vec<&str> = mlir.lines()
        .filter(|l| l.trim_start().starts_with("func.return"))
        .collect();

    assert!(!return_lines.is_empty(), "No func.return found in MLIR:\n{}", mlir);

    for line in &return_lines {
        assert!(line.contains("loc(\"test.salt\""),
            "MISSING LOC: func.return without loc annotation: {}", line);
    }
}

// =============================================================================
// Test 3: If statement cf.cond_br gets loc annotation
// =============================================================================
#[test]
fn test_if_stmt_cond_br_has_loc() {
    let src = r#"
        package test::if_loc;
        fn main() -> i32 {
            let x: i32 = 5;
            if x > 0 {
                return 1;
            }
            return 0;
        }
    "#;
    let mlir = compile_with_debug(src);

    // cf.cond_br for the if condition must carry a loc annotation
    let cond_br_lines: Vec<&str> = mlir.lines()
        .filter(|l| l.trim_start().starts_with("cf.cond_br"))
        .collect();

    assert!(!cond_br_lines.is_empty(), "No cf.cond_br found in MLIR:\n{}", mlir);

    for line in &cond_br_lines {
        assert!(line.contains("loc(\"test.salt\""),
            "MISSING LOC: cf.cond_br without loc annotation: {}", line);
    }
}

// =============================================================================
// Test 4: While loop cf.cond_br gets loc annotation
// =============================================================================
#[test]
fn test_while_loop_cond_br_has_loc() {
    let src = r#"
        package test::while_loc;
        fn main() -> i32 {
            let mut i: i32 = 0;
            let mut sum: i32 = 0;
            while i < 10 {
                sum = sum + i;
                i = i + 1;
            }
            return sum;
        }
    "#;
    let mlir = compile_with_debug(src);

    // The while condition branch must carry a loc
    let cond_br_lines: Vec<&str> = mlir.lines()
        .filter(|l| l.trim_start().starts_with("cf.cond_br"))
        .collect();

    assert!(!cond_br_lines.is_empty(), "No cf.cond_br found in MLIR:\n{}", mlir);

    for line in &cond_br_lines {
        assert!(line.contains("loc(\"test.salt\""),
            "MISSING LOC: while cf.cond_br without loc annotation: {}", line);
    }
}

// =============================================================================
// Test 5: For loop — return after loop gets loc
// =============================================================================
#[test]
fn test_for_loop_return_has_loc() {
    let src = r#"
        package test::for_loc;
        fn main() -> i32 {
            let mut total: i32 = 0;
            for i in 0..10 {
                total = total + i;
            }
            return total;
        }
    "#;
    let mlir = compile_with_debug(src);

    // The return statement must have a loc.
    let return_lines: Vec<&str> = mlir.lines()
        .filter(|l| l.trim_start().starts_with("func.return"))
        .collect();

    assert!(!return_lines.is_empty(), "No func.return found in MLIR:\n{}", mlir);

    for line in &return_lines {
        assert!(line.contains("loc(\"test.salt\""),
            "MISSING LOC: func.return after for loop without loc: {}", line);
    }
}

// =============================================================================
// Test 6: Multiple returns in different branches each get their own loc
// =============================================================================
#[test]
fn test_multiple_returns_each_have_loc() {
    let src = r#"
        package test::multi_ret;
        fn main() -> i32 {
            let x: i32 = 5;
            if x < 0 {
                return 0 - x;
            }
            return x;
        }
    "#;
    let mlir = compile_with_debug(src);

    let return_lines: Vec<&str> = mlir.lines()
        .filter(|l| l.trim_start().starts_with("func.return"))
        .collect();

    assert!(return_lines.len() >= 2,
        "Expected at least 2 func.return ops for branching returns, found {}.\nMLIR:\n{}",
        return_lines.len(), mlir);

    for (i, line) in return_lines.iter().enumerate() {
        assert!(line.contains("loc(\"test.salt\""),
            "MISSING LOC: func.return #{} without loc annotation: {}", i, line);
    }
}

// =============================================================================
// Test 7: Void return (no return value) gets loc
// =============================================================================
#[test]
fn test_void_return_has_loc() {
    let src = r#"
        package test::void_ret;
        fn main() {
            let x: i32 = 1;
            return;
        }
    "#;
    let mlir = compile_with_debug(src);

    let return_lines: Vec<&str> = mlir.lines()
        .filter(|l| l.trim_start().starts_with("func.return"))
        .collect();

    assert!(!return_lines.is_empty(), "No func.return found in MLIR:\n{}", mlir);

    // The explicit `return;` should get a loc even for void.
    // Note: implicit returns (falling off the end) may not have a loc
    // since they lack an AST span. Only explicit `return;` is required.
    let explicit_return_has_loc = return_lines.iter()
        .any(|l| l.contains("loc(\"test.salt\""));

    assert!(explicit_return_has_loc,
        "MISSING LOC: explicit void return should have loc.\nReturn lines: {:?}", return_lines);
}

// =============================================================================
// Test 8: If-else — cond_br and both returns get loc
// =============================================================================
#[test]
fn test_if_else_cond_br_has_loc() {
    let src = r#"
        package test::if_else_loc;
        fn main() -> i32 {
            let a: i32 = 10;
            let b: i32 = 20;
            if a > b {
                return a;
            } else {
                return b;
            }
        }
    "#;
    let mlir = compile_with_debug(src);

    // The if-else condition branch must carry a loc
    let cond_br_lines: Vec<&str> = mlir.lines()
        .filter(|l| l.trim_start().starts_with("cf.cond_br"))
        .collect();

    assert!(!cond_br_lines.is_empty(), "No cf.cond_br found for if-else:\n{}", mlir);

    for line in &cond_br_lines {
        assert!(line.contains("loc(\"test.salt\""),
            "MISSING LOC: if-else cf.cond_br without loc: {}", line);
    }

    // Both return statements should also have locs
    let return_lines: Vec<&str> = mlir.lines()
        .filter(|l| l.trim_start().starts_with("func.return"))
        .collect();

    assert!(return_lines.len() >= 2,
        "Expected 2 returns for if-else, found {}", return_lines.len());

    for line in &return_lines {
        assert!(line.contains("loc(\"test.salt\""),
            "MISSING LOC: func.return in if-else without loc: {}", line);
    }
}

// =============================================================================
// Test 9: loc annotations contain valid line numbers (> 0)
// =============================================================================
#[test]
fn test_loc_contains_valid_line_numbers() {
    let src = r#"
        package test::line_nums;
        fn main() -> i32 {
            return 1;
        }
    "#;
    let mlir = compile_with_debug(src);

    // Extract all loc annotations and verify they have valid line:col
    let mut found_loc = false;
    for line in mlir.lines() {
        if let Some(loc_start) = line.find("loc(\"test.salt\"") {
            found_loc = true;
            let loc_str = &line[loc_start..];
            // Format: loc("test.salt":LINE:COL)
            let parts: Vec<&str> = loc_str.split(':').collect();
            assert!(parts.len() >= 3,
                "MALFORMED LOC: expected loc(\"file\":line:col), got: {}", loc_str);

            let line_num_str = parts[1].trim();
            let line_num: usize = line_num_str.parse().unwrap_or(0);
            assert!(line_num > 0,
                "INVALID LINE: loc annotation has line number 0: {}", loc_str);
        }
    }
    assert!(found_loc, "No loc annotations found to validate.\nMLIR:\n{}", mlir);
}

// =============================================================================
// Test 10: Debug info is additive — all non-debug behavior preserved
// =============================================================================
#[test]
fn test_debug_preserves_semantics() {
    let src = r#"
        package test::semantics;
        fn main() -> i32 {
            let x: i32 = 7;
            return x * x;
        }
    "#;

    let mlir_debug = compile_with_debug(src);
    let mlir_nodebug = compile_without_debug(src);

    // Both must compile successfully (already asserted by helpers)
    // Both must contain the main function
    assert!(mlir_debug.contains("@main"),
        "Debug MLIR missing main function");
    assert!(mlir_nodebug.contains("@main"),
        "Non-debug MLIR missing main function");

    // Both must contain arith.muli (the multiply)
    assert!(mlir_debug.contains("arith.muli"),
        "Debug MLIR missing multiply op");
    assert!(mlir_nodebug.contains("arith.muli"),
        "Non-debug MLIR missing multiply op");

    // Debug version must have MORE content (loc annotations add text)
    assert!(mlir_debug.len() > mlir_nodebug.len(),
        "Debug MLIR should be larger than non-debug MLIR.\n\
         Debug: {} bytes, Non-debug: {} bytes",
        mlir_debug.len(), mlir_nodebug.len());
}

// =============================================================================
// Test 11: Nested control flow — locs on all terminal ops
// =============================================================================
#[test]
fn test_nested_control_flow_locs() {
    let src = r#"
        package test::nested_cf;
        fn main() -> i32 {
            let x: i32 = 150;
            if x > 100 {
                if x > 200 {
                    return 3;
                }
                return 2;
            }
            return 1;
        }
    "#;
    let mlir = compile_with_debug(src);

    // Should have at least 2 cf.cond_br (outer if, inner if)
    let cond_br_count = mlir.lines()
        .filter(|l| l.trim_start().starts_with("cf.cond_br"))
        .count();

    assert!(cond_br_count >= 2,
        "Expected at least 2 cf.cond_br for nested if, found {}", cond_br_count);

    // ALL cf.cond_br ops must have locs
    for line in mlir.lines() {
        if line.trim_start().starts_with("cf.cond_br") {
            assert!(line.contains("loc(\"test.salt\""),
                "MISSING LOC: nested cf.cond_br without loc: {}", line);
        }
    }

    // ALL func.return ops must have locs
    for line in mlir.lines() {
        if line.trim_start().starts_with("func.return") {
            assert!(line.contains("loc(\"test.salt\""),
                "MISSING LOC: nested func.return without loc: {}", line);
        }
    }
}

// =============================================================================
// Test 12: Source file name propagates into loc annotations
// =============================================================================
#[test]
fn test_source_file_name_in_locs() {
    let src = r#"
        package test::filename;
        fn main() -> i32 {
            return 42;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, true, "my_module.salt");
    assert!(res.is_ok(), "Compilation failed: {:?}", res.err());
    let mlir = res.unwrap();

    // The loc should contain the custom filename, not a hardcoded one
    assert!(mlir.contains("loc(\"my_module.salt\""),
        "loc should contain custom source filename 'my_module.salt'.\nMLIR:\n{}", mlir);
}
