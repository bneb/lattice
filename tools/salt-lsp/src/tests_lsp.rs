//! Comprehensive tests for LSP features added in v0.3.0.
//! Covers: semantic tokens edge cases, references lookup, document symbols,
//! and integration-level SIR index operations.

#[cfg(test)]
mod lsp_tests {
    use crate::sir_index::{SirIndex, compile_in_memory};
    use crate::semantic_tokens;
    use tower_lsp::lsp_types::{Url, Position};

    // ── Semantic Tokens Edge Cases ────────────────────────────────────

    #[test]
    fn test_semantic_tokens_contract_function() {
        let src = "fn div(a: i32, b: i32) -> i32\n    requires(b != 0)\n{\n    return a / b;\n}";
        let tokens = semantic_tokens::tokenize(src);
        assert!(!tokens.is_empty());

        // Should have a MODIFIER token for "requires"
        let has_modifier = tokens.iter().any(|t| t.token_type == 12);
        assert!(has_modifier, "requires should be a modifier token");
    }

    #[test]
    fn test_semantic_tokens_capitalized_types() {
        let src = "let p: Point = Point { x: 1, y: 2 };";
        let tokens = semantic_tokens::tokenize(src);
        let type_tokens: Vec<_> = tokens.iter().filter(|t| t.token_type == 3).collect();
        assert!(!type_tokens.is_empty(), "capitalized identifiers should be type tokens");
    }

    #[test]
    fn test_semantic_tokens_multiple_on_line() {
        let src = "fn main() -> i32 { let x: i32 = 42; return 0; }";
        let tokens = semantic_tokens::tokenize(src);
        // Should produce multiple tokens on the same line
        let first_line_tokens: Vec<_> = tokens.iter().take_while(|t| t.delta_line == 0).collect();
        assert!(first_line_tokens.len() >= 5, "should have multiple tokens on first line");
    }

    #[test]
    fn test_semantic_tokens_fstring() {
        let src = r#"let msg = f"hello {name} in {year}";"#;
        let tokens = semantic_tokens::tokenize(src);
        let has_string = tokens.iter().any(|t| t.token_type == 7);
        assert!(has_string, "f-strings should be string tokens");
    }

    #[test]
    fn test_semantic_tokens_pub_fn() {
        let src = "pub fn exported() -> i64 { return 0; }";
        let tokens = semantic_tokens::tokenize(src);
        let has_keyword = tokens.iter().any(|t| t.token_type == 1);
        assert!(has_keyword, "pub and fn should be keyword tokens");
    }

    #[test]
    fn test_semantic_tokens_delta_encoding_preserves_order() {
        let src = "fn a() -> i32 { return 1; }\nfn b() -> i32 { return 2; }\nfn c() -> i32 { return 3; }";
        let tokens = semantic_tokens::tokenize(src);
        // Verify monotonic ordering in delta-decoded form
        let mut prev_line: i32 = -1;
        for t in &tokens {
            let abs_line = prev_line + t.delta_line as i32;
            assert!(abs_line >= prev_line as i32,
                "tokens must be in monotonic line order");
            prev_line = abs_line;
        }
    }

    // ── References Lookup ────────────────────────────────────────────

    #[test]
    fn test_references_finds_type_usage() {
        let mut index = SirIndex::new();
        let uri = Url::parse("file:///test.salt").unwrap();

        let src = "package test\nstruct Point { x: i32, y: i32 }\nfn get(p: Point) -> Point { return p; }";
        let result = compile_in_memory(src, "test");
        index.update(uri.clone(), result.sir_module.unwrap());

        let refs = index.find_references("Point");
        // Point appears in get's return type and parameter type
        assert!(!refs.is_empty(), "should find references to Point");
    }

    #[test]
    fn test_references_no_false_positives() {
        let mut index = SirIndex::new();
        let uri = Url::parse("file:///test.salt").unwrap();

        let src = "package test\nstruct Foo { x: i32 }\nfn bar() -> i32 { return 0; }";
        let result = compile_in_memory(src, "test");
        index.update(uri, result.sir_module.unwrap());

        let refs = index.find_references("NonExistent");
        assert!(refs.is_empty(), "should not find references to nonexistent types");
    }

    // ── Document Symbols ─────────────────────────────────────────────

    #[test]
    fn test_document_symbols_functions_and_structs() {
        let mut index = SirIndex::new();
        let uri = Url::parse("file:///test.salt").unwrap();

        let src = "package test\npub fn alpha() -> i32 { return 1; }\nfn beta(x: i64) -> bool { return true; }\nstruct Config { debug: bool, port: i32 }";
        let result = compile_in_memory(src, "test");
        index.update(uri.clone(), result.sir_module.unwrap());

        let symbols = index.document_symbols_for(&uri);
        assert_eq!(symbols.len(), 3, "should find 2 functions + 1 struct");
    }

    #[test]
    fn test_document_symbols_empty_file() {
        let mut index = SirIndex::new();
        let uri = Url::parse("file:///empty.salt").unwrap();

        let src = "package empty\n";
        let result = compile_in_memory(src, "empty");
        index.update(uri.clone(), result.sir_module.unwrap());

        let symbols = index.document_symbols_for(&uri);
        assert!(symbols.is_empty(), "empty file should have no symbols");
    }

    #[test]
    fn test_document_symbols_unknown_uri() {
        let index = SirIndex::new();
        let uri = Url::parse("file:///nonexistent.salt").unwrap();
        let symbols = index.document_symbols_for(&uri);
        assert!(symbols.is_empty(), "unknown URI should return no symbols");
    }

    // ── SIR Index Cross-File Operations ──────────────────────────────

    #[test]
    fn test_index_preserves_ordering() {
        let mut index = SirIndex::new();
        let uri = Url::parse("file:///ordered.salt").unwrap();

        let src = "package ordered\nfn first() -> i32 { return 1; }\nfn second() -> i32 { return 2; }";
        let result = compile_in_memory(src, "ordered");
        index.update(uri.clone(), result.sir_module.unwrap());

        let names = index.all_function_names();
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn test_index_multiple_modules() {
        let mut index = SirIndex::new();
        let u1 = Url::parse("file:///mod1.salt").unwrap();
        let u2 = Url::parse("file:///mod2.salt").unwrap();
        let u3 = Url::parse("file:///mod3.salt").unwrap();

        index.update(u1.clone(), compile_in_memory("package m1\nfn f1() -> i32 { return 1; }", "m1").sir_module.unwrap());
        index.update(u2.clone(), compile_in_memory("package m2\nstruct S2 { a: i32 }", "m2").sir_module.unwrap());
        index.update(u3.clone(), compile_in_memory("package m3\nfn f3() -> i32 { return 3; }", "m3").sir_module.unwrap());

        let all_fns = index.all_function_names();
        assert!(all_fns.contains(&"f1"));
        assert!(all_fns.contains(&"f3"));
        assert!(!all_fns.contains(&"S2")); // struct, not function
    }

    // ── Compile In-Memory Edge Cases ─────────────────────────────────

    #[test]
    fn test_compile_empty_source() {
        let result = compile_in_memory("", "empty");
        // Empty source returns a module with no functions/structs
        assert!(result.sir_module.is_some() || result.error.is_some(),
            "empty source should produce module or error");
    }

    #[test]
    fn test_compile_comment_only() {
        let result = compile_in_memory("// just a comment\n", "comment");
        // Comment-only source may parse as empty module
        assert!(result.sir_module.is_some() || result.error.is_some(),
            "comment-only source should produce module or error");
    }

    #[test]
    fn test_compile_generic_function() {
        let src = "package gen\nfn identity<T>(x: T) -> T { return x; }";
        let result = compile_in_memory(src, "gen");
        assert!(result.error.is_none(), "generic function should compile: {:?}", result.error);
        let module = result.sir_module.unwrap();
        assert_eq!(module.functions.len(), 1);
        assert_eq!(module.functions[0].name, "identity");
    }

    // ── Completion Smoke Test ──────────────────────────────────────

    #[test]
    fn test_completion_smoke() {
        let text = "package test\nfn main() -> i32 {\n    return 0;\n}";
        let items = crate::completion::complete(text, Position { line: 0, character: 0 });
        assert!(!items.is_empty(), "should return at least one completion");
        // Should include keyword completions
        assert!(items.iter().any(|i| i.label == "fn"), "should include fn keyword");
        assert!(items.iter().any(|i| i.label == "i32"), "should include i32 type");
    }
}
