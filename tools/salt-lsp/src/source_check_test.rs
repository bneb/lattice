//! Tests for source-level diagnostic checks and definition lookup.

#[cfg(test)]
mod tests {
    use crate::source_check::*;
    use tower_lsp::lsp_types::DiagnosticSeverity;

    // ── Missing Return ────────────────────────────────────────────

    #[test]
    fn test_missing_return_detected() {
        let src = "fn foo() -> i32 {\n    let x = 42;\n}";
        let diags = check_missing_return(src);
        assert!(!diags.is_empty(), "should flag missing return");
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
    }

    #[test]
    fn test_missing_return_ok() {
        let src = "fn foo() -> i32 {\n    return 42;\n}";
        assert!(check_missing_return(src).is_empty());
    }

    #[test]
    fn test_void_fn_no_return_ok() {
        let src = "fn foo() {\n    let x = 1;\n}";
        assert!(check_missing_return(src).is_empty());
    }

    #[test]
    fn test_nested_block_return_found() {
        let src = "fn bar() -> i32 {\n    if true {\n        return 1;\n    }\n}";
        let diags = check_missing_return(src);
        assert!(diags.is_empty(), "return in nested block should be found");
    }

    // ── Unused Variable ────────────────────────────────────────────

    #[test]
    fn test_unused_var_detected() {
        let src = "fn foo() {\n    let x = 42;\n    let z = x + 1;\n}";
        let diags = check_unused_vars(src);
        assert!(diags.iter().any(|d| d.message.contains("z")),
            "z should be flagged as unused");
    }

    #[test]
    fn test_unused_var_underscore_ok() {
        let src = "fn foo() { let _x = 42; }";
        assert!(check_unused_vars(src).is_empty());
    }

    #[test]
    fn test_param_used_passes() {
        let src = "fn foo(x: i32) -> i32 { return x; }";
        assert!(check_unused_vars(src).is_empty());
    }

    #[test]
    fn test_param_unused_detected() {
        let src = "fn foo(x: i32) -> i32 { return 0; }";
        let diags = check_unused_vars(src);
        assert!(diags.iter().any(|d| d.message.contains("x")),
            "x param should be flagged as unused");
    }

    // ── Type Mismatch ──────────────────────────────────────────────

    #[test]
    fn test_type_mismatch_string_literal() {
        let src = "fn foo() { let x: i32 = \"hello\"; }";
        let diags = check_type_mismatch(src);
        assert!(!diags.is_empty(), "should flag string->i32");
        assert!(diags[0].message.contains("Type mismatch"));
    }

    #[test]
    fn test_type_mismatch_correct_ok() {
        let src = "fn foo() { let x: i32 = 42; }";
        assert!(check_type_mismatch(src).is_empty());
    }

    #[test]
    fn test_type_mismatch_string_var_ok() {
        let src = "fn foo() { let x: String = \"hello\"; }";
        assert!(check_type_mismatch(src).is_empty(),
            "String = \"hello\" should be fine");
    }

    // ── Unknown Variable ──────────────────────────────────────────

    #[test]
    fn test_unknown_var_detected() {
        let src = "fn foo() {\n    let x = unknown_name;\n}";
        let diags = check_unknown_var(src);
        assert!(!diags.is_empty(), "should flag unknown_name");
    }

    #[test]
    fn test_unknown_var_known_ok() {
        let src = "fn foo() {\n    let x = 42;\n    let y = x + 1;\n}";
        assert!(check_unknown_var(src).is_empty(),
            "should not flag defined variable");
    }

    #[test]
    fn test_keyword_not_flagged() {
        let src = "fn foo() { return true; }";
        assert!(check_unknown_var(src).is_empty(),
            "keywords should not be flagged");
    }

    // ── Definition Lookup ─────────────────────────────────────────

    #[test]
    fn test_find_let_definition() {
        let src = "fn foo() {\n    let x = 42;\n    let y = x + 1;\n}";
        let pos = find_var_definition(src, "x", 2);
        assert!(pos.is_some(), "should find definition of x");
        assert_eq!(pos.unwrap().0, 1, "x defined on line 1");
    }

    #[test]
    fn test_find_param_definition() {
        let src = "fn foo(x: i32, y: i32) -> i32 {\n    return x + y;\n}";
        let pos = find_var_definition(src, "x", 1);
        assert!(pos.is_some());
        assert_eq!(pos.unwrap().0, 0);
    }

    #[test]
    fn test_find_field_definition() {
        let src = "struct Point { x: i32, y: i32 }";
        let pos = find_var_definition(src, "x", 1);
        assert!(pos.is_some(), "should find field x");
        assert_eq!(pos.unwrap().0, 0);
    }

    #[test]
    fn test_find_definition_nonexistent() {
        let src = "fn foo() { let y = 42; }";
        assert!(find_var_definition(src, "z", 1).is_none());
    }

    // ── Full suite integration ─────────────────────────────────────

    #[test]
    fn test_diagnose_source_clean() {
        let src = "package test\nfn main() -> i32 { return 0; }";
        let diags = diagnose_source(src);
        assert!(diags.is_empty(), "clean code: {:?}", diags);
    }
}
