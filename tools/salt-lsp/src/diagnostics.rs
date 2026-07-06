//! Salt LSP Diagnostics — Pattern-based + In-Memory Compiler Diagnostics
//!
//! Two-tier diagnostic system:
//!   1. Fast-path: instant pattern-based lint checks (runs synchronously, <1ms)
//!   2. Deep-path: in-memory salt-front compilation via library API (<5ms)
//!
//! Both tiers run on every keystroke. No subprocess, no temp files, no I/O.

use tower_lsp::lsp_types::*;

use crate::sir_index;
use crate::source_check;

// =============================================================================
// Fast-Path: Pattern-Based Lint Diagnostics
// =============================================================================

/// Diagnose a Salt source file with instant pattern-based checks.
pub fn diagnose(text: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    for (line_idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }

        // Check: `import` keyword should be `use`
        if trimmed.starts_with("import ") {
            diags.push(make_diagnostic(
                line_idx,
                0,
                6,
                "The `import` keyword is abolished in Salt. Use `use` instead.",
                DiagnosticSeverity::ERROR,
            ));
        }

        // Check: NativePtr / NodePtr usage (abolished types)
        if trimmed.contains("NativePtr") {
            let col = line.find("NativePtr").unwrap_or(0);
            diags.push(make_diagnostic(
                line_idx,
                col,
                col + 9,
                "Legacy type `NativePtr` is abolished. Use `Ptr<T>` instead.",
                DiagnosticSeverity::ERROR,
            ));
        }
        if trimmed.contains("NodePtr") {
            let col = line.find("NodePtr").unwrap_or(0);
            diags.push(make_diagnostic(
                line_idx,
                col,
                col + 7,
                "Legacy type `NodePtr` is abolished. Use `Ptr<T>` instead.",
                DiagnosticSeverity::ERROR,
            ));
        }

        // Check: double underscore in identifiers (reserved for mangling)
        if (trimmed.starts_with("let ") || trimmed.starts_with("fn ")) && trimmed.contains("__") {
            let col = line.find("__").unwrap_or(0);
            diags.push(make_diagnostic(
                line_idx,
                col,
                col + 2,
                "Identifiers cannot contain `__` (reserved for symbol mangling).",
                DiagnosticSeverity::WARNING,
            ));
        }

        // Check: unclosed string literals
        let in_comment = trimmed.starts_with("//");
        if !in_comment {
            let quote_count = trimmed.chars().filter(|c| *c == '"').count();
            if quote_count % 2 != 0 && !trimmed.contains("f\"") {
                diags.push(make_diagnostic(
                    line_idx,
                    0,
                    line.len(),
                    "Unclosed string literal.",
                    DiagnosticSeverity::ERROR,
                ));
            }
        }

        // Check: missing semicolons on let statements
        if trimmed.starts_with("let ") && !trimmed.ends_with('{') && !trimmed.ends_with(';') {
            diags.push(make_diagnostic(
                line_idx,
                line.len().saturating_sub(1),
                line.len(),
                "Missing semicolon after `let` statement.",
                DiagnosticSeverity::WARNING,
            ));
        }
    }

    diags.extend(source_check::diagnose_source(text));
    diags
}

// =============================================================================
// Deep-Path: In-Memory Compiler Diagnostics
// =============================================================================

/// Run the salt-front compiler in-memory and return diagnostics + SIR module.
/// This calls salt-front's library API directly — zero subprocess overhead.
pub fn diagnose_with_compiler(text: &str, module_name: &str) -> (Vec<Diagnostic>, Option<sir_index::SirModule>) {
    let result = sir_index::compile_in_memory(text, module_name);

    let mut diags = Vec::new();

    if let Some(error_msg) = &result.error {
        // Parse line:col from error message format "module:line:col: message"
        let diag = parse_error_to_diagnostic(error_msg, text);
        diags.push(diag);
    }

    (diags, result.sir_module)
}

/// Parse a compiler error string into an LSP diagnostic.
/// Format: "module:line:col: message" or just "message"
fn parse_error_to_diagnostic(error: &str, source: &str) -> Diagnostic {
    // Try to extract line:col from "module:line:col: message" format
    let parts: Vec<&str> = error.splitn(4, ':').collect();

    if parts.len() >= 4 {
        // Format: module:line:col: message
        if let (Ok(line), Ok(col)) = (parts[1].trim().parse::<u32>(), parts[2].trim().parse::<u32>()) {
            let line_0 = line.saturating_sub(1); // Convert 1-indexed to 0-indexed
            let col_0 = col.saturating_sub(1);

            // Calculate end column from the line content
            let end_col = source.lines().nth(line_0 as usize)
                .map(|l| l.len() as u32)
                .unwrap_or(col_0 + 1);

            return Diagnostic {
                range: Range {
                    start: Position { line: line_0, character: col_0 },
                    end: Position { line: line_0, character: end_col },
                },
                severity: Some(DiagnosticSeverity::ERROR),
                code: None,
                code_description: None,
                source: Some("saltc".to_string()),
                message: parts[3].trim().to_string(),
                related_information: None,
                tags: None,
                data: None,
            };
        }
    }

    // Fallback: no line info — highlight first code line
    let first_code_line = source.lines().enumerate()
        .find(|(_, l)| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with("//") && !t.starts_with("package ")
        })
        .map(|(idx, l)| (idx as u32, l.len() as u32))
        .unwrap_or((0, 1));

    Diagnostic {
        range: Range {
            start: Position { line: first_code_line.0, character: 0 },
            end: Position { line: first_code_line.0, character: first_code_line.1 },
        },
        severity: Some(DiagnosticSeverity::ERROR),
        code: None,
        code_description: None,
        source: Some("saltc".to_string()),
        message: error.to_string(),
        related_information: None,
        tags: None,
        data: None,
    }
}

// =============================================================================
// Shared Diagnostic Helpers
// =============================================================================

fn make_diagnostic(
    line: usize,
    start_col: usize,
    end_col: usize,
    message: &str,
    severity: DiagnosticSeverity,
) -> Diagnostic {
    Diagnostic {
        range: Range {
            start: Position {
                line: line as u32,
                character: start_col as u32,
            },
            end: Position {
                line: line as u32,
                character: end_col as u32,
            },
        },
        severity: Some(severity),
        code: None,
        code_description: None,
        source: Some("salt-lsp".to_string()),
        message: message.to_string(),
        related_information: None,
        tags: None,
        data: None,
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── Pattern-Based Diagnostics ────────────────────────────────────

    #[test]
    fn test_import_keyword_error() {
        let diags = diagnose("import std.core.result.Result");
        assert!(diags.iter().any(|d| d.message.contains("abolished")),
            "should flag import keyword: {:?}", diags);
    }

    #[test]
    fn test_native_ptr_error() {
        let diags = diagnose("let x: NativePtr = null;");
        assert!(diags.iter().any(|d| d.message.contains("NativePtr")));
    }

    #[test]
    fn test_clean_code_no_errors() {
        let code = r#"
package main

use std.core.result.Result

fn main() -> i32 {
    let x: i32 = 42;
    return 0;
}
"#;
        let diags = diagnose(code);
        let errors: Vec<_> = diags.iter().filter(|d| d.severity == Some(DiagnosticSeverity::ERROR)).collect();
        assert!(errors.is_empty(), "Clean code should have no ERROR diagnostics, got: {:?}", diags);
    }

    #[test]
    fn test_double_underscore_warning() {
        let diags = diagnose("let my__var: i32 = 0;");
        assert!(diags.iter().any(|d| d.message.contains("__")));
    }

    // ── In-Memory Compiler Diagnostics ───────────────────────────────

    #[test]
    fn test_compiler_diag_valid_code() {
        let source = "package test\nfn add(a: i32, b: i32) -> i32 { return a + b; }";
        let (diags, sir) = diagnose_with_compiler(source, "test");
        assert!(diags.is_empty(), "Valid code should have no compiler errors: {:?}", diags);
        assert!(sir.is_some());
    }

    #[test]
    fn test_compiler_diag_invalid_code() {
        let source = "package test\nfn broken( { }";
        let (diags, sir) = diagnose_with_compiler(source, "test");
        assert!(!diags.is_empty(), "Invalid code should produce diagnostics");
        assert!(sir.is_none());
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
        assert!(diags[0].source.as_deref() == Some("saltc"));
    }

    #[test]
    fn test_compiler_diag_extracts_sir_on_success() {
        let source = "package test\nstruct Foo { x: i32, }\npub fn bar() -> i32 { return 0; }";
        let (diags, sir) = diagnose_with_compiler(source, "test");
        assert!(diags.is_empty());
        let module = sir.unwrap();
        assert_eq!(module.structs.len(), 1);
        assert_eq!(module.functions.len(), 1);
        assert!(module.functions[0].is_pub);
    }

    // ── Error Parser ─────────────────────────────────────────────────

    #[test]
    fn test_parse_error_with_location() {
        let error = "test:3:5: expected identifier";
        let diag = parse_error_to_diagnostic(error, "line1\nline2\nfn broken(\nline4");
        assert_eq!(diag.range.start.line, 2); // 0-indexed
        assert_eq!(diag.range.start.character, 4); // 0-indexed
        assert_eq!(diag.message, "expected identifier");
    }

    #[test]
    fn test_parse_error_without_location() {
        let error = "cannot parse string into token stream";
        let source = "package test\n\nfn broken() {\n}";
        let diag = parse_error_to_diagnostic(error, source);
        // Should highlight first code line (fn broken)
        assert_eq!(diag.range.start.line, 2);
    }
}
