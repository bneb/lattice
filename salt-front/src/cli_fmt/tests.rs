use super::format_salt;

// ── Required test cases ──

#[test]
fn test_basic_function_formatting() {
    let input = "fn add(x: i32, y: i32) -> i32 {\nreturn x + y;\n}\n";
    let expected = "fn add(x: i32, y: i32) -> i32 {\n    return x + y;\n}\n";
    let output = format_salt(input).unwrap();
    assert_eq!(output, expected, "function body should be indented");
}

#[test]
fn test_indentation_normalization() {
    let input = "fn foo() {\nlet x = 1;\nlet y = 2;\nif x > 0 {\nreturn x;\n}\n}\n";
    let expected = "fn foo() {\n    let x = 1;\n    let y = 2;\n    if x > 0 {\n        return x;\n    }\n}\n";
    let output = format_salt(input).unwrap();
    assert_eq!(output, expected);
}

#[test]
fn test_trailing_whitespace_removal() {
    let input = "fn foo() {   \n    let x = 1;   \n}   \n";
    let expected = "fn foo() {\n    let x = 1;\n}\n";
    let output = format_salt(input).unwrap();
    assert_eq!(output, expected);
}

#[test]
fn test_comment_preservation() {
    let input = "// this is a comment\nfn foo() {\n    // inner comment\n    let x = 1; // inline comment\n}\n";
    let output = format_salt(input).unwrap();
    assert!(output.contains("// this is a comment"));
    assert!(output.contains("// inner comment"));
    assert!(output.contains("// inline comment"));
}

#[test]
fn test_idempotency() {
    let input = "fn foo() {\n    let x = 1;\n    if x > 0 {\n        return x;\n    }\n}\n";
    let once = format_salt(input).unwrap();
    let twice = format_salt(&once).unwrap();
    assert_eq!(once, twice, "formatting twice should give the same result");
}

// ── Additional tests ──

#[test]
fn test_standalone_brace_merged() {
    let input = "fn foo()\n{\nlet x = 1;\n}\n";
    let expected = "fn foo() {\n    let x = 1;\n}\n";
    let output = format_salt(input).unwrap();
    assert_eq!(output, expected);
}

#[test]
fn test_nested_standalone_braces() {
    let input = "fn test()\n{\nif true\n{\nreturn 1;\n}\n}\n";
    let expected = "fn test() {\n    if true {\n        return 1;\n    }\n}\n";
    let output = format_salt(input).unwrap();
    assert_eq!(output, expected);
}

#[test]
fn test_operator_spacing_assignment() {
    let input = "let x=1;\nlet y =2;\nlet z= 3;\n";
    let expected = "let x = 1;\nlet y = 2;\nlet z = 3;\n";
    let output = format_salt(input).unwrap();
    assert_eq!(output, expected);
}

#[test]
fn test_operator_spacing_comparison() {
    let input = "if x>0&&y<5{\nreturn x;\n}\n";
    let output = format_salt(input).unwrap();
    assert!(output.contains("if x > 0 && y < 5 {"));
}

#[test]
fn test_operator_spacing_arithmetic() {
    let input = "let z=x+y*w;\n";
    let output = format_salt(input).unwrap();
    assert!(output.contains("let z = x + y * w;"));
}

#[test]
fn test_blank_lines_between_functions() {
    let input = "fn a() {\n    return 1;\n}\nfn b() {\n    return 2;\n}\n";
    let output = format_salt(input).unwrap();
    assert!(output.contains("}\n\nfn b()"));
}

#[test]
fn test_consecutive_blank_lines_collapsed() {
    let input = "fn a() {\n    return 1;\n}\n\n\n\nfn b() {\n    return 2;\n}\n";
    let output = format_salt(input).unwrap();
    assert!(output.contains("}\n\nfn b()"));
    assert!(!output.contains("\n\n\n"));
}

#[test]
fn test_doc_comments_preserved() {
    let input = "/// This is a doc comment\nfn documented() {\n    /// inner doc\n    let x = 1;\n}\n";
    let output = format_salt(input).unwrap();
    assert!(output.contains("/// This is a doc comment"));
    assert!(output.contains("/// inner doc"));
}

#[test]
fn test_fstring_preserved() {
    let input = "fn greet(name: &str) {\n    let msg = f\"hello {name}\";\n}\n";
    let output = format_salt(input).unwrap();
    assert!(output.contains("f\"hello {name}\""));
}

#[test]
fn test_struct_formatting() {
    let input = "struct Point {\nx: i32,\ny: i32,\n}\n";
    let expected = "struct Point {\n    x: i32,\n    y: i32,\n}\n";
    let output = format_salt(input).unwrap();
    assert_eq!(output, expected);
}

#[test]
fn test_enum_formatting() {
    let input = "enum Color {\nRed,\nGreen,\nBlue,\n}\n";
    let expected = "enum Color {\n    Red,\n    Green,\n    Blue,\n}\n";
    let output = format_salt(input).unwrap();
    assert_eq!(output, expected);
}

#[test]
fn test_deeply_nested() {
    let input = "fn outer() {\nif true {\nif true {\nlet x = 1;\n}\n}\n}\n";
    let expected = "fn outer() {\n    if true {\n        if true {\n            let x = 1;\n        }\n    }\n}\n";
    let output = format_salt(input).unwrap();
    assert_eq!(output, expected);
}

#[test]
fn test_idempotency_multiple_items() {
    let input = "fn foo() {\n    return 1;\n}\n\nfn bar() {\n    return 2;\n}\n";
    let once = format_salt(input).unwrap();
    let twice = format_salt(&once).unwrap();
    assert_eq!(once, twice);
}

#[test]
fn test_no_trailing_whitespace_any_line() {
    let input = "fn a() {\n    return 1;\n}   \n   \nfn b() {\n    return 2;\n}   \n";
    let output = format_salt(input).unwrap();
    for line in output.lines() {
        assert_eq!(line, line.trim_end(), "no trailing whitespace");
    }
}

#[test]
fn test_no_spaces_around_dot() {
    let input = "fn call(obj: MyType) {\n    let _ = obj.method();\n}\n";
    let output = format_salt(input).unwrap();
    assert!(output.contains("obj.method()"));
}

#[test]
fn test_operators_not_modified_in_strings() {
    let input = "fn test() {\n    let s = \"x+y=z\";\n}\n";
    let output = format_salt(input).unwrap();
    assert!(output.contains("\"x+y=z\""));
}

#[test]
fn test_empty_file() {
    let output = format_salt("\n").unwrap();
    assert_eq!(output, "\n");
}

#[test]
fn test_just_comments() {
    let input = "// line 1\n// line 2\n";
    let output = format_salt(input).unwrap();
    assert_eq!(output, input);
}

#[test]
fn test_single_statement_fn() {
    let input = "fn answer() -> i32 {\n    42\n}\n";
    let output = format_salt(input).unwrap();
    assert_eq!(output, input, "already-indented single statement should be unchanged");
}

#[test]
fn test_use_statements() {
    let input = "use std::core::ptr::*;\nuse std::status::*;\nfn main() {\n}\n";
    let output = format_salt(input).unwrap();
    assert!(output.contains("use std::core::ptr::*;"));
    assert!(output.contains("use std::status::*;"));
}
