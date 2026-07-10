// src/codegen/module_loader.rs

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::fs;

use crate::grammar::{SaltFile, ImportDecl, Item};
use crate::registry::{Registry, ModuleInfo};

const MAX_RECURSION_DEPTH: usize = 33;

pub struct ModuleLoader {
    /// Search roots: [./src, ../std, /usr/lib/salt/std]
    root_paths: Vec<PathBuf>,
    /// Tracks modules already processed to avoid redundant parsing
    loaded_modules: HashSet<String>,
    /// Tracks modules currently in the recursion stack to detect cycles
    loading_stack: Vec<String>,
    /// Adjacency list for the dependency graph (A depends on B, C)
    dependency_graph: HashMap<String, Vec<String>>,
    /// Map of namespace to parsed SaltFile
    pub loaded_files: HashMap<String, SaltFile>,
    /// Combined AST accumulator (optional, for final link)
    pub combined_ast: SaltFile,
}

impl ModuleLoader {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self {
            root_paths: roots,
            loaded_modules: HashSet::new(),
            loading_stack: Vec::new(),
            dependency_graph: HashMap::new(),
            loaded_files: HashMap::new(),
            combined_ast: SaltFile::empty(),
        }
    }

    /// Maps a namespace (e.g., "std.collections.Vec") to a physical path.
    /// Priority 1: root/std/collections/Vec.salt
    /// Priority 2: root/std/collections/Vec/mod.salt
    pub fn resolve_filepath(&self, namespace: &str) -> Result<PathBuf, String> {
        let is_std = namespace.starts_with("std.");
        let relative_path_str = namespace.replace('.', "/");
        let relative_path = Path::new(&relative_path_str);

        // Define which roots are allowed for this namespace
        let search_roots = if is_std {
            // ONLY search the official stdlib path for 'std' imports
            self.root_paths.iter().filter(|p| {
                let s = p.to_string_lossy();
                s.ends_with("/std") || s == "std" || s.ends_with("..") || s.contains("/std/") || p.join("std").exists()
            }).collect::<Vec<_>>()
        } else {
            // Search all roots (local project first) for non-std imports
            self.root_paths.iter().collect::<Vec<_>>()
        };

        for root in search_roots {
            // Option A: Check for a direct file match (namespace.salt)
            let direct_file = root.join(relative_path).with_extension("salt");
            if direct_file.exists() {
                return Ok(direct_file);
            }

            // Option B: Check for a directory root (namespace/mod.salt)
            let mod_file = root.join(relative_path).join("mod.salt");
            if mod_file.exists() {
                return Ok(mod_file);
            }

            // Option C: Try without the 'std.' prefix if we are already in the std root
            if is_std {
                let stripped = &relative_path_str[4..]; // strip "std/"
                let stripped_path = Path::new(stripped);
                
                let direct_stripped = root.join(stripped_path).with_extension("salt");
                if direct_stripped.exists() { return Ok(direct_stripped); }
                
                let mod_stripped = root.join(stripped_path).join("mod.salt");
                if mod_stripped.exists() { return Ok(mod_stripped); }

                // Try lowercase versions of stripped
                let lower_stripped = stripped.to_lowercase();
                let direct_lower = root.join(&lower_stripped).with_extension("salt");
                if direct_lower.exists() { return Ok(direct_lower); }
                
                let mod_lower = root.join(&lower_stripped).join("mod.salt");
                if mod_lower.exists() { return Ok(mod_lower); }
            }

            // Option D: Try lowercase version of full path
            let lower_relative = relative_path_str.to_lowercase();
            let direct_lower = root.join(&lower_relative).with_extension("salt");
            if direct_lower.exists() {
                return Ok(direct_lower);
            }
            let mod_lower = root.join(&lower_relative).join("mod.salt");
            if mod_lower.exists() {
                return Ok(mod_lower);
            }
        }

        Err(format!(
            "Could not resolve module '{}'. Searched in: {:?}. Path shadowing protected.",
            namespace, self.root_paths
        ))
    }

    /// Recursively loads a module and its dependencies
    /// Check the bundled stdlib for a namespace before hitting the filesystem.
    fn try_bundled_stdlib(&self, namespace: &str) -> Option<String> {
        if !namespace.starts_with("std") {
            return None;
        }
        let bundle = crate::stdlib_bundle::stdlib_sources();
        // Try exact match first, then try stripping the last component
        // (e.g., std.core.ptr.Ptr → std.core.ptr)
        let parts: Vec<&str> = namespace.split('.').collect();
        for len in (1..=parts.len()).rev() {
            let key = parts[..len].join(".");
            if bundle.contains_key(&key) {
                return bundle.get(&key).map(|s| s.to_string());
            }
        }
        None
    }

    pub fn load_module(&mut self, namespace: &str, registry: &mut Registry) -> Result<(), String> {
        // 1. Check if already loaded
        if self.loaded_modules.contains(namespace) {
            return Ok(());
        }
        
        // 2. 33-Level Recursion Guard / Cycle Detection
        if self.loading_stack.len() > MAX_RECURSION_DEPTH {
            return Err(format!("Module nesting depth exceeded or circularity at '{}'", namespace));
        }
        if self.loading_stack.contains(&namespace.to_string()) {
            return Err(format!("Circular dependency detected: {} -> {}", self.loading_stack.join(" -> "), namespace));
        }

        self.loading_stack.push(namespace.to_string());

        // 3. Check bundled stdlib first (no filesystem needed)
        if let Some(source) = self.try_bundled_stdlib(namespace) {
            let processed = crate::preprocess(&source);
            if let Ok(ast) = syn::parse_str::<crate::grammar::SaltFile>(&processed) {
                let mut info = ModuleInfo::new(namespace);
                info.imports = ast.imports.clone();
                for item in &ast.items {
                    self.extract_item_info(item, &mut info, &ast.imports);
                }
                registry.register(info);
                self.merge_into_combined(ast.clone());
                self.loaded_files.insert(namespace.to_string(), ast.clone());
                self.loaded_modules.insert(namespace.to_string());
                // Recursively load the stdlib module's own imports
                let sub_imports: Vec<String> = ast.imports.iter()
                    .map(|imp| imp.name.iter().map(|id| id.to_string()).collect::<Vec<_>>().join("."))
                    .collect();
                self.loading_stack.pop();
                for sub in &sub_imports {
                    let _ = self.load_module(sub, registry);
                }
                return Ok(());
            }
        }

        // 4. Resolve Filepath
        let path = match self.resolve_filepath(namespace) {
            Ok(p) => p,
            Err(e) => {
                // Fallback: Check if this is an item import (e.g., std.io.print.print_int)
                // Try to load the parent module (std.io.print)
                if let Some(r) = namespace.rfind('.') {
                    let parent = &namespace[0..r];
                    match self.resolve_filepath(parent) {
                        Ok(_p) => {
                            // Parent exists, load it instead!
                            // We recurse into load_module with parent, ensuring it's loaded.
                            // The current import (namespace) will be satisifed by the parent's items.
                            let res = self.load_module(parent, registry);
                            if res.is_ok() {
                                self.loaded_modules.insert(namespace.to_string());
                            }
                            self.loading_stack.pop();
                            return res;
                        },
                        Err(_) => return Err(e), // Return original error
                    }
                }
                return Err(e);
            }
        };

        let code = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read '{}': {}", path.display(), e))?;
        
        let processed = crate::preprocess(&code);
        let ast: SaltFile = match syn::parse_str(&processed) {
            Ok(file) => file,
            Err(e) => {
                let line_idx = e.span().start().line - 1;
                let lines: Vec<&str> = processed.lines().collect();
                let snippet = if line_idx < lines.len() {
                    lines[line_idx].trim()
                } else {
                    "<EOF>"
                };
                self.loading_stack.pop();
                return Err(format!("Failed to parse '{}': {} at line {}: '{}'", path.display(), e, line_idx + 1, snippet));
            }
        };

        // 4. Record Dependencies for Topological Sort
        let deps = ast.get_use_namespaces();
        self.dependency_graph.insert(namespace.to_string(), deps.clone());

        // 5. Recurse into dependencies
        for dep in deps {
            if let Err(e) = self.load_module(&dep, registry) {
                self.loading_stack.pop();
                return Err(e);
            }
        }

        // Register module info in registry for cross-module symbol resolution
        let mut info = ModuleInfo::new(namespace);
        info.imports = ast.imports.clone();
        for item in &ast.items {
            self.extract_item_info(item, &mut info, &ast.imports);
        }
        registry.register(info);

        // 6. Finalize this module
        self.merge_into_combined(ast.clone());
        self.loaded_files.insert(namespace.to_string(), ast);
        self.loaded_modules.insert(namespace.to_string());
        self.loading_stack.pop();

        Ok(())
    }

    fn merge_into_combined(&mut self, ast: SaltFile) {
        self.combined_ast.imports.extend(ast.imports);
        self.combined_ast.items.extend(ast.items);
    }

    /// Extract type/function info from an item for the registry
    fn extract_item_info(&self, item: &Item, info: &mut ModuleInfo, imports: &[ImportDecl]) {
        let pkg_mangled = info.package.replace(".", "__");
        
        match item {
            Item::Fn(f) => {
                let args: Vec<crate::types::Type> = f.args.iter()
                    .filter_map(|arg| arg.ty.as_ref().and_then(crate::types::Type::from_syn))
                    .collect();
                let ret = f.ret_type.as_ref()
                    .and_then(crate::types::Type::from_syn)
                    .unwrap_or(crate::types::Type::Unit);
                info.functions.insert(f.name.to_string(), (args, ret));
                
                // Store AST for monomorphization (Global Function Discovery)
                info.function_templates.insert(f.name.to_string(), f.clone());
                
                // Register categorical export
                let generic_count = f.generics.as_ref().map(|g| g.params.len()).unwrap_or(0);
                info.exports.insert(f.name.to_string(), crate::registry::ExportMetadata {
                    fqn: format!("{}__{}" , pkg_mangled, f.name),
                    kind: crate::registry::SymbolKind::Function,
                    generic_params: generic_count,
                });
            }
            Item::ExternFn(ef) => {
                let args: Vec<crate::types::Type> = ef.args.iter()
                    .filter_map(|arg| arg.ty.as_ref().and_then(crate::types::Type::from_syn))
                    .collect();
                let ret = ef.ret_type.as_ref()
                    .and_then(crate::types::Type::from_syn)
                    .unwrap_or(crate::types::Type::Unit);
                info.functions.insert(ef.name.to_string(), (args, ret));
                
                // Extern functions are Intrinsics
                info.exports.insert(ef.name.to_string(), crate::registry::ExportMetadata {
                    fqn: ef.name.to_string(),  // Externs use raw name
                    kind: crate::registry::SymbolKind::Intrinsic,
                    generic_params: 0,
                });
            }
            Item::Struct(s) => {
                let generic_count = s.generics.as_ref().map(|g| g.params.len()).unwrap_or(0);
                
                if s.generics.is_some() {
                    info.struct_templates.insert(s.name.to_string(), s.clone());
                } else {
                    let fields: Vec<(String, crate::types::Type)> = s.fields.iter()
                        .filter_map(|f| {
                            crate::types::Type::from_syn(&f.ty)
                                .map(|ty| (f.name.to_string(), ty))
                        })
                        .collect();
                    info.structs.insert(s.name.to_string(), fields);
                }
                
                // Register categorical export (LeafType)
                info.exports.insert(s.name.to_string(), crate::registry::ExportMetadata {
                    fqn: format!("{}__{}" , pkg_mangled, s.name),
                    kind: crate::registry::SymbolKind::LeafType,
                    generic_params: generic_count,
                });
            }
            Item::Enum(e) => {
                let generic_count = e.generics.as_ref().map(|g| g.params.len()).unwrap_or(0);
                
                if e.generics.is_some() {
                    info.enum_templates.insert(e.name.to_string(), e.clone());
                } else {
                    // Placeholder for concrete enums
                }
                
                // Register categorical export (LeafType)
                info.exports.insert(e.name.to_string(), crate::registry::ExportMetadata {
                    fqn: format!("{}__{}" , pkg_mangled, e.name),
                    kind: crate::registry::SymbolKind::LeafType,
                    generic_params: generic_count,
                });
            }
            Item::Const(c) => {
                let eval = crate::evaluator::Evaluator::new();
                if let Ok(crate::evaluator::ConstValue::Integer(val)) = eval.eval_expr(&c.value) {
                    info.constants.insert(c.name.to_string(), val);
                }
            }
            Item::Impl(i) => {
                // Store impl for cross-crate specialization.
                // We store the AST and the imports context.
                info.impls.push((i.clone(), imports.to_vec()));
            }
            _ => {}
        }
    }

    /// Generates the compilation order using DFS post-order
    pub fn get_compilation_order(&self) -> Result<Vec<String>, String> {
        let mut order = Vec::new();
        let mut visited = HashSet::new();
        let mut temp_mark = HashSet::new();

        // Sort keys for deterministic compilation order.
        // HashMap iteration is non-deterministic in Rust.
        let mut modules: Vec<&String> = self.dependency_graph.keys().collect();
        modules.sort();
        for module in modules {
            self.topological_visit(module, &mut visited, &mut temp_mark, &mut order)?;
        }

        Ok(order)
    }

    fn topological_visit(
        &self,
        node: &String,
        visited: &mut HashSet<String>,
        temp_mark: &mut HashSet<String>,
        order: &mut Vec<String>,
    ) -> Result<(), String> {
        if temp_mark.contains(node) {
            return Err(format!("Cycle detected in module graph involving: {}", node));
        }
        if !visited.contains(node) {
            temp_mark.insert(node.clone());
            if let Some(deps) = self.dependency_graph.get(node) {
                for dep in deps {
                    self.topological_visit(dep, visited, temp_mark, order)?;
                }
            }
            temp_mark.remove(node);
            visited.insert(node.clone());
            order.push(node.clone());
        }
        Ok(())
    }
}

