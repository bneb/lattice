//! Pointer State Tracker — Flow-Sensitive 3-State Machine
//!
//! Tracks the compile-time state of every pointer variable:
//! - **Valid**: safe to dereference (from Box::new, Arena::alloc, or narrowed Optional)
//! - **Empty**: sentinel address 0, NOT safe to dereference (from Ptr::empty)
//! - **Optional**: unknown state, NOT safe to dereference (merge points, function args)
//!
//! At runtime, Ptr<T> remains a u64. These states exist ONLY at compile time.

use std::collections::HashMap;

/// The 5 states of a pointer in the Salt Memory Model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerState {
    /// Declared but not yet holding a valid allocation
    Uninitialized,
    /// Safe to dereference. Source: Box::new(), Arena::alloc(), narrowed Optional.
    Valid,
    /// Sentinel (address 0). Dereference is a compile error.
    Empty,
    /// Unknown — could be Valid or Empty. Dereference is a compile error.
    /// Must be narrowed via `if p.addr != 0` to become Valid.
    Optional,
    /// Has been passed to free(). Dereference is a compile error.
    Freed,
}

impl std::fmt::Display for PointerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PointerState::Uninitialized => write!(f, "Uninitialized"),
            PointerState::Valid => write!(f, "Valid"),
            PointerState::Empty => write!(f, "Empty"),
            PointerState::Optional => write!(f, "Optional"),
            PointerState::Freed => write!(f, "Freed"),
        }
    }
}

/// Flow-sensitive pointer state tracker.
///
/// Tracks PointerState for each variable name, with scope support
/// for branching (push/pop) and merging.
#[derive(Default)]
pub struct PointerStateTracker {
    /// Current scope's variable states
    states: HashMap<String, PointerState>,
    /// Stack of saved scopes for branching
    scope_stack: Vec<HashMap<String, PointerState>>,
}

impl PointerStateTracker {
    /// Create a new, empty tracker.
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            scope_stack: Vec::new(),
        }
    }

    /// Mark a variable as Uninitialized.
    pub fn mark_uninitialized(&mut self, name: &str) {
        self.states.insert(name.to_string(), PointerState::Uninitialized);
    }

    /// Mark a variable as Valid (e.g., after Box::new or Arena::alloc).
    pub fn mark_valid(&mut self, name: &str) {
        self.states.insert(name.to_string(), PointerState::Valid);
    }

    /// Mark a variable as Empty (e.g., after Ptr::empty or from_addr(0)).
    pub fn mark_empty(&mut self, name: &str) {
        self.states.insert(name.to_string(), PointerState::Empty);
    }

    /// Mark a variable as Freed (after free()).
    pub fn mark_freed(&mut self, name: &str) {
        self.states.insert(name.to_string(), PointerState::Freed);
    }

    /// Mark a variable as Optional (e.g., function arg of type Ptr<T>, or merge point).
    pub fn mark_optional(&mut self, name: &str) {
        self.states.insert(name.to_string(), PointerState::Optional);
    }

    /// Get the current state of a variable. Returns None if not tracked.
    pub fn get_state(&self, name: &str) -> Option<PointerState> {
        self.states.get(name).copied()
    }

    /// Check if a dereference is allowed. Returns Ok(()) if Valid, Err with
    /// a diagnostic message if Empty, Optional, Freed, or Uninitialized.
    pub fn check_deref(&self, name: &str) -> Result<(), String> {

        match self.states.get(name) {
            Some(PointerState::Valid) => Ok(()),
            Some(PointerState::Freed) => Err(format!(
                "Cannot dereference 'Freed' pointer '{}'. \
                 It has already been freed.",
                name
            )),
            Some(PointerState::Uninitialized) => Err(format!(
                "Cannot dereference 'Uninitialized' pointer '{}'. \
                 It has not been assigned a valid allocation.",
                name
            )),
            Some(PointerState::Empty) => Err(format!(
                "Cannot dereference 'Empty' pointer '{}'. \
                 Ptr::empty() is a sentinel — it cannot be read or written.",
                name
            )),
            Some(PointerState::Optional) => Err(format!(
                "Cannot dereference 'Optional' pointer '{}'. \
                 Check with `if {}.addr() != 0` before dereferencing.",
                name, name
            )),
            None => {
                // Not tracked — assume safe (foreign pointer, non-Ptr type, etc.)
                Ok(())
            }
        }
    }

    /// Push a new scope (snapshot current state for branching).
    pub fn push_scope(&mut self) {
        self.scope_stack.push(self.states.clone());
    }

    /// Pop scope, returning the saved state. The caller can use this
    /// with `merge` to combine branch outcomes.
    pub fn pop_scope(&mut self) -> Option<HashMap<String, PointerState>> {
        self.scope_stack.pop()
    }

    /// Merge two state maps. For each variable present in both:
    /// - Valid + Valid = Valid
    /// - Empty + Empty = Empty
    /// - Any other combination = Optional
    pub fn merge(&mut self, other: &HashMap<String, PointerState>) {
        // Collect all keys from both maps
        let all_keys: Vec<String> = self.states.keys()
            .chain(other.keys())
            .cloned()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        for key in all_keys {
            let state_a = self.states.get(&key).copied();
            let state_b = other.get(&key).copied();

            let merged = match (state_a, state_b) {
                (Some(a), Some(b)) if a == b => a,
                (Some(PointerState::Freed), _) | (_, Some(PointerState::Freed)) => PointerState::Freed,
                (Some(PointerState::Uninitialized), _) | (_, Some(PointerState::Uninitialized)) => PointerState::Uninitialized,
                (Some(_), Some(_)) => PointerState::Optional,
                (Some(a), None) => a,
                (None, Some(b)) => b,
                (None, None) => unreachable!(),
            };

            self.states.insert(key, merged);
        }
    }

    /// Clear all tracked state (for new function scope).
    pub fn clear(&mut self) {
        self.states.clear();
        self.scope_stack.clear();
    }

    /// Restore specific state map (e.g., from pop_scope).
    pub fn restore_state(&mut self, state: HashMap<String, PointerState>) {
        self.states = state;
    }

    /// Returns number of tracked variables.
    pub fn tracked_count(&self) -> usize {
        self.states.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // TDD Phase 1: Basic State Assignment
    // ========================================================================

    #[test]
    fn test_empty_state_from_ptr_empty() {
        let mut tracker = PointerStateTracker::new();
        tracker.mark_empty("p");
        assert_eq!(tracker.get_state("p"), Some(PointerState::Empty));
    }

    #[test]
    fn test_valid_state_from_box_new() {
        let mut tracker = PointerStateTracker::new();
        tracker.mark_valid("node");
        assert_eq!(tracker.get_state("node"), Some(PointerState::Valid));
    }

    #[test]
    fn test_optional_state_for_fn_arg() {
        let mut tracker = PointerStateTracker::new();
        tracker.mark_optional("arg_ptr");
        assert_eq!(tracker.get_state("arg_ptr"), Some(PointerState::Optional));
    }

    #[test]
    fn test_untracked_returns_none() {
        let tracker = PointerStateTracker::new();
        assert_eq!(tracker.get_state("unknown"), None);
    }

    // ========================================================================
    // TDD Phase 2: Dereference Checks
    // ========================================================================

    #[test]
    fn test_deref_valid_ok() {
        let mut tracker = PointerStateTracker::new();
        tracker.mark_valid("node");
        assert!(tracker.check_deref("node").is_ok());
    }

    #[test]
    fn test_deref_empty_error() {
        let mut tracker = PointerStateTracker::new();
        tracker.mark_empty("sentinel");
        let err = tracker.check_deref("sentinel").unwrap_err();
        assert!(err.contains("Empty"), "Error should mention 'Empty': {}", err);
        assert!(err.contains("sentinel"), "Error should mention var name: {}", err);
    }

    #[test]
    fn test_deref_optional_error() {
        let mut tracker = PointerStateTracker::new();
        tracker.mark_optional("maybe");
        let err = tracker.check_deref("maybe").unwrap_err();
        assert!(err.contains("Optional"), "Error should mention 'Optional': {}", err);
        assert!(err.contains("maybe"), "Error should mention var name: {}", err);
    }

    #[test]
    fn test_deref_freed_error() {
        let mut tracker = PointerStateTracker::new();
        tracker.mark_freed("dead");
        let err = tracker.check_deref("dead").unwrap_err();
        assert!(err.contains("Freed"), "Error should mention 'Freed': {}", err);
        assert!(err.contains("dead"), "Error should mention var name: {}", err);
    }

    #[test]
    fn test_deref_untracked_ok() {
        let tracker = PointerStateTracker::new();
        // Untracked variables (non-Ptr, foreign) pass through
        assert!(tracker.check_deref("foreign").is_ok());
    }

    // ========================================================================
    // TDD Phase 3: Narrowing (Optional → Valid via null check)
    // ========================================================================

    #[test]
    fn test_narrowing_optional_to_valid() {
        let mut tracker = PointerStateTracker::new();
        tracker.mark_optional("p");

        // Simulate: `if p.addr != 0 { ... }`
        // Inside the true branch, p is Valid
        tracker.push_scope();
        tracker.mark_valid("p"); // narrowed

        assert_eq!(tracker.get_state("p"), Some(PointerState::Valid));
        assert!(tracker.check_deref("p").is_ok());
    }

    #[test]
    fn test_narrowing_does_not_leak_to_outer_scope() {
        let mut tracker = PointerStateTracker::new();
        tracker.mark_optional("p");

        // Enter branch: narrow to Valid
        tracker.push_scope();
        tracker.mark_valid("p");
        assert_eq!(tracker.get_state("p"), Some(PointerState::Valid));

        // Exit branch: restore outer scope
        let saved = tracker.pop_scope().expect("scope was pushed earlier in test");
        tracker.states = saved; // restore

        // Outside the branch, p is still Optional
        assert_eq!(tracker.get_state("p"), Some(PointerState::Optional));
    }

    // ========================================================================
    // TDD Phase 4: Merge Points
    // ========================================================================

    #[test]
    fn test_merge_valid_and_empty_creates_optional() {
        let mut tracker = PointerStateTracker::new();

        // Branch A: p = Box::new(...)  → Valid
        let mut branch_a = HashMap::new();
        branch_a.insert("p".to_string(), PointerState::Valid);

        // Branch B: p = Ptr::empty()  → Empty
        tracker.states.insert("p".to_string(), PointerState::Empty);

        // Merge
        tracker.merge(&branch_a);
        // Valid + Empty = Optional
        assert_eq!(tracker.get_state("p"), Some(PointerState::Optional));
    }

    #[test]
    fn test_merge_valid_and_valid_stays_valid() {
        let mut tracker = PointerStateTracker::new();
        tracker.mark_valid("p");

        let mut other = HashMap::new();
        other.insert("p".to_string(), PointerState::Valid);

        tracker.merge(&other);
        assert_eq!(tracker.get_state("p"), Some(PointerState::Valid));
    }

    #[test]
    fn test_merge_empty_and_empty_stays_empty() {
        let mut tracker = PointerStateTracker::new();
        tracker.mark_empty("p");

        let mut other = HashMap::new();
        other.insert("p".to_string(), PointerState::Empty);

        tracker.merge(&other);
        assert_eq!(tracker.get_state("p"), Some(PointerState::Empty));
    }

    // ========================================================================
    // TDD Phase 5: Scope Isolation
    // ========================================================================

    #[test]
    fn test_scope_push_pop_restores_state() {
        let mut tracker = PointerStateTracker::new();
        tracker.mark_optional("p");
        tracker.mark_valid("q");

        tracker.push_scope();
        tracker.mark_valid("p");
        tracker.mark_empty("q");

        // Inside scope: modified
        assert_eq!(tracker.get_state("p"), Some(PointerState::Valid));
        assert_eq!(tracker.get_state("q"), Some(PointerState::Empty));

        // Pop: restore
        let saved = tracker.pop_scope().expect("scope was pushed earlier in test");
        tracker.states = saved;

        assert_eq!(tracker.get_state("p"), Some(PointerState::Optional));
        assert_eq!(tracker.get_state("q"), Some(PointerState::Valid));
    }

    // ========================================================================
    // TDD Phase 6: Clear and Count
    // ========================================================================

    #[test]
    fn test_clear_resets_all() {
        let mut tracker = PointerStateTracker::new();
        tracker.mark_valid("a");
        tracker.mark_empty("b");
        tracker.mark_optional("c");
        assert_eq!(tracker.tracked_count(), 3);

        tracker.clear();
        assert_eq!(tracker.tracked_count(), 0);
        assert_eq!(tracker.get_state("a"), None);
    }

    #[test]
    fn test_display_trait() {
        assert_eq!(format!("{}", PointerState::Valid), "Valid");
        assert_eq!(format!("{}", PointerState::Empty), "Empty");
        assert_eq!(format!("{}", PointerState::Optional), "Optional");
    }
}
