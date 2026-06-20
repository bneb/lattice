//! Phase 1: Discovery State
//! Contains templates, registries, and imports - read-mostly after initialization.

use std::collections::{BTreeMap, HashMap, HashSet};
use crate::grammar::{SaltFile, SaltFn, ImportDecl, StructDef, EnumDef};
use crate::registry::{StructInfo, EnumInfo};
use crate::types::{Type, TypeKey};
use crate::hir::items::Item;
use crate::codegen::collector::EntityRegistry;
use crate::codegen::trait_registry::TraitRegistry;

use crate::codegen::passes::liveness::LivenessResult;

pub mod scanner;

/// Phase 1: Template and registry discovery (read-mostly after initialization)
pub struct DiscoveryState {
    // --- Absorbed from CodegenContext façade ---
    // NOTE: `file` and `registry` are immutable config and remain on CodegenContext
    // to avoid co-borrow panics with RefCell.
    /// Functions that consume arguments by position
    pub consuming_fns: HashMap<String, HashSet<usize>>,
    /// Current package declaration scope
    pub current_package: Option<crate::grammar::PackageDecl>,
    // --- Original discovery fields ---
    /// Struct template definitions (generic structs before monomorphization)
    pub struct_templates: HashMap<String, StructDef>,
    /// Enum template definitions (generic enums before monomorphization)
    pub enum_templates: HashMap<String, EnumDef>,
    /// Resolved struct type information (after monomorphization)
    pub struct_registry: HashMap<TypeKey, StructInfo>,
    /// Resolved enum type information (after monomorphization)
    pub enum_registry: HashMap<TypeKey, EnumInfo>,
    /// Signature-aware method registry - the ONLY method lookup path
    /// All methods must be registered with their signature for overload resolution.
    pub trait_registry: TraitRegistry,
    /// Global variable types
    pub globals: BTreeMap<String, Type>,
    /// Import declarations for current scope
    pub imports: Vec<ImportDecl>,
    /// Generic implementations: name -> (function, imports)
    pub generic_impls: HashMap<String, (SaltFn, Vec<ImportDecl>)>,
    /// The unified entity registry from collector phase
    pub entity_registry: EntityRegistry,
    /// String prefix handlers: prefix -> handler function name
    /// e.g., "f" -> "std__string__fstring_handler", "sql" -> "sql_handler"
    pub string_prefix_handlers: HashMap<String, String>,
    /// Comptime phase tracking for bootstrap safety
    /// During Bootstrap: use hardcoded Rust handlers (prevents circular dependency)
    /// After Ready: use Salt-native handlers from string_prefix_handlers
    pub comptime_ready: bool,
    /// Pulse function registry: name -> (frequency_hz, tier)
    /// Used by yield injection pass to determine deadline checking behavior
    pub pulse_functions: HashMap<String, (u32, u8)>,
    /// Type Origin Registry: mangled_type_name -> module_package
    /// Maps each struct/enum to the module where it was first defined.
    /// This is the "KeuOS Home" — only the home module may emit trait impls for a type.
    pub type_origins: HashMap<String, String>,
    /// Trait Origin Registry: trait_name -> module_package
    /// Tracks which module defined each trait for orphan rule enforcement.
    pub trait_origins: HashMap<String, String>,
    /// Trait Impl Registry: (type_name, trait_name) -> module_package
    /// Tracks all trait implementations for duplicate detection and coherence validation.
    pub trait_impls: HashMap<(String, String), String>,
    /// Cross-yield liveness results: fn_name -> LivenessResult
    /// Populated by the liveness analysis phase for @yielding/@pulse functions.
    /// Used by emit_fn to divert async functions to StateMachineEmitter.
    pub liveness_results: HashMap<String, LivenessResult>,
    /// HIR async items: fn_name -> lowered Items (struct + step fn)
    /// Populated by lower_async_fn_cfg. When emit_fn sees a @yielding function,
    /// it checks here first; if items exist, it bypasses AST codegen entirely.
    pub hir_async_items: HashMap<String, Vec<Item>>,
}

impl DiscoveryState {
    pub fn new(file: &SaltFile) -> Self {
        let current_package = file.package.clone();
        Self {
            consuming_fns: HashMap::new(),
            current_package,
            struct_templates: HashMap::new(),
            enum_templates: HashMap::new(),
            struct_registry: HashMap::new(),
            enum_registry: HashMap::new(),
            trait_registry: TraitRegistry::default(),
            globals: BTreeMap::new(),
            imports: Vec::new(),
            generic_impls: HashMap::new(),
            entity_registry: EntityRegistry::default(),
            string_prefix_handlers: HashMap::new(),
            comptime_ready: false,
            pulse_functions: HashMap::new(),
            type_origins: HashMap::new(),
            trait_origins: HashMap::new(),
            trait_impls: HashMap::new(),
            liveness_results: HashMap::new(),
            hir_async_items: HashMap::new(),
        }
    }

    /// Register a type's "KeuOS Home" module.
    /// First-writer-wins: prevents type hijacking across modules.
    pub fn register_type_home(&mut self, type_name: String, module_package: String) {
        self.type_origins.entry(type_name).or_insert(module_package);
    }

    /// Register a trait's home module.
    pub fn register_trait_home(&mut self, trait_name: String, module_package: String) {
        self.trait_origins.entry(trait_name).or_insert(module_package);
    }

    pub fn require_local_function(&mut self, mangled_name: &str, file: &crate::grammar::SaltFile, _expansion: &mut crate::codegen::phases::ExpansionState) -> bool {
        // Check if already requested in the global registry
        if self.entity_registry.identity_map.contains(mangled_name) {
            return true;
        }

        let current_pkg_prefix = if let Some(pkg) = &file.package {
             crate::common::mangling::Mangler::mangle(&pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>()) + "__"
        } else {
             String::new()
        };

        // Try to find in current file
        let mut result = None;
        for item in &file.items {
            if let crate::grammar::Item::Fn(f) = item {
                let my_mangled = if f.attributes.iter().any(|a| a.name == "no_mangle" || a.name == "export") {
                    f.name.to_string()
                } else {
                    format!("{}{}", current_pkg_prefix, f.name)
                };
                if my_mangled == mangled_name {
                    let path = if let Some(pkg) = &file.package {
                        pkg.name.iter().map(|id| id.to_string()).collect()
                    } else {
                        vec![]
                    };
                    let identity = crate::types::TypeKey {
                        path,
                        name: f.name.to_string(),
                        specialization: None,
                    };
                    result = Some(crate::codegen::collector::MonomorphizationTask {
                        identity,
                        mangled_name: mangled_name.to_string(),
                        func: f.clone(),
                        concrete_tys: vec![],
                        self_ty: None,
                        imports: crate::codegen::context::CodegenContext::compute_full_imports(file),
                        type_map: std::collections::BTreeMap::new(),
                    });
                    break;
                }
            }
        }

        if let Some(_task) = result {
            self.entity_registry.identity_map.insert(mangled_name.to_string());
            return true;
        }
        false
    }

    /// The KeuOS Check: Does this module own the type?
    pub fn is_type_home(&self, type_name: &str, current_module: &str) -> bool {
        match self.type_origins.get(type_name) {
            Some(home_module) => home_module == current_module,
            None => false,
        }
    }

    /// Check if this module owns the trait.
    pub fn is_trait_home(&self, trait_name: &str, current_module: &str) -> bool {
        match self.trait_origins.get(trait_name) {
            Some(home_module) => home_module == current_module,
            None => false,
        }
    }

    /// Register a trait implementation and check for duplicates.
    /// Returns Err if a duplicate (Type, Trait) pair is found in the same module.
    pub fn register_trait_impl(&mut self, type_name: String, trait_name: String, module_package: String) -> Result<(), String> {
        let key = (type_name.clone(), trait_name.clone());
        if let Some(existing_module) = self.trait_impls.get(&key) {
            if *existing_module == module_package {
                return Err(format!(
                    "Duplicate Implementation: trait '{}' is already implemented for type '{}' in module '{}'",
                    trait_name, type_name, module_package
                ));
            }
            // Different modules — will be caught by validate_coherence
        }
        self.trait_impls.insert(key, module_package);
        Ok(())
    }

    /// Validate coherence: every impl must reside in the Home of
    /// either the Type or the Trait. Orphan implementations are rejected.
    pub fn validate_coherence(&self) -> Result<(), String> {
        for ((type_name, trait_name), impl_module) in &self.trait_impls {
            let is_type_local = self.is_type_home(type_name, impl_module);
            let is_trait_local = self.is_trait_home(trait_name, impl_module);

            if !is_type_local && !is_trait_local {
                return Err(format!(
                    "Orphan Implementation: Cannot implement trait '{}' for type '{}' in module '{}'. \
                     Salt requires that implementations reside in the 'Home' of either the Type or the Trait.",
                    trait_name, type_name, impl_module
                ));
            }
        }
        Ok(())
    }
}

