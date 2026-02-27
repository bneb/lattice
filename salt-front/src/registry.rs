use std::collections::HashMap;
use crate::types::Type;
use crate::grammar::{StructDef, EnumDef};

// =============================================================================
// Registry V3.0: Categorical Symbol Resolution
// =============================================================================
// Every symbol is tagged with a SymbolKind to enable correct resolution:
// - Namespace: Container of other symbols (module) - searchable via wildcard
// - LeafType: Struct/Enum definition - cannot have child symbols
// - Function: Callable entity - searchable via wildcard  
// - Intrinsic: Compiler builtin behavior
// - Alias: Pointer to an FQN (mangled or friendly name)
// =============================================================================

/// Symbol category in the Registry taxonomy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    /// Container of other symbols (a Module). Searchable via `*`.
    Namespace,
    /// Static definition (Struct/Enum). Cannot have child symbols.
    LeafType,
    /// Callable entity. Searchable via `*`.
    Function,
    /// Compiler-builtin behavior.
    Intrinsic,
    /// Pointer to a Fully Qualified Name.
    Alias,
}

/// Export metadata for categorical resolution
#[derive(Debug, Clone)]
pub struct ExportMetadata {
    /// Fully qualified mangled name (e.g., "std__string__InterpolatedStringHandler")
    pub fqn: String,
    /// Category for resolution logic
    pub kind: SymbolKind,
    /// Required generic argument count (0 for non-generic)
    pub generic_params: usize,
}


#[derive(Debug, Clone)]
pub struct StructInfo {
    pub name: String,
    pub fields: HashMap<String, (usize, Type)>,
    pub field_order: Vec<Type>,
    pub field_alignments: Vec<Option<u32>>,
    pub template_name: Option<String>,
    pub specialization_args: Vec<Type>,
}

#[derive(Debug, Clone)]
pub struct EnumInfo {
    pub name: String,
    pub variants: Vec<(String, Option<Type>, i32)>,
    pub max_payload_size: usize,
    pub template_name: Option<String>,
    pub specialization_args: Vec<Type>,
}

#[derive(Clone, Debug)]
pub struct ModuleInfo {
    pub package: String,
    /// V3.0: Categorical exports for symbol resolution
    pub exports: HashMap<String, ExportMetadata>,
    pub functions: HashMap<String, (Vec<Type>, Type)>,
    pub structs: HashMap<String, Vec<(String, Type)>>, // name -> field types (concrete)
    pub struct_templates: HashMap<String, StructDef>,  // name -> AST (generic)
    pub enum_templates: HashMap<String, EnumDef>,
    pub function_templates: HashMap<String, crate::grammar::SaltFn>, // name -> AST
    pub enums: HashMap<String, EnumInfo>,
    pub constants: HashMap<String, i64>,
    pub globals: HashMap<String, Type>,
    pub impls: Vec<(crate::grammar::SaltImpl, Vec<crate::grammar::ImportDecl>)>,
    pub imports: Vec<crate::grammar::ImportDecl>,
}

impl ModuleInfo {
    pub fn new(package: &str) -> Self {
        Self {
            package: package.to_string(),
            exports: HashMap::new(),
            functions: HashMap::new(),
            structs: HashMap::new(),
            struct_templates: HashMap::new(),
            enum_templates: HashMap::new(),
            function_templates: HashMap::new(),
            enums: HashMap::new(),
            constants: HashMap::new(),
            globals: HashMap::new(),
            impls: Vec::new(),
            imports: Vec::new(),
        }
    }
}


pub struct Registry {
    pub modules: HashMap<String, ModuleInfo>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    pub fn new() -> Self {
        Self { modules: HashMap::new() }
    }

    pub fn register(&mut self, info: ModuleInfo) {
        self.modules.insert(info.package.clone(), info);
    }

    pub fn resolve_fn(&self, package: &str, name: &str) -> Option<&(Vec<Type>, Type)> {
        self.modules.get(package)?.functions.get(name)
    }

    pub fn resolve_struct(&self, package: &str, name: &str) -> Option<&Vec<(String, Type)>> {
        self.modules.get(package)?.structs.get(name)
    }
}
