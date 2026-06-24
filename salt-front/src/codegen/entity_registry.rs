use std::collections::{HashMap, VecDeque, HashSet};
use crate::types::Type;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct HydrationTask {
    pub template_name: String,
    pub concrete_types: Vec<Type>,
    pub mangle_id: String,
}

pub struct EntityRegistry {
    /// Tier 1: The Canonical Identity Map
    /// Key: Structural Hash (TemplateID + Hashed Concrete Types) -> MangleID
    pub identity_map: HashMap<u64, String>,

    /// Tier 2: The Hydration Worklist
    /// A queue of unique tasks that have been discovered but not yet emitted.
    pub worklist: VecDeque<HydrationTask>,

    /// The Definition Store
    /// Finalized MLIR/LLVM fragments associated with their MangleID.
    pub definitions: HashMap<String, String>,

    /// Erased Identities (Linus Rule)
    /// Set of MangleIDs that have been erased/flattened.
    pub erased_identities: HashSet<String>,

    /// Active LTO Hooks
    /// Set of external symbols requested during hydration.
    pub active_hooks: HashSet<String>,

    /// Global Definitions
    /// Deduplicated global variables (name -> definition).
    pub globals: HashMap<String, String>,
}

impl EntityRegistry {
    pub fn new() -> Self {
        Self {
            identity_map: HashMap::new(),
            worklist: VecDeque::new(),
            definitions: HashMap::new(),
            erased_identities: HashSet::new(),
            active_hooks: HashSet::new(),
            globals: HashMap::new(),
        }
    }

    /// Calculates a structural hash for a template instantiation
    pub fn calculate_hash(template_name: &str, concrete_types: &[Type]) -> u64 {
        let mut hasher = DefaultHasher::new();
        template_name.hash(&mut hasher);
        for ty in concrete_types {
            // Assuming Type implements Hash, otherwise a custom hash helper is needed
            // If Type doesn't implement Hash, a Debug string or similar must be used.
            // For now, let's assume standard Hash derive on Type or use to_string proxy.
            format!("{:?}", ty).hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Registers a potential task. Returns the canonical MangleID.
    /// If it's a new unique instantiation, adds it to the worklist.
    pub fn register_task(&mut self, template_name: &str, concrete_types: Vec<Type>, mangle_id_hint: String) -> String {
        let hash = Self::calculate_hash(template_name, &concrete_types);

        if let Some(existing_id) = self.identity_map.get(&hash) {
            return existing_id.clone();
        }

        // New identity
        let mangle_id = mangle_id_hint; // iterate or de-duplicate if needed, but for now trust hint or use hash
        self.identity_map.insert(hash, mangle_id.clone());
        
        self.worklist.push_back(HydrationTask {
            template_name: template_name.to_string(),
            concrete_types,
            mangle_id: mangle_id.clone(),
        });

        mangle_id
    }
    
    // specialized helper for main/roots
    pub fn register_root(&mut self, name: &str) {
        // Roots don't have template types, but are treated as empty
        let task = HydrationTask {
            template_name: name.to_string(),
            concrete_types: vec![],
            mangle_id: name.to_string(),
        };
        // Avoid calculating hash for main if hash calculation is unnecessary; be consistent
        let hash = Self::calculate_hash(name, &[]);
        if !self.identity_map.contains_key(&hash) {
            self.identity_map.insert(hash, name.to_string());
            self.worklist.push_back(task);
        }
    }

    pub fn pop_worklist(&mut self) -> Option<HydrationTask> {
        self.worklist.pop_front()
    }

    pub fn is_hydrated(&self, mangle_id: &str) -> bool {
        self.definitions.contains_key(mangle_id) || self.erased_identities.contains(mangle_id)
    }

    pub fn mark_erased(&mut self, mangle_id: &str) {
        self.erased_identities.insert(mangle_id.to_string());
    }

    pub fn save_definition(&mut self, mangle_id: &str, body: String) {
        self.definitions.insert(mangle_id.to_string(), body);
    }
    
    pub fn register_hook(&mut self, hook_name: &str) {
        self.active_hooks.insert(hook_name.to_string());
    }
    
    pub fn get_active_hooks(&self) -> Vec<String> {
        let mut hooks: Vec<String> = self.active_hooks.iter().cloned().collect();
        hooks.sort();
        hooks
    }
    
    pub fn get_globals(&self) -> &HashMap<String, String> {
        &self.globals
    }
    
    pub fn add_global(&mut self, name: String, definition: String) {
        self.globals.insert(name, definition);
    }
}
