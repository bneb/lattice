use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use crate::codegen::context::CodegenContext;
use crate::types::{Type, TypeKey};
use crate::grammar::{SaltFn, ImportDecl};

/// The atomic unit of work for the EntityRegistry.
/// Represents a function that needs to be scanned for transitive dependencies
/// and eventually emitted.
#[derive(Clone, Debug)]
pub struct MonomorphizationTask {
    pub identity: TypeKey,
    pub mangled_name: String,
    pub func: SaltFn,
    pub concrete_tys: Vec<Type>,
    pub self_ty: Option<Type>,
    pub imports: Vec<ImportDecl>,
    pub type_map: BTreeMap<String, Type>, // For RAII guard — BTreeMap enforces deterministic iteration
}

/// A specialized function definition ready for emission.
/// This is stored in the Closed Graph.
#[derive(Clone, Debug)]
pub struct SpecializedFn {
    pub func: SaltFn,
    pub concrete_tys: Vec<Type>,
    pub self_ty: Option<Type>,
    pub imports: Vec<ImportDecl>,
    pub is_flattened: bool, // "Linus" Rule: If true, Body is invalid/empty, handled as alias
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HydrationStatus {
    Pending,
    Hydrated, // Ready for emission (in definitions map)
    Emitted,  // Already written to MLIR
}

#[derive(Default)]
pub struct EntityRegistry {
    /// Maps a unique MangleID to the hydrated Function Definition
    /// Key: "std__collections__vec__Vec_i64__push"
    pub definitions: HashMap<String, SpecializedFn>,

    /// The Worklist: A FIFO queue of symbols that need hydration
    pub worklist: VecDeque<MonomorphizationTask>,

    /// The Global Identity Map: Prevents duplicate emission across modules
    /// Tracks MangledIDs that have been requested/processed.
    pub identity_map: HashSet<String>,
    
    /// Tracks status for linear phases
    pub status_map: HashMap<String, HydrationStatus>,

    /// Erased Identities (Linus Rule)
    pub erased_identities: HashSet<String>,

    /// Active LTO Hooks
    pub active_hooks: HashSet<String>,

    /// Global Definitions
    pub globals: HashMap<String, String>,
}

impl EntityRegistry {
    pub fn new() -> Self {
        Self {
            definitions: HashMap::new(),
            worklist: VecDeque::new(),
            identity_map: HashSet::new(),
            status_map: HashMap::new(),
            erased_identities: HashSet::new(),
            active_hooks: HashSet::new(),
            globals: HashMap::new(),
        }
    }

    /// The Identity Interceptor
    /// Checks if a specialization already exists or is pending.
    /// Returns the Mangled ID.
    pub fn request_specialization(
        &mut self, 
        task: MonomorphizationTask
    ) -> String {
        let mangle_id = task.mangled_name.clone();
        
        // The Identity Barrier: 
        // If we've seen this structural identity before, reuse it.
        if self.identity_map.contains(&mangle_id) {
            return mangle_id;
        }

        // Otherwise, queue it for the Hydration Driver.
        self.worklist.push_back(task);
        self.identity_map.insert(mangle_id.clone());
        self.status_map.insert(mangle_id.clone(), HydrationStatus::Pending);
        
        mangle_id
    }
    
    pub fn register_root(&mut self, mangled_name: &str) {
        if !self.identity_map.contains(mangled_name) {
             self.identity_map.insert(mangled_name.to_string());
             // Note: Root tasks usually need to be pushed with full details.
             // If this is just marking 'main' as seen so we don't duplicate, fine.
             // But usually we need to hydrate it.
             // The CodegenContext will likely handle the actual task creation for main.
        }
    }
    
    pub fn is_hydrated(&self, mangled_id: &str) -> bool {
        matches!(self.status_map.get(mangled_id), Some(HydrationStatus::Hydrated) | Some(HydrationStatus::Emitted))
    }
    
    pub fn mark_flattened(&mut self, mangled_id: &str) {
         self.erased_identities.insert(mangled_id.to_string());
    }
    
    pub fn mark_hydrated(&mut self, mangled_id: String, def: SpecializedFn) {
        self.definitions.insert(mangled_id.clone(), def);
        self.status_map.insert(mangled_id, HydrationStatus::Hydrated);
    }

    pub fn register_hook(&mut self, hook_name: &str) {
        self.active_hooks.insert(hook_name.to_string());
    }

    pub fn get_active_hooks(&self) -> Vec<String> {
        let mut hooks: Vec<String> = self.active_hooks.iter().cloned().collect();
        hooks.sort();
        hooks
    }

    pub fn add_global(&mut self, name: String, definition: String) {
        self.globals.insert(name, definition);
    }
}

pub struct SymbolCollector<'a, 'b> {
    _ctx: &'a CodegenContext<'b>,
    // Wraps the registry inside context for collection phases
}

impl<'a, 'b> SymbolCollector<'a, 'b> {
    pub fn new(ctx: &'a CodegenContext<'b>) -> Self {
        Self { _ctx: ctx }
    }
    
    // Legacy support or helper
    // The main logic moves to CodegenContext::hydrate_task/discover_dependencies
}

