//! Provenance tracking for pointer identity optimization.
//!
//! The ProvenanceMap tracks local variables that hold base addresses
//! (e.g., from Buffer<T>::alloc), enabling the codegen to emit
//! `llvm.getelementptr` instead of opaque `inttoptr` arithmetic.
//!
//! This preserves pointer identity and enables LLVM's loop vectorizer.
//!
//! ## Buffer-First The Salty Contract
//!
//! When a Buffer<T> is used, the compiler:
//! 1. **Hoists** the inttoptr conversion to the function pre-header
//! 2. **Pins** the resulting !llvm.ptr in the ProvenanceMap
//! 3. **Routes** all buffer[i] accesses through GEP using the pinned pointer

use std::collections::{BTreeMap, HashMap};
use crate::types::Type;

/// Tracks the origin of u64 addresses to enable GEP-based optimization.
#[derive(Default, Clone, Debug)]
pub struct ProvenanceMap {
    /// Maps a local variable name to its underlying element Type.
    /// e.g., "height" -> Type::I32 for Buffer<i32>
    bases: BTreeMap<String, Type>,
}

impl ProvenanceMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a local variable as a tracked memory base.
    pub fn register_base(&mut self, name: String, element_ty: Type) {
        self.bases.insert(name, element_ty);
    }

    /// Looks up the element type if the name is a known provenance base.
    pub fn lookup_base(&self, name: &str) -> Option<&Type> {
        self.bases.get(name)
    }
    
    /// Check if a variable is a known base
    pub fn is_base(&self, name: &str) -> bool {
        self.bases.contains_key(name)
    }
}

// ============================================================================
// PILLAR 1: Origin-Aware Hoisting (The "SSA Shadow" Fix)
// ============================================================================
// Maps SSA result values back to their origin variable names.
// When a Buffer struct is loaded into an SSA register, we track where it came from
// so that subsequent .get()/.set() calls can still use the hoisted pointer.

/// Tracks the origin of SSA values back to source variables.
#[derive(Default, Clone, Debug)]
pub struct OriginMap {
    /// Maps SSA value name (e.g., "%load_scratch_43") to source variable (e.g., "scratch")
    origins: HashMap<String, String>,
}

impl OriginMap {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Register that an SSA value originated from a variable.
    /// Called when emitting loads from local variables.
    pub fn register(&mut self, ssa_value: String, source_var: String) {
        self.origins.insert(ssa_value, source_var);
    }
    
    /// Look up the origin variable for an SSA value.
    pub fn get_origin(&self, ssa_value: &str) -> Option<&String> {
        self.origins.get(ssa_value)
    }
    
    /// Clear origins (for scope transitions).
    pub fn clear(&mut self) {
        self.origins.clear();
    }
}

// ============================================================================
// PILLAR 2: Global Value Pinning (Local Value Numbering for Globals)
// ============================================================================
// Caches the "current value" of a global within a FUNCTION scope.
// The first load is performed; subsequent reads in the SAME function reuse
// the cached SSA value. Different functions get their own cache entries.
//
// Uses composite key (FuncName, Symbol) to prevent 
// cross-function SSA value reuse. Each function resolves constants independently.

/// Local Value Numbering cache for global variables.
/// Uses composite key (func_name, symbol) for function-scoped caching.
/// 
/// # SSA Dominance Safety
/// The `snapshot_stack` enables correct behavior across control flow divergence.
/// Before entering an if/else branch, call `push_snapshot()` to save the cache.
/// After the branch completes, call `pop_snapshot()` to restore it. This ensures
/// SSA values loaded in one branch never leak into sibling branches, which would
/// violate SSA dominance and crash the MLIR verifier.
#[derive(Default, Clone, Debug)]
pub struct GlobalLVN {
    /// Maps (function_name, global_symbol) -> cached SSA value
    /// e.g., ("write_i32", "BUFFER_SIZE") -> "%global_val_42"
    cache: HashMap<(String, String), String>,
    /// Current function being compiled - set via set_current_function()
    current_function: Option<String>,
    /// Snapshot stack for control flow scoping (if/else, match, etc.)
    /// Each entry is a saved copy of the cache at a branch point.
    snapshot_stack: Vec<HashMap<(String, String), String>>,
}

impl GlobalLVN {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Set the current function context for cache lookups.
    /// Returns the previous function name (if any) to allow restoration.
    pub fn set_current_function(&mut self, func_name: String) -> Option<String> {
        let prev = self.current_function.clone();
        self.current_function = Some(func_name);
        prev
    }
    
    /// Clear current function (optional, for cleanup).
    pub fn clear_current_function(&mut self) {
        self.current_function = None;
    }

    /// Get the current function name (for debugging).
    pub fn get_current_function(&self) -> Option<String> {
        self.current_function.clone()
    }
    
    /// Clear all cached values for the current function.
    /// Must be called at the start of emit_fn to prevent stale SSA reuse
    /// if the function is re-compiled (e.g. during re-hydration phases).
    pub fn clear_current_func_cache(&mut self) {
        if let Some(ref func) = self.current_function {
            // Retain only keys that do NOT match the current function
            self.cache.retain(|(f, _), _| f != func);
        }
    }
    
    /// Save a snapshot of the current cache state before entering a branch.
    /// Must be paired with `pop_snapshot()` after the branch completes.
    ///
    /// This is the core mechanism for SSA dominance correctness:
    /// values loaded in a then-branch must not be visible in the else-branch.
    pub fn push_snapshot(&mut self) {
        self.snapshot_stack.push(self.cache.clone());
    }
    
    /// Restore the cache to the state saved by the most recent `push_snapshot()`.
    /// Any SSA values cached during the branch are discarded, preventing
    /// cross-branch value leakage that would violate SSA dominance.
    pub fn pop_snapshot(&mut self) {
        if let Some(saved) = self.snapshot_stack.pop() {
            self.cache = saved;
        }
    }
    
    /// Cache a loaded global value for the current function.
    pub fn cache_value(&mut self, symbol: String, ssa_value: String) {
        if let Some(ref func) = self.current_function {
            self.cache.insert((func.clone(), symbol), ssa_value);
        }
    }
    
    /// Get cached value for a global in the current function.
    pub fn get_cached(&self, symbol: &str) -> Option<&String> {
        self.current_function.as_ref().and_then(|func| {
            self.cache.get(&(func.clone(), symbol.to_string()))
        })
    }
    
    /// Update the cached value after a store to a global.
    pub fn update(&mut self, symbol: &str, new_value: String) {
        if let Some(ref func) = self.current_function {
            let key = (func.clone(), symbol.to_string());
            if self.cache.contains_key(&key) {
                self.cache.insert(key, new_value);
            }
        }
    }
    
    /// Invalidate cache entry for current function.
    pub fn invalidate(&mut self, symbol: &str) {
        if let Some(ref func) = self.current_function {
            self.cache.remove(&(func.clone(), symbol.to_string()));
        }
    }
    
    /// Clear all cached values (for full reset, rarely needed).
    pub fn clear(&mut self) {
        self.cache.clear();
    }
}


