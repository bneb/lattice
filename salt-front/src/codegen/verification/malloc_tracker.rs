//! MallocTracker — Standalone DAG-based malloc leak detector
//!
//! Tracks allocations as a Directed Acyclic Graph of value dependencies.
//! This enables proving that returned structs (like HashMap) safely carry
//! their allocated pointers out of scope.
//!
//! No Z3 dependency — pure Rust data structures.

use std::collections::{HashMap, HashSet};

/// Tracks malloc'd allocations and their flow through struct construction,
/// casts, and returns using a dependency graph.
#[derive(Debug, Clone)]
pub struct MallocTracker {
    /// Active (un-freed, un-escaped) allocations: alloc_id → source variable name
    active_allocs: HashMap<String, String>,

    /// Dependency graph: composite value → list of component values it contains.
    /// e.g., "map" → ["malloc:ctrl_addr", "malloc:entries_addr"]
    dependencies: HashMap<String, Vec<String>>,
}

impl Default for MallocTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl MallocTracker {
    pub fn new() -> Self {
        Self {
            active_allocs: HashMap::new(),
            dependencies: HashMap::new(),
        }
    }

    /// Register a new malloc allocation.
    /// `alloc_id`: unique identifier like "malloc:buf"
    /// `source_var`: the variable name that holds the raw pointer
    pub fn track(&mut self, alloc_id: String, source_var: String) {
        self.active_allocs.insert(alloc_id, source_var);
    }

    /// Register an explicit free. Removes the allocation from tracking.
    pub fn free(&mut self, alloc_id: &str) {
        self.active_allocs.remove(alloc_id);
    }

    /// Record that `composite` transitively contains `component`.
    /// Used for: struct construction (struct → field), cast aliasing (q → p).
    pub fn link_dependency(&mut self, composite: impl Into<String>, component: impl Into<String>) {
        self.dependencies
            .entry(composite.into())
            .or_default()
            .push(component.into());
    }

    /// Check if a value has any dependencies in the DAG.
    pub fn has_dependencies(&self, val_name: &str) -> bool {
        self.dependencies.contains_key(val_name)
    }

    /// Migrate all edges from the `__pending_struct` sentinel to `target_name`.
    /// Called when a let-binding assigns a struct construction to a variable.
    /// Struct construction links fields to `__pending_struct`; this renames them.
    pub fn drain_pending_to(&mut self, target_name: &str) {
        if let Some(deps) = self.dependencies.remove("__pending_struct") {
            for dep in deps {
                self.link_dependency(target_name, dep);
            }
        }
    }

    /// Recursively mark a value and all its transitive dependencies as escaped.
    /// Called on: return, field-assign to &mut self.
    pub fn mark_escaped(&mut self, val_name: &str) {
        let mut visited = HashSet::new();
        self.escape_recurse(val_name, &mut visited);
    }

    fn escape_recurse(&mut self, val_name: &str, visited: &mut HashSet<String>) {
        if !visited.insert(val_name.to_string()) {
            return; // Cycle protection (shouldn't happen in SSA, but safety first)
        }

        // Base case: if this is a tracked allocation, it escapes
        self.active_allocs.remove(val_name);

        // Recursive step: if this is a composite, recurse into children
        if let Some(children) = self.dependencies.get(val_name).cloned() {
            for child in children {
                self.escape_recurse(&child, visited);
            }
        }
    }

    /// Check if a variable name is tracked (has an active allocation).
    pub fn is_tracked(&self, var_name: &str) -> bool {
        self.active_allocs.values().any(|v| v == var_name)
    }

    /// Look up the alloc_id for a given source variable name.
    pub fn alloc_id_for_var(&self, var_name: &str) -> Option<String> {
        self.active_allocs.iter()
            .find(|(_, v)| v.as_str() == var_name)
            .map(|(k, _)| k.clone())
    }

    /// Get the alloc_id directly if it exists.
    pub fn get_alloc(&self, alloc_id: &str) -> Option<&String> {
        self.active_allocs.get(alloc_id)
    }

    /// Check if an alloc_id is actively tracked.
    pub fn contains_alloc(&self, alloc_id: &str) -> bool {
        self.active_allocs.contains_key(alloc_id)
    }

    /// Verify no allocations remain un-freed and un-escaped.
    pub fn verify(&self) -> Result<(), String> {
        if self.active_allocs.is_empty() {
            return Ok(());
        }

        let (var, info) = self.active_allocs.iter().next().expect("verify called with non-empty active_allocs");
        Err(format!(
            "Memory Leak Detected: Allocation '{}' ({}) was neither freed nor returned.",
            var, info
        ))
    }

    /// Reset for a new function scope.
    pub fn clear(&mut self) {
        self.active_allocs.clear();
        self.dependencies.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ====================================================================
    // Basic allocation tracking
    // ====================================================================

    #[test]
    fn test_empty_tracker_passes() {
        let tracker = MallocTracker::new();
        assert!(tracker.verify().is_ok());
    }

    #[test]
    fn test_malloc_without_free_is_leak() {
        let mut tracker = MallocTracker::new();
        tracker.track("malloc:buf".into(), "buf".into());
        let result = tracker.verify();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Memory Leak Detected"));
    }

    #[test]
    fn test_malloc_with_free_no_leak() {
        let mut tracker = MallocTracker::new();
        tracker.track("malloc:buf".into(), "buf".into());
        tracker.free("malloc:buf");
        assert!(tracker.verify().is_ok());
    }

    #[test]
    fn test_multiple_allocs_all_freed() {
        let mut tracker = MallocTracker::new();
        tracker.track("malloc:a".into(), "a".into());
        tracker.track("malloc:b".into(), "b".into());
        tracker.track("malloc:c".into(), "c".into());
        tracker.free("malloc:a");
        tracker.free("malloc:b");
        tracker.free("malloc:c");
        assert!(tracker.verify().is_ok());
    }

    #[test]
    fn test_partial_free_is_leak() {
        let mut tracker = MallocTracker::new();
        tracker.track("malloc:a".into(), "a".into());
        tracker.track("malloc:b".into(), "b".into());
        tracker.free("malloc:a");
        // malloc:b not freed
        assert!(tracker.verify().is_err());
    }

    #[test]
    fn test_free_untracked_is_silent() {
        let mut tracker = MallocTracker::new();
        tracker.free("malloc:unknown"); // No panic
        assert!(tracker.verify().is_ok());
    }

    #[test]
    fn test_clear_resets_tracker() {
        let mut tracker = MallocTracker::new();
        tracker.track("malloc:a".into(), "a".into());
        tracker.link_dependency("s", "malloc:a");
        tracker.clear();
        assert!(tracker.verify().is_ok());
        assert!(tracker.active_allocs.is_empty());
        assert!(tracker.dependencies.is_empty());
    }

    // ====================================================================
    // Direct pointer return (escape)
    // ====================================================================

    /// `fn alloc() -> u64 { let p = malloc(24); return p; }`
    #[test]
    fn test_malloc_returned_directly_is_escaped() {
        let mut tracker = MallocTracker::new();
        tracker.track("malloc:p".into(), "p".into());
        tracker.mark_escaped("malloc:p");
        assert!(tracker.verify().is_ok());
    }

    // ====================================================================
    // Struct field return (transitive escape)
    // ====================================================================

    /// `let p = malloc(24); return S { field: p };`
    #[test]
    fn test_malloc_in_struct_field_returned_is_escaped() {
        let mut tracker = MallocTracker::new();
        tracker.track("malloc:p".into(), "p".into());
        tracker.link_dependency("result_struct", "malloc:p");
        tracker.mark_escaped("result_struct");
        assert!(tracker.verify().is_ok(),
            "Struct return should transitively escape contained pointer");
    }

    /// Struct with malloc field NOT returned = leak
    #[test]
    fn test_malloc_in_struct_not_returned_is_leak() {
        let mut tracker = MallocTracker::new();
        tracker.track("malloc:p".into(), "p".into());
        tracker.link_dependency("orphan_struct", "malloc:p");
        // Neither returned nor freed
        assert!(tracker.verify().is_err());
    }

    /// Two pointers in struct, only one linked to returned struct
    #[test]
    fn test_partial_escape_detects_remaining_leak() {
        let mut tracker = MallocTracker::new();
        tracker.track("malloc:a".into(), "a".into());
        tracker.track("malloc:b".into(), "b".into());
        tracker.link_dependency("result", "malloc:a");
        tracker.mark_escaped("result");
        // malloc:b is orphaned
        assert!(tracker.verify().is_err());
    }

    // ====================================================================
    // Cast aliasing propagation
    // ====================================================================

    /// `let p = malloc(24); let q = p as Ptr<T>; return S { field: q };`
    #[test]
    fn test_cast_alias_propagates_through_dependency() {
        let mut tracker = MallocTracker::new();
        tracker.track("malloc:p".into(), "p".into());
        // Cast: q aliases p
        tracker.link_dependency("q", "malloc:p");
        // Struct: S { field: q }
        tracker.link_dependency("s", "q");
        // Return S
        tracker.mark_escaped("s");
        assert!(tracker.verify().is_ok(),
            "Cast alias through struct should escape");
    }

    // ====================================================================
    // Field-assign escape (grow pattern)
    // ====================================================================

    /// `self.ctrl = new_ctrl;` where new_ctrl is malloc'd
    #[test]
    fn test_field_assign_escape_via_mut_self() {
        let mut tracker = MallocTracker::new();
        // Old alloc freed
        tracker.track("malloc:old".into(), "old".into());
        tracker.free("malloc:old");
        // New alloc escapes via field assign
        tracker.track("malloc:new".into(), "new_ptr".into());
        tracker.mark_escaped("malloc:new");
        assert!(tracker.verify().is_ok());
    }

    /// The exact HashMap grow() pattern
    #[test]
    fn test_hashmap_grow_pattern() {
        let mut tracker = MallocTracker::new();
        tracker.track("malloc:new_ctrl_addr".into(), "new_ctrl_addr".into());
        tracker.track("malloc:new_entries_addr".into(), "new_entries_addr".into());
        // Cast aliases
        tracker.link_dependency("new_ctrl", "malloc:new_ctrl_addr");
        tracker.link_dependency("new_entries", "malloc:new_entries_addr");
        // Field assigns → escape
        tracker.mark_escaped("malloc:new_ctrl_addr");
        tracker.mark_escaped("malloc:new_entries_addr");
        assert!(tracker.verify().is_ok(),
            "HashMap grow() pattern should pass");
    }

    // ====================================================================
    // Deep transitive nesting
    // ====================================================================

    /// outer → inner → malloc:deep_ptr
    #[test]
    fn test_deep_transitive_escape() {
        let mut tracker = MallocTracker::new();
        tracker.track("malloc:deep".into(), "deep".into());
        tracker.link_dependency("inner", "malloc:deep");
        tracker.link_dependency("outer", "inner");
        tracker.mark_escaped("outer");
        assert!(tracker.verify().is_ok(),
            "3-level transitive escape should work");
    }

    // ====================================================================
    // HashMap with_capacity pattern (inline struct return)
    // ====================================================================

    /// Full HashMap::with_capacity flow:
    /// malloc ctrl → cast → malloc entries → cast → return HashMap { ctrl, entries, ... }
    #[test]
    fn test_hashmap_with_capacity_full_flow() {
        let mut tracker = MallocTracker::new();
        // Two mallocs
        tracker.track("malloc:ctrl_addr".into(), "ctrl_addr".into());
        tracker.track("malloc:entries_addr".into(), "entries_addr".into());
        // Two casts
        tracker.link_dependency("ctrl", "malloc:ctrl_addr");
        tracker.link_dependency("entries", "malloc:entries_addr");
        // Struct construction links fields
        tracker.link_dependency("map", "ctrl");
        tracker.link_dependency("map", "entries");
        // Return struct → transitive escape
        tracker.mark_escaped("map");
        assert!(tracker.verify().is_ok(),
            "Full HashMap::with_capacity pattern should pass");
    }

    // ====================================================================
    // Lookup helpers
    // ====================================================================

    #[test]
    fn test_is_tracked() {
        let mut tracker = MallocTracker::new();
        tracker.track("malloc:buf".into(), "buf".into());
        assert!(tracker.is_tracked("buf"));
        assert!(!tracker.is_tracked("unknown"));
    }

    #[test]
    fn test_alloc_id_for_var() {
        let mut tracker = MallocTracker::new();
        tracker.track("malloc:buf".into(), "buf".into());
        assert_eq!(tracker.alloc_id_for_var("buf"), Some("malloc:buf".into()));
        assert_eq!(tracker.alloc_id_for_var("nope"), None);
    }
}
