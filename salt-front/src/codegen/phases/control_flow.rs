//! Phase 5: Control Flow State
//! Contains loop labels, break/continue targets, and cleanup stack.

use std::collections::HashSet;
use crate::types::Type;

/// A cleanup task representing a resource that must be freed at scope exit.
/// Used by the RAII-Lite system to implement Implicit Scoped Drop.
#[derive(Clone, Debug)]
pub struct CleanupTask {
    /// The MLIR SSA value (the Vec struct/pointer to clean up)
    pub value: String,
    /// The drop function to call (e.g., "std__collections__vec__Vec__drop_u8")
    pub drop_fn: String,
    /// The variable name (for debugging and Z3 tracking)
    pub var_name: String,
    /// The type of the owned resource
    pub ty: Type,
}

/// Phase 5: Control flow state (loop management, cleanup)
#[derive(Default)]
pub struct ControlFlowState {
    /// Stack of loop exit labels for break statements
    pub loop_exit_stack: Vec<String>,
    /// Break target labels stack
    pub break_labels: Vec<String>,
    /// Continue target labels stack
    pub continue_labels: Vec<String>,
    /// Memory region stack for region-based memory
    pub region_stack: Vec<String>,
    /// Stack of cleanup scopes, each containing cleanup tasks
    pub cleanup_stack: Vec<Vec<CleanupTask>>,
    /// Set of variables that have been mutated
    pub mutated_vars: HashSet<String>,
    /// Set of variables that have been consumed (moved)
    pub consumed_vars: HashSet<String>,
    /// Map of consumption locations: var_name -> location description
    pub consumption_locs: std::collections::HashMap<String, String>,
    /// Set of devoured (fully consumed) variables
    pub devoured_vars: HashSet<String>,
    /// Current affine loop nesting depth (for affine.load/store selection)
    pub affine_depth: usize,
    /// Whether we're in an unsafe block
    pub is_unsafe_block: bool,
    /// Whether we're in a @dynamic_check block
    pub is_dynamic_check_block: bool,
    /// Whether yield is disabled
    pub no_yield: bool,
    /// Active pulse budget from @yielding(N)
    pub current_pulse: Option<u32>,
    /// Hot path optimization flag
    pub is_hot_path: bool,
    
    // === Per-Argument Alias Scopes ===
    /// Maps SSA argument name (e.g., "%arg_w") to its unique scope ID
    /// Used to emit fine-grained noalias metadata for pointer arguments
    pub arg_alias_scopes: std::collections::HashMap<String, usize>,
    /// Next available argument scope ID
    pub next_arg_scope_id: usize,
    /// Maps SSA pointer values (including GEP results) to their origin scope ID
    /// Enables scope propagation: GEP result inherits scope from base pointer
    pub ssa_alias_scopes: std::collections::HashMap<String, usize>,
}

impl ControlFlowState {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Check if we're inside an affine loop context
    pub fn is_in_affine_context(&self) -> bool {
        self.affine_depth > 0
    }
    
    /// Enter an affine loop scope
    pub fn enter_affine_scope(&mut self) {
        self.affine_depth += 1;
    }
    
    /// Exit an affine loop scope
    pub fn exit_affine_scope(&mut self) {
        self.affine_depth = self.affine_depth.saturating_sub(1);
    }
    
    // === Per-Argument Alias Scope Methods ===
    
    /// Register a pointer argument with a unique alias scope
    /// Returns the scope ID for this argument
    pub fn register_arg_scope(&mut self, ssa_name: &str) -> usize {
        let scope_id = self.next_arg_scope_id;
        self.next_arg_scope_id += 1;
        self.arg_alias_scopes.insert(ssa_name.to_string(), scope_id);
        scope_id
    }
    
    /// Get the alias scope ID for a pointer argument
    pub fn get_arg_scope(&self, ssa_name: &str) -> Option<usize> {
        self.arg_alias_scopes.get(ssa_name).copied()
    }
    
    /// Clear all argument scopes (called at function entry)
    pub fn clear_arg_scopes(&mut self) {
        self.arg_alias_scopes.clear();
        self.ssa_alias_scopes.clear();
        self.next_arg_scope_id = 0;
    }
    
    /// Get all scope IDs except the given one (for noalias list)
    pub fn get_other_arg_scopes(&self, except_scope: usize) -> Vec<usize> {
        self.arg_alias_scopes
            .values()
            .filter(|&&id| id != except_scope)
            .copied()
            .collect()
    }
    
    /// Propagate scope from base pointer to derived pointer (GEP inheritance)
    /// When %gep_result = getelementptr %base_ptr[...], gep_result inherits base_ptr's scope
    pub fn propagate_scope_provenance(&mut self, from_ssa: &str, to_ssa: &str) {
        // Check arg_alias_scopes first (original function arguments)
        if let Some(&scope_id) = self.arg_alias_scopes.get(from_ssa) {
            self.ssa_alias_scopes.insert(to_ssa.to_string(), scope_id);
        }
        // Then check ssa_alias_scopes (for transitive propagation)
        else if let Some(&scope_id) = self.ssa_alias_scopes.get(from_ssa) {
            self.ssa_alias_scopes.insert(to_ssa.to_string(), scope_id);
        }
    }
    
    /// Get the scope ID for any pointer (argument or derived)
    pub fn get_pointer_scope(&self, ssa_name: &str) -> Option<usize> {
        self.arg_alias_scopes.get(ssa_name).copied()
            .or_else(|| self.ssa_alias_scopes.get(ssa_name).copied())
    }
}
