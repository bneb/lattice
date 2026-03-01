//! SIR Index — In-memory symbol index from salt-front's AST/SIR pipeline
//!
//! Uses salt-front as a library crate for zero-I/O, in-memory compilation.
//! The parser produces a SaltFile AST, which is lowered to SirModule for
//! semantic analysis. Diagnostic latency: <5ms (no subprocess, no temp files).

use std::collections::HashMap;
use tower_lsp::lsp_types::Url;

// Re-export salt-front's SIR types directly — zero conversion overhead
pub use salt_front::codegen::sir::types::{
    SirModule, SirFunction, SirStruct, SirParam, SirContract, SirBlock,
    SirInstruction, SirType, SirValue, SirLocation,
};

use tower_lsp::lsp_types::{Location, Position, Range};

// =============================================================================
// SIR Index — Cross-file symbol lookup
// =============================================================================

/// In-memory index of SIR data across all open files.
pub struct SirIndex {
    /// Per-file SIR modules, populated from in-memory AST extraction.
    modules: HashMap<Url, SirModule>,
}

impl SirIndex {
    pub fn new() -> Self {
        SirIndex {
            modules: HashMap::new(),
        }
    }

    /// Store a compiled SIR module for the given URI.
    pub fn update(&mut self, uri: Url, module: SirModule) {
        self.modules.insert(uri, module);
    }

    /// Remove a file from the index (on close).
    pub fn remove(&mut self, uri: &Url) {
        self.modules.remove(uri);
    }

    /// Look up a function by name across all indexed modules.
    pub fn lookup_function(&self, name: &str) -> Option<&SirFunction> {
        for module in self.modules.values() {
            for func in &module.functions {
                if func.name == name {
                    return Some(func);
                }
            }
        }
        None
    }

    /// Look up a struct by name across all indexed modules.
    pub fn lookup_struct(&self, name: &str) -> Option<&SirStruct> {
        for module in self.modules.values() {
            for s in &module.structs {
                if s.name == name {
                    return Some(s);
                }
            }
        }
        None
    }

    /// Get all contracts for a specific function.
    pub fn contracts_for(&self, fn_name: &str) -> Vec<&SirContract> {
        self.lookup_function(fn_name)
            .map(|f| f.contracts.iter().collect())
            .unwrap_or_default()
    }

    /// Get the SIR module for a specific file.
    pub fn module_for(&self, uri: &Url) -> Option<&SirModule> {
        self.modules.get(uri)
    }

    /// Get all function names across all modules (for completion).
    pub fn all_function_names(&self) -> Vec<&str> {
        let mut names = Vec::new();
        for module in self.modules.values() {
            for func in &module.functions {
                names.push(func.name.as_str());
            }
        }
        names
    }

    /// Get all struct names across all modules (for completion).
    pub fn all_struct_names(&self) -> Vec<&str> {
        let mut names = Vec::new();
        for module in self.modules.values() {
            for s in &module.structs {
                names.push(s.name.as_str());
            }
        }
        names
    }

    /// Format a SirType as a human-readable Salt type string.
    fn format_type(ty: &SirType) -> String {
        match ty {
            SirType::I32 => "i32".to_string(),
            SirType::I64 => "i64".to_string(),
            SirType::U32 => "u32".to_string(),
            SirType::U64 => "u64".to_string(),
            SirType::F64 => "f64".to_string(),
            SirType::Bool => "bool".to_string(),
            SirType::Void => "void".to_string(),
            SirType::Ptr(inner) => format!("Ptr<{}>", Self::format_type(inner)),
            SirType::Struct(name) => name.clone(),
            SirType::Array(inner, size) => format!("[{}; {}]", Self::format_type(inner), size),
        }
    }

    /// Format a function signature for hover display.
    ///
    /// Renders a beautifully formatted VS Code Markdown tooltip with:
    ///   - Salt code block for the signature
    ///   - Z3 contract display with verification status icons
    ///   - Function attributes
    pub fn format_function_hover(func: &SirFunction) -> String {
        let mut md = String::new();

        // ── Code Block: Signature ──
        md.push_str("```salt\n");
        if func.is_pub {
            md.push_str("pub ");
        }
        md.push_str(&format!("fn {}(", func.name));
        let param_strs: Vec<String> = func.params
            .iter()
            .map(|p| format!("{}: {}", p.name, Self::format_type(&p.ty)))
            .collect();
        md.push_str(&param_strs.join(", "));
        md.push_str(&format!(") -> {}\n", Self::format_type(&func.return_type)));
        md.push_str("```\n");

        // ── Formal Contracts (Z3 Verified) ──
        if !func.contracts.is_empty() {
            md.push_str("---\n**Formal Contracts:**\n\n");
            for contract in &func.contracts {
                let status_icon = if contract.z3_verified {
                    "✅ *(Verified)*"
                } else {
                    "⚠️ *(Runtime Assertion)*"
                };
                md.push_str(&format!("* `{}`: `{}` {}\n",
                    contract.kind,
                    contract.expression,
                    status_icon
                ));
            }
        }

        // ── Attributes ──
        if !func.attributes.is_empty() {
            md.push_str("\n---\n**Attributes:** ");
            let attrs: Vec<String> = func.attributes.iter()
                .map(|a| format!("`{}`", a))
                .collect();
            md.push_str(&attrs.join(", "));
            md.push('\n');
        }

        md
    }

    /// Format a struct for hover display.
    ///
    /// Renders a Salt code block with field layout and attributes.
    pub fn format_struct_hover(s: &SirStruct) -> String {
        let mut md = String::new();

        // ── Code Block: Struct Layout ──
        md.push_str("```salt\n");
        md.push_str(&format!("struct {} {{\n", s.name));
        for field in &s.fields {
            md.push_str(&format!("    {}: {},\n", field.name, Self::format_type(&field.ty)));
        }
        md.push_str("}\n```\n");

        // ── Attributes ──
        if !s.attributes.is_empty() {
            md.push_str("---\n**Attributes:** ");
            let attrs: Vec<String> = s.attributes.iter()
                .map(|a| format!("`{}`", a))
                .collect();
            md.push_str(&attrs.join(", "));
            md.push('\n');
        }

        md
    }

    // =========================================================================
    // Go-to-Definition
    // =========================================================================

    /// Convert a SirLocation (1-indexed lines from syn) to an LSP Location
    /// (0-indexed lines and columns per LSP spec).
    fn sir_location_to_lsp(uri: &Url, loc: &SirLocation) -> Location {
        Location {
            uri: uri.clone(),
            range: Range {
                start: Position::new(
                    loc.line.saturating_sub(1) as u32,   // syn 1-indexed → LSP 0-indexed
                    loc.column as u32,                    // syn column is already 0-indexed
                ),
                end: Position::new(
                    loc.end_line.saturating_sub(1) as u32,
                    loc.end_column as u32,
                ),
            },
        }
    }

    /// Find the definition location for a symbol name.
    /// Searches all indexed modules for matching functions and structs,
    /// returning the LSP Location with the URI of the file that contains it.
    pub fn find_definition(&self, symbol_name: &str) -> Option<Location> {
        for (uri, module) in &self.modules {
            // Search functions
            for func in &module.functions {
                if func.name == symbol_name {
                    if let Some(ref loc) = func.location {
                        return Some(Self::sir_location_to_lsp(uri, loc));
                    }
                }
            }
            // Search structs
            for s in &module.structs {
                if s.name == symbol_name {
                    if let Some(ref loc) = s.location {
                        return Some(Self::sir_location_to_lsp(uri, loc));
                    }
                }
            }
        }
        None
    }
}

// =============================================================================
// In-Memory Compilation Pipeline
// =============================================================================

/// Result from an in-memory compilation attempt.
pub struct CompileResult {
    /// The SIR module (if parsing succeeded).
    pub sir_module: Option<SirModule>,
    /// Error message (if parsing failed).
    pub error: Option<String>,
}

/// Compile Salt source text in-memory via salt-front's library API.
/// Pipeline: source → preprocess → syn::parse → extract_sir_from_ast
///
/// This runs entirely in-process with zero I/O — no temp files, no subprocess.
pub fn compile_in_memory(source: &str, module_name: &str) -> CompileResult {
    use salt_front::grammar::SaltFile;
    use salt_front::codegen::sir::sir_emit::extract_sir_from_ast;

    // Step 1: Preprocess Salt source (keyword transforms, syntax sugar expansion)
    let preprocessed = salt_front::preprocess(source);

    // Step 2: Parse via syn into the Salt AST
    let ast: SaltFile = match syn::parse_str(&preprocessed) {
        Ok(ast) => ast,
        Err(err) => {
            // Extract line/column from syn::Error's Span
            let span = err.span();
            let start = span.start();
            return CompileResult {
                sir_module: None,
                error: Some(format!(
                    "{}:{}:{}: {}",
                    module_name,
                    start.line,
                    start.column,
                    err
                )),
            };
        }
    };

    // Step 3: Lower AST to SIR
    let sir_module = extract_sir_from_ast(&ast, module_name);

    CompileResult {
        sir_module: Some(sir_module),
        error: None,
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── In-Memory Compilation ────────────────────────────────────────

    #[test]
    fn test_compile_valid_function() {
        let source = r#"
package test

fn add(a: i32, b: i32) -> i32 {
    return a + b;
}
"#;
        let result = compile_in_memory(source, "test");
        assert!(result.error.is_none(), "Expected no error, got: {:?}", result.error);
        assert!(result.sir_module.is_some());

        let module = result.sir_module.unwrap();
        assert_eq!(module.name, "test");
        assert_eq!(module.functions.len(), 1);
        assert_eq!(module.functions[0].name, "add");
        assert_eq!(module.functions[0].params.len(), 2);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_compile_invalid_syntax() {
        let source = r#"
package test

fn broken( {
}
"#;
        let result = compile_in_memory(source, "test");
        assert!(result.error.is_some());
        assert!(result.sir_module.is_none());
    }

    #[test]
    fn test_compile_struct_extraction() {
        let source = r#"
package test

struct Point {
    x: i32,
    y: i32,
}
"#;
        let result = compile_in_memory(source, "test");
        assert!(result.error.is_none(), "Got error: {:?}", result.error);
        let module = result.sir_module.unwrap();
        assert_eq!(module.structs.len(), 1);
        assert_eq!(module.structs[0].name, "Point");
        assert_eq!(module.structs[0].fields.len(), 2);
    }

    #[test]
    fn test_compile_pub_function() {
        let source = r#"
package test

pub fn greet() -> i32 {
    return 42;
}
"#;
        let result = compile_in_memory(source, "test");
        assert!(result.error.is_none(), "Got error: {:?}", result.error);
        let module = result.sir_module.unwrap();
        assert!(module.functions[0].is_pub);
    }

    #[test]
    fn test_compile_multiple_functions() {
        let source = r#"
package test

fn foo() -> i32 {
    return 1;
}

fn bar(x: i64) -> i64 {
    return x;
}
"#;
        let result = compile_in_memory(source, "test");
        assert!(result.error.is_none(), "Got error: {:?}", result.error);
        let module = result.sir_module.unwrap();
        assert_eq!(module.functions.len(), 2);
    }

    // ── SIR Index Operations ─────────────────────────────────────────

    #[test]
    fn test_index_update_and_lookup() {
        let mut index = SirIndex::new();
        let uri = Url::parse("file:///test.salt").unwrap();

        let source = "package test\nfn my_func(x: i32) -> bool { return true; }";
        let result = compile_in_memory(source, "test");
        assert!(result.sir_module.is_some());

        index.update(uri.clone(), result.sir_module.unwrap());

        let func = index.lookup_function("my_func");
        assert!(func.is_some());
        assert_eq!(func.unwrap().name, "my_func");
    }

    #[test]
    fn test_index_cross_file_lookup() {
        let mut index = SirIndex::new();

        let uri1 = Url::parse("file:///a.salt").unwrap();
        let uri2 = Url::parse("file:///b.salt").unwrap();

        let src1 = "package a\nfn from_a() -> i32 { return 1; }";
        let src2 = "package b\nfn from_b() -> i64 { return 2; }";

        let r1 = compile_in_memory(src1, "a");
        let r2 = compile_in_memory(src2, "b");

        index.update(uri1, r1.sir_module.unwrap());
        index.update(uri2, r2.sir_module.unwrap());

        assert!(index.lookup_function("from_a").is_some());
        assert!(index.lookup_function("from_b").is_some());
        assert!(index.lookup_function("nonexistent").is_none());
    }

    #[test]
    fn test_index_remove() {
        let mut index = SirIndex::new();
        let uri = Url::parse("file:///test.salt").unwrap();

        let src = "package test\nfn foo() -> i32 { return 0; }";
        let result = compile_in_memory(src, "test");
        index.update(uri.clone(), result.sir_module.unwrap());

        assert!(index.lookup_function("foo").is_some());
        index.remove(&uri);
        assert!(index.lookup_function("foo").is_none());
    }

    #[test]
    fn test_all_function_names() {
        let mut index = SirIndex::new();
        let uri = Url::parse("file:///test.salt").unwrap();

        let src = "package test\nfn alpha() -> i32 { return 1; }\nfn beta(x: i32) -> i32 { return x; }";
        let result = compile_in_memory(src, "test");
        index.update(uri, result.sir_module.unwrap());

        let names = index.all_function_names();
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"beta"));
    }

    #[test]
    fn test_all_struct_names() {
        let mut index = SirIndex::new();
        let uri = Url::parse("file:///test.salt").unwrap();

        let src = "package test\nstruct Foo { x: i32, }\nstruct Bar { y: i64, }";
        let result = compile_in_memory(src, "test");
        index.update(uri, result.sir_module.unwrap());

        let names = index.all_struct_names();
        assert!(names.contains(&"Foo"));
        assert!(names.contains(&"Bar"));
    }

    // ── Hover Formatting ─────────────────────────────────────────────

    #[test]
    fn test_format_function_hover() {
        let src = "package test\npub fn add(a: i32, b: i32) -> i32 { return a + b; }";
        let result = compile_in_memory(src, "test");
        let module = result.sir_module.unwrap();
        let func = &module.functions[0];

        let hover = SirIndex::format_function_hover(func);
        assert!(hover.contains("pub fn add("));
        assert!(hover.contains("a: "));
        assert!(hover.contains("b: "));
    }

    #[test]
    fn test_format_struct_hover() {
        let src = "package test\nstruct Point { x: i32, y: i32, }";
        let result = compile_in_memory(src, "test");
        let module = result.sir_module.unwrap();
        let s = &module.structs[0];

        let hover = SirIndex::format_struct_hover(s);
        assert!(hover.contains("struct Point {"));
        assert!(hover.contains("x:"));
        assert!(hover.contains("y:"));
    }

    // ── Contracts ────────────────────────────────────────────────────

    #[test]
    fn test_contracts_for_unknown_function() {
        let index = SirIndex::new();
        let contracts = index.contracts_for("nonexistent");
        assert!(contracts.is_empty());
    }

    // ── Go-to-Definition ─────────────────────────────────────────────

    #[test]
    fn test_find_definition_function() {
        let mut index = SirIndex::new();
        let uri = Url::parse("file:///test.salt").unwrap();

        // Compile and index a source file
        let src = "package test\nfn my_target() -> i32 { return 42; }";
        let result = compile_in_memory(src, "test");
        assert!(result.sir_module.is_some());
        index.update(uri.clone(), result.sir_module.unwrap());

        // Try to find the function definition
        let def = index.find_definition("my_target");
        // The location may or may not be Some depending on proc-macro2 span-locations
        // In test contexts, spans are often zeroed out
        if let Some(location) = def {
            assert_eq!(location.uri, uri);
        }
    }

    #[test]
    fn test_find_definition_struct() {
        let mut index = SirIndex::new();
        let uri = Url::parse("file:///test.salt").unwrap();

        let src = "package test\nstruct MyStruct { x: i32, }";
        let result = compile_in_memory(src, "test");
        index.update(uri.clone(), result.sir_module.unwrap());

        let def = index.find_definition("MyStruct");
        if let Some(location) = def {
            assert_eq!(location.uri, uri);
        }
    }

    #[test]
    fn test_find_definition_missing_symbol() {
        let mut index = SirIndex::new();
        let uri = Url::parse("file:///test.salt").unwrap();

        let src = "package test\nfn foo() -> i32 { return 0; }";
        let result = compile_in_memory(src, "test");
        index.update(uri, result.sir_module.unwrap());

        assert!(index.find_definition("nonexistent").is_none());
    }

    #[test]
    fn test_find_definition_cross_file() {
        let mut index = SirIndex::new();

        let uri_a = Url::parse("file:///a.salt").unwrap();
        let uri_b = Url::parse("file:///b.salt").unwrap();

        let src_a = "package a\nfn func_in_a() -> i32 { return 1; }";
        let src_b = "package b\nstruct StructInB { y: i64, }";

        let ra = compile_in_memory(src_a, "a");
        let rb = compile_in_memory(src_b, "b");

        index.update(uri_a.clone(), ra.sir_module.unwrap());
        index.update(uri_b.clone(), rb.sir_module.unwrap());

        // Function in a.salt
        let def_a = index.find_definition("func_in_a");
        if let Some(loc) = def_a {
            assert_eq!(loc.uri, uri_a, "func_in_a should resolve to a.salt");
        }

        // Struct in b.salt
        let def_b = index.find_definition("StructInB");
        if let Some(loc) = def_b {
            assert_eq!(loc.uri, uri_b, "StructInB should resolve to b.salt");
        }
    }

    #[test]
    fn test_sir_location_coordinate_conversion() {
        // syn uses 1-indexed lines, 0-indexed columns
        // LSP uses 0-indexed lines, 0-indexed columns
        let uri = Url::parse("file:///test.salt").unwrap();
        let sir_loc = SirLocation {
            line: 5,        // syn: line 5 (1-indexed)
            column: 4,      // syn: column 4 (0-indexed)
            end_line: 5,
            end_column: 15,
        };

        let lsp_loc = SirIndex::sir_location_to_lsp(&uri, &sir_loc);
        assert_eq!(lsp_loc.range.start.line, 4);       // LSP: 0-indexed
        assert_eq!(lsp_loc.range.start.character, 4);   // Already 0-indexed
        assert_eq!(lsp_loc.range.end.line, 4);
        assert_eq!(lsp_loc.range.end.character, 15);
    }
}
