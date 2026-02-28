//! Phase 3: Emission State
//! Contains MLIR emission buffers, counters, and caches.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;
use crate::types::Type;
use crate::codegen::const_eval::BootstrapPatch;
use crate::codegen::types::{TypeIDRegistry, ProvenanceMap, OriginMap, GlobalLVN};

/// String interning pool for efficient memory usage
#[derive(Default)]
pub struct StringInterner {
    pool: HashSet<Rc<str>>,
}

impl StringInterner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intern(&mut self, s: &str) -> Rc<str> {
        if let Some(interned) = self.pool.get(s) {
            return Rc::clone(interned);
        }
        let rc: Rc<str> = Rc::from(s);
        self.pool.insert(Rc::clone(&rc));
        rc
    }
}

/// Tensor memory layout information
#[derive(Debug, Clone)]
pub struct TensorLayout {
    pub shape: Vec<usize>,
    pub strides: Vec<usize>,
    pub is_row_major: bool,
}

/// [VERIFIED METAL] Normalize a type name for MLIR alias compatibility.
/// Strips special characters and normalizes underscores.
pub fn normalize_type_name_for_mlir(name: &str) -> String {
    name.replace("__", "_")
        .replace('<', "_")
        .replace('>', "_")
        .replace(',', "_")
        .replace(' ', "")
}

/// Phase 3: MLIR emission and output buffering
#[derive(Default)]
pub struct EmissionState {
    /// SSA value counter for unique register names
    pub val_counter: usize,
    /// Alloca instructions buffer (emitted at function entry)
    pub alloca_out: String,
    /// Declaration output buffer
    pub decl_out: String,
    /// Main definitions output buffer
    pub definitions_buffer: String,
    /// String literals: (name, content, length)
    pub string_literals: Vec<(String, String, usize)>,
    /// Set of already-defined function names
    pub defined_functions: HashSet<String>,
    /// Set of already-defined struct MLIR types
    pub defined_structs: HashSet<String>,
    /// Set of already-defined enum MLIR types
    pub defined_enums: HashSet<String>,
    /// Set of emitted type declarations
    pub emitted_types: HashSet<String>,
    /// External function declarations
    pub external_decls: HashSet<String>,
    /// Pending function declarations: name → declaration string.
    /// These are emitted only if the function is NOT in defined_functions at module assembly time.
    /// This prevents the "redefinition of symbol" MLIR error when a forward declaration
    /// and a full definition coexist for the same function.
    pub pending_func_decls: BTreeMap<String, String>,
    /// Initialized global variables
    pub initialized_globals: HashSet<String>,
    
    // Performance caches
    /// Type layout cache: Type -> (size, alignment)
    pub layout_cache: HashMap<Type, (usize, usize)>,
    /// Tensor layout cache
    pub tensor_layout_cache: HashMap<Type, TensorLayout>,
    /// MLIR type string cache
    pub mlir_type_cache: HashMap<Type, String>,
    /// Struct field type cache
    pub struct_type_cache: Option<HashMap<String, Vec<Type>>>,
    /// String interning pool
    pub interner: StringInterner,
    /// LLVM metadata ID counter
    pub metadata_id_counter: usize,
    
    /// Pending bootstrap patches for global initializers
    pub pending_bootstrap_patches: Vec<BootstrapPatch>,
    
    /// LinAlg dialect initialized flag
    pub linalg_initialized: bool,
    
    /// [VERIFIED METAL] Canonical Type Identity Registry
    /// Maps TypeID → canonical name for O(1) type identity comparison
    pub type_id_registry: TypeIDRegistry,
    
    /// [VERIFIED METAL] Phase 4: Buffered function bodies
    /// These are accumulated during hydration and emitted after fixed-point
    pub body_buffer: String,
    
    /// [VERIFIED METAL] Phase 4: Fixed-point reached flag
    /// Set to true after all specializations are complete
    pub fixed_point_reached: bool,
    
    /// [VERIFIED METAL] Provenance tracking for GEP optimization
    /// Maps local variable names to their element types for Buffer<T>
    pub provenance_map: ProvenanceMap,
    
    /// [PILLAR 1] Origin-Aware Hoisting: Maps SSA values to source variables
    /// Enables Buffer pinning even when struct is loaded into SSA register
    pub origin_map: OriginMap,
    
    /// [PILLAR 2] Global Value Pinning (LVN): Caches global loads in a block
    /// Eliminates redundant addressof + load for globals like COUNTER
    pub global_lvn: GlobalLVN,
    
    /// [SSA PROMOTION] Ephemeral Pointer Registry
    /// SSA values that ARE pointers but lack Type::Reference wrapper.
    /// Used by reinterpret_cast to avoid spilling pointer values to stack.
    /// Example: `let p = reinterpret_cast::<&Pixel>(addr)` → p is kept in register
    pub ephemeral_refs: HashSet<String>,
    
    // === V7.3: Alias Scope Metadata for LLVM Vectorization ===
    /// [V7.3] Alias scope domain ID (distinct ID for module-level domain)
    pub alias_domain_id: usize,
    /// [V7.3] Tensor ID → scope name mapping for noalias metadata
    pub tensor_scopes: HashMap<usize, String>,
    
    /// [V7.5] Fast-math reduction context flag
    /// When true, floating-point arithmetic emits {fastmath = #arith.fastmath<reassoc, contract>}
    /// Enables LLVM to reorder FP operations for vectorization
    pub in_fast_math_reduction: bool,
    /// [V8] Function-level fast-math flag (set by @fast_math attribute)
    /// When true, ALL floating-point operations emit fast-math flags, not just reductions.
    /// This enables LLVM to fully vectorize and reassociate FP arithmetic across the function.
    pub in_fast_math_fn: bool,
    /// [V7.3] Next tensor scope ID counter
    pub next_tensor_scope_id: usize,
    /// [V7.3] Flag indicating alias preamble has been emitted
    pub alias_preamble_emitted: bool,
    
    /// [v0.9.2] Path condition stack for Z3 postcondition verification.
    /// Tracks the branch conditions that are known to hold at the current code point.
    /// Pushed when entering if-then (condition) or if-else (negated condition),
    /// popped when leaving the branch.
    pub path_conditions: Vec<syn::Expr>,
}

impl EmissionState {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Get next unique SSA value ID (1-based, post-increment)
    pub fn next_id(&mut self) -> usize {
        self.val_counter += 1;
        self.val_counter
    }
    
    /// Get next unique metadata ID (1-based, post-increment)
    pub fn next_metadata_id(&mut self) -> usize {
        self.metadata_id_counter += 1;
        self.metadata_id_counter
    }
    
    /// [VERIFIED METAL] Append to body buffer during hydration
    pub fn buffer_body(&mut self, code: &str) {
        self.body_buffer.push_str(code);
    }
    
    /// [VERIFIED METAL] Get buffered body content
    pub fn get_buffered_body(&self) -> &str {
        &self.body_buffer
    }
    
    /// [VERIFIED METAL] Phase 4: Generate canonical MLIR aliases from TypeIDRegistry
    /// 
    /// This method iterates over all TypeIDs discovered during Phases 1-3 and
    /// generates MLIR type aliases that ensure consistent naming across the module.
    /// 
    /// # Arguments
    /// * `struct_registry` - The struct registry for looking up physical layouts
    /// 
    /// # Returns
    /// A string containing all canonical alias definitions
    pub fn generate_canonical_aliases<F>(&self, lookup_struct_layout: F) -> String 
    where
        F: Fn(&str) -> Option<String>
    {
        let mut aliases = String::new();
        aliases.push_str("// --- VERIFIED METAL CANONICAL ALIASES ---\n");
        
        // Track what we've emitted to avoid duplicates
        let mut emitted: HashSet<String> = HashSet::new();
        
        // Iterate over all registered TypeIDs
        for (type_id, canonical_name) in self.type_id_registry.iter() {
            let mlir_alias = normalize_type_name_for_mlir(canonical_name);
            
            // Skip if already emitted
            if emitted.contains(&mlir_alias) {
                continue;
            }
            
            // Try to find the physical layout for this canonical name
            if let Some(physical_layout) = lookup_struct_layout(canonical_name) {
                aliases.push_str(&format!(
                    "// TypeID: {} -> {}\n!struct_{} = {}\n",
                    type_id, canonical_name, mlir_alias, physical_layout
                ));
                emitted.insert(mlir_alias);
            }
        }
        
        aliases.push_str("\n");
        aliases
    }
    
    /// [VERIFIED METAL] Phase 4: Finalize MLIR output after fixed-point
    /// 
    /// This is called only after all specializations are complete.
    /// It generates the complete MLIR module with:
    /// 1. Canonical type aliases from TypeIDRegistry
    /// 2. All buffered function bodies
    /// 
    /// # Arguments
    /// * `header` - The existing MLIR header (struct defs, externals, etc.)
    /// * `lookup_struct_layout` - Function to look up physical layout by canonical name
    /// 
    /// # Returns
    /// The complete, finalized MLIR output
    pub fn finalize<F>(&mut self, header: &str, lookup_struct_layout: F) -> String 
    where
        F: Fn(&str) -> Option<String>
    {
        let mut final_output = String::new();
        
        // 1. Generate canonical aliases from TypeIDRegistry
        let canonical_aliases = self.generate_canonical_aliases(lookup_struct_layout);
        final_output.push_str(&canonical_aliases);
        
        // 2. Append the existing header (struct defs, externals, etc.)
        final_output.push_str(header);
        
        // 3. Append buffered function bodies
        final_output.push_str("\n// --- FUNCTION BODIES ---\n");
        final_output.push_str(&self.get_buffered_body());
        
        // Mark fixed-point as reached
        self.fixed_point_reached = true;
        
        final_output
    }
    
    // === V7.3: Alias Scope Metadata for LLVM Vectorization ===
    
    /// [V7.3] Generate the alias preamble defining the Salt memory domain
    /// This should be called once at the start of MLIR module emission
    pub fn emit_alias_preamble(&mut self) -> String {
        if self.alias_preamble_emitted {
            return String::new();
        }
        self.alias_preamble_emitted = true;
        self.alias_domain_id = 0;
        
        // Define the global Salt memory domain
        format!(
            "#salt_domain = #llvm.alias_scope_domain<id = distinct[{}]<>, description = \"salt_mem_domain\">\n",
            self.alias_domain_id
        )
    }
    
    /// [V7.3] Register a tensor and generate its unique alias scope
    /// Returns the scope identifier (e.g., "#scope_weights")
    pub fn register_tensor_scope(&mut self, description: &str) -> (usize, String) {
        let tensor_id = self.next_tensor_scope_id;
        self.next_tensor_scope_id += 1;
        
        let scope_name = format!("scope_{}", description.replace(' ', "_"));
        self.tensor_scopes.insert(tensor_id, scope_name.clone());
        
        (tensor_id, scope_name)
    }
    
    /// [V7.3] Generate MLIR scope definition for a registered tensor
    pub fn emit_scope_definition(&self, tensor_id: usize, description: &str) -> String {
        let scope_name = self.tensor_scopes.get(&tensor_id)
            .map(|s| s.as_str())
            .unwrap_or("scope_unknown");
        
        // distinct IDs start at 1 (0 is the domain)
        format!(
            "#{} = #llvm.alias_scope<id = distinct[{}]<>, domain = #salt_domain, description = \"{}\">\n",
            scope_name,
            tensor_id + 1,
            description
        )
    }
    
    /// [V7.3] Get comma-separated list of noalias scopes (all scopes except active_tensor_id)
    pub fn get_noalias_scopes(&self, active_tensor_id: usize) -> String {
        self.tensor_scopes
            .iter()
            .filter(|(&id, _)| id != active_tensor_id)
            .map(|(_, scope)| format!("#{}", scope))
            .collect::<Vec<_>>()
            .join(", ")
    }
    
    /// [V7.3] Format load instruction with alias metadata
    pub fn format_load_with_alias(
        &self,
        result: &str,
        ptr: &str,
        ty: &str,
        active_tensor_id: usize,
    ) -> String {
        let alias_scope = self.tensor_scopes.get(&active_tensor_id)
            .map(|s| format!("#{}", s))
            .unwrap_or_default();
        let noalias = self.get_noalias_scopes(active_tensor_id);
        
        if alias_scope.is_empty() {
            format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", result, ptr, ty)
        } else if noalias.is_empty() {
            format!(
                "    {} = llvm.load {} {{ alias_scopes = [{}] }} : !llvm.ptr -> {}\n",
                result, ptr, alias_scope, ty
            )
        } else {
            format!(
                "    {} = llvm.load {} {{ alias_scopes = [{}], noalias_scopes = [{}] }} : !llvm.ptr -> {}\n",
                result, ptr, alias_scope, noalias, ty
            )
        }
    }
    
    /// [V7.3] Format store instruction with alias metadata
    pub fn format_store_with_alias(
        &self,
        value: &str,
        ptr: &str,
        ty: &str,
        active_tensor_id: usize,
    ) -> String {
        let alias_scope = self.tensor_scopes.get(&active_tensor_id)
            .map(|s| format!("#{}", s))
            .unwrap_or_default();
        let noalias = self.get_noalias_scopes(active_tensor_id);
        
        if alias_scope.is_empty() {
            format!("    llvm.store {}, {} : {}, !llvm.ptr\n", value, ptr, ty)
        } else if noalias.is_empty() {
            format!(
                "    llvm.store {}, {} {{ alias_scopes = [{}] }} : {}, !llvm.ptr\n",
                value, ptr, alias_scope, ty
            )
        } else {
            format!(
                "    llvm.store {}, {} {{ alias_scopes = [{}], noalias_scopes = [{}] }} : {}, !llvm.ptr\n",
                value, ptr, alias_scope, noalias, ty
            )
        }
    }
}
