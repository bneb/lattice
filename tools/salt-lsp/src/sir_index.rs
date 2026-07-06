//! SIR Index — In-memory symbol index from salt-front's AST/SIR pipeline
//!
//! Uses salt-front as a library crate for zero-I/O, in-memory compilation.

use std::collections::HashMap;
use tower_lsp::lsp_types::Url;

pub use saltc::codegen::sir::types::{
    SirModule, SirFunction, SirStruct, SirParam, SirContract, SirBlock,
    SirInstruction, SirType, SirValue, SirLocation,
};

use tower_lsp::lsp_types::{Location, Position, Range};

use crate::sir_display;

/// In-memory index of SIR data across all open files.
pub struct SirIndex {
    modules: HashMap<Url, SirModule>,
}

impl Default for SirIndex {
    fn default() -> Self { Self::new() }
}

impl SirIndex {
    pub fn new() -> Self {
        SirIndex { modules: HashMap::new() }
    }

    pub fn update(&mut self, uri: Url, module: SirModule) {
        self.modules.insert(uri, module);
    }

    pub fn remove(&mut self, uri: &Url) {
        self.modules.remove(uri);
    }

    pub fn lookup_function(&self, name: &str) -> Option<&SirFunction> {
        for module in self.modules.values() {
            for func in &module.functions {
                if func.name == name { return Some(func); }
            }
        }
        None
    }

    pub fn lookup_struct(&self, name: &str) -> Option<&SirStruct> {
        for module in self.modules.values() {
            for s in &module.structs {
                if s.name == name { return Some(s); }
            }
        }
        None
    }

    pub fn contracts_for(&self, fn_name: &str) -> Vec<&SirContract> {
        self.lookup_function(fn_name)
            .map(|f| f.contracts.iter().collect())
            .unwrap_or_default()
    }

    pub fn module_for(&self, uri: &Url) -> Option<&SirModule> {
        self.modules.get(uri)
    }

    pub fn all_function_names(&self) -> Vec<&str> {
        let mut names = Vec::new();
        for module in self.modules.values() {
            for func in &module.functions {
                names.push(func.name.as_str());
            }
        }
        names
    }

    pub fn all_struct_names(&self) -> Vec<&str> {
        let mut names = Vec::new();
        for module in self.modules.values() {
            for s in &module.structs {
                names.push(s.name.as_str());
            }
        }
        names
    }

    // Delegate display formatting to sir_display module
    pub fn format_function_hover(func: &SirFunction) -> String {
        sir_display::format_function_hover(func)
    }

    pub fn format_struct_hover(s: &SirStruct) -> String {
        sir_display::format_struct_hover(s)
    }

    // ── Go-to-Definition ─────────────────────────────────────────────

    fn sir_location_to_lsp(uri: &Url, loc: &SirLocation) -> Location {
        Location {
            uri: uri.clone(),
            range: Range {
                start: Position::new(
                    loc.line.saturating_sub(1) as u32,
                    loc.column as u32,
                ),
                end: Position::new(
                    loc.end_line.saturating_sub(1) as u32,
                    loc.end_column as u32,
                ),
            },
        }
    }

    pub fn find_definition(&self, symbol_name: &str) -> Option<Location> {
        for (uri, module) in &self.modules {
            for func in &module.functions {
                if func.name == symbol_name {
                    if let Some(ref loc) = func.location {
                        return Some(Self::sir_location_to_lsp(uri, loc));
                    }
                }
            }
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

    // ── Find References (new) ────────────────────────────────────────

    /// Find all references to a symbol name across all indexed modules.
    /// Returns locations where the symbol appears in return types and struct field types.
    pub fn find_references(&self, _symbol_name: &str) -> Vec<Location> {
        let mut refs = Vec::new();
        for (uri, module) in &self.modules {
            for func in &module.functions {
                // Check return type
                if type_contains_name(&func.return_type, _symbol_name) {
                    if let Some(ref loc) = func.location {
                        refs.push(Self::sir_location_to_lsp(uri, loc));
                    }
                }
            }
            for s in &module.structs {
                for field in &s.fields {
                    if type_contains_name(&field.ty, _symbol_name) {
                        // Use struct location as proxy for field type reference
                        if let Some(ref loc) = s.location {
                            refs.push(Self::sir_location_to_lsp(uri, loc));
                            break; // one ref per struct
                        }
                    }
                }
            }
        }
        refs
    }

    // ── Document Symbols (new) ───────────────────────────────────────

    /// Produce document symbols (outline) for a given file.
    pub fn document_symbols_for(&self, uri: &Url) -> Vec<DocumentSymbolEntry> {
        let module = match self.module_for(uri) {
            Some(m) => m,
            None => return vec![],
        };

        let mut symbols = Vec::new();
        for func in &module.functions {
            let (line, col) = func.location.as_ref()
                .map(|l| (l.line.saturating_sub(1) as u32, l.column as u32))
                .unwrap_or((0, 0));
            symbols.push(DocumentSymbolEntry {
                name: func.name.clone(),
                kind: SymbolKind::FUNCTION,
                line, column: col,
                detail: format!("fn({} params) -> {}",
                    func.params.len(),
                    sir_display::format_type(&func.return_type)),
                is_pub: func.is_pub,
            });
        }
        for s in &module.structs {
            let (line, col) = s.location.as_ref()
                .map(|l| (l.line.saturating_sub(1) as u32, l.column as u32))
                .unwrap_or((0, 0));
            symbols.push(DocumentSymbolEntry {
                name: s.name.clone(),
                kind: SymbolKind::STRUCT,
                line, column: col,
                detail: format!("struct ({} fields)", s.fields.len()),
                is_pub: true, // structs are always visible in their module
            });
        }
        symbols
    }
}

/// Check if a SirType contains a named type reference.
fn type_contains_name(ty: &SirType, name: &str) -> bool {
    match ty {
        SirType::Struct(n) => n == name,
        SirType::Ptr(inner) => type_contains_name(inner, name),
        SirType::Array(inner, _) => type_contains_name(inner, name),
        _ => false,
    }
}

#[derive(Debug, Clone)]
pub enum SymbolKind { FUNCTION, STRUCT }

#[derive(Debug, Clone)]
pub struct DocumentSymbolEntry {
    pub name: String,
    pub kind: SymbolKind,
    pub line: u32,
    pub column: u32,
    pub detail: String,
    pub is_pub: bool,
}

// ── In-Memory Compilation ──────────────────────────────────────────

pub struct CompileResult {
    pub sir_module: Option<SirModule>,
    pub error: Option<String>,
}

/// Compile Salt source text in-memory via salt-front's library API.
pub fn compile_in_memory(source: &str, module_name: &str) -> CompileResult {
    use saltc::grammar::SaltFile;
    use saltc::codegen::sir::sir_emit::extract_sir_from_ast;

    let preprocessed = saltc::preprocess(source);
    let ast: SaltFile = match syn::parse_str(&preprocessed) {
        Ok(ast) => ast,
        Err(err) => {
            let span = err.span();
            let start = span.start();
            return CompileResult {
                sir_module: None,
                error: Some(format!("{}:{}:{}: {}", module_name, start.line, start.column, err)),
            };
        }
    };
    let sir_module = extract_sir_from_ast(&ast, module_name);
    CompileResult { sir_module: Some(sir_module), error: None }
}
