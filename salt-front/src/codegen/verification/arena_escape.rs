//! Arena Escape Analysis — Depth-Based Taint Tracker
//!
//! Implements the "Scope Ladder" system for zero-annotation lifetime safety.
//!
//! ## The Depth Model
//! Every variable is assigned an integer **depth** based on its scope:
//! - **Depth 0**: Global / static — lives forever
//! - **Depth 1**: Function arguments — outlive the function body
//! - **Depth 2+**: Local variables — die when their scope ends
//!
//! ## The Three Laws
//! 1. **Return Rule**: `return x` is valid iff `depth(x) <= 1`
//! 2. **Assignment Rule**: `a = b` is valid iff `depth(b) <= depth(a)`
//! 3. **Transitivity Rule**: `s.field` inherits `depth(s)`
//!
//! Arena pointers inherit the depth of the arena they were allocated from.
//! This catches dangling pointers at compile time with zero syntax overhead.

use std::collections::HashMap;

/// Depth-based taint tracker for arena escape analysis.
///
/// Tracks the "taint depth" of every pointer variable. A pointer's depth
/// is inherited from the arena it was allocated from.
pub struct ArenaEscapeTracker {
    /// Variable name → depth (taint level)
    taint: HashMap<String, usize>,
    /// Current scope depth (2 = function body, 3+ = nested blocks)
    current_depth: usize,
    /// Whether escape analysis is active for this function
    active: bool,
}

impl ArenaEscapeTracker {
    pub fn new() -> Self {
        Self {
            taint: HashMap::new(),
            current_depth: 2, // function body
            active: false,
        }
    }

    /// Clear all state for a new function scope.
    pub fn clear(&mut self) {
        self.taint.clear();
        self.current_depth = 2;
        self.active = false;
    }

    /// Activate tracking (called when we see an Arena in the function).
    pub fn activate(&mut self) {
        self.active = true;
    }

    /// Check if tracking is active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Register an arena variable at the current scope depth.
    /// Called when `let arena = Arena::new(...)` is seen.
    pub fn register_arena(&mut self, name: &str) {
        self.taint.insert(name.to_string(), self.current_depth);
        self.activate();
    }

    /// Register an arena function argument at depth 1.
    /// Called when a function parameter has Arena type.
    pub fn register_arena_arg(&mut self, name: &str) {
        self.taint.insert(name.to_string(), 1);
        self.activate();
    }

    /// Register a pointer allocated from an arena.
    /// The pointer inherits the arena's depth.
    pub fn register_alloc(&mut self, ptr_name: &str, arena_name: &str) {
        let arena_depth = self.taint.get(arena_name).copied().unwrap_or(self.current_depth);
        self.taint.insert(ptr_name.to_string(), arena_depth);
    }

    /// Register an ArenaAllocator wrapper that wraps a specific arena.
    /// The allocator inherits the arena's depth.
    /// Called when: `let alloc = ArenaAllocator { arena: my_arena }`
    pub fn register_arena_allocator(&mut self, alloc_name: &str, arena_name: &str) {
        if let Some(arena_depth) = self.taint.get(arena_name).copied() {
            self.taint.insert(alloc_name.to_string(), arena_depth);
        }
        // If arena is not tracked, allocator is not tracked either.
        // This is the conservative path — HeapAllocator stays invisible.
    }

    /// Register a Vec (or any container) that was constructed with a tracked allocator.
    /// The Vec inherits the allocator's depth.
    /// Called when: `let v = Vec::new(alloc, cap)`
    pub fn register_vec_from_allocator(&mut self, vec_name: &str, allocator_name: &str) {
        if let Some(alloc_depth) = self.taint.get(allocator_name).copied() {
            self.taint.insert(vec_name.to_string(), alloc_depth);
        }
        // If allocator is not tracked (e.g. HeapAllocator), Vec is not tracked.
        // Untracked variables pass all escape checks (conservative allow).
    }

    /// Register a variable with explicit depth (e.g., function arguments = depth 1).
    pub fn register_arg(&mut self, name: &str) {
        self.taint.insert(name.to_string(), 1);
    }

    /// Get the depth of a variable. Returns None if not tracked.
    pub fn get_depth(&self, name: &str) -> Option<usize> {
        self.taint.get(name).copied()
    }

    /// Push a new scope (if/loop/block). Increments depth.
    pub fn push_scope(&mut self) {
        self.current_depth += 1;
    }

    /// Pop a scope. Decrements depth.
    pub fn pop_scope(&mut self) {
        if self.current_depth > 2 {
            self.current_depth -= 1;
        }
    }

    /// Get current scope depth.
    pub fn current_depth(&self) -> usize {
        self.current_depth
    }

    // ====================================================================
    // Arena safety rules
    // ====================================================================

    /// **Law I: The Return Rule**
    /// `return x` is valid iff depth(x) <= 1.
    /// A local-depth pointer cannot escape the function.
    pub fn check_return_escape(&self, var_name: &str) -> Result<(), String> {
        if !self.active {
            return Ok(());
        }
        if let Some(depth) = self.taint.get(var_name) {
            if *depth > 1 {
                return Err(format!(
                    "Arena escape violation: pointer '{}' (depth {}) cannot be returned. \
                     It was allocated from a local arena that dies when this function returns.",
                    var_name, depth
                ));
            }
        }
        Ok(())
    }

    /// **Law II: The Assignment Rule**
    /// `a = b` is valid iff depth(b) <= depth(a).
    /// Cannot put a short-lived value into a long-lived container.
    pub fn check_store_escape(&self, rhs_var: &str, lhs_var: &str) -> Result<(), String> {
        if !self.active {
            return Ok(());
        }
        let rhs_depth = match self.taint.get(rhs_var) {
            Some(d) => *d,
            None => return Ok(()), // Not tracked — safe
        };
        let lhs_depth = match self.taint.get(lhs_var) {
            Some(d) => *d,
            None => return Ok(()), // Not tracked — assume safe
        };
        if rhs_depth > lhs_depth {
            return Err(format!(
                "Arena escape violation: pointer '{}' (depth {}) stored into '{}' (depth {}). \
                 The source has a shorter lifetime than the destination.",
                rhs_var, rhs_depth, lhs_var, lhs_depth
            ));
        }
        Ok(())
    }
}

impl Default for ArenaEscapeTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ====================================================================
    // Law I: Return Rule
    // ====================================================================

    #[test]
    fn test_local_arena_ptr_cannot_return() {
        let mut tracker = ArenaEscapeTracker::new();
        tracker.register_arena("arena");        // depth 2 (local)
        tracker.register_alloc("p", "arena");   // p inherits depth 2
        assert!(tracker.check_return_escape("p").is_err());
    }

    #[test]
    fn test_arg_arena_ptr_can_return() {
        let mut tracker = ArenaEscapeTracker::new();
        tracker.register_arena_arg("arena");    // depth 1 (argument)
        tracker.register_alloc("p", "arena");   // p inherits depth 1
        assert!(tracker.check_return_escape("p").is_ok());
    }

    #[test]
    fn test_untracked_ptr_can_return() {
        let mut tracker = ArenaEscapeTracker::new();
        tracker.activate();
        assert!(tracker.check_return_escape("unknown_ptr").is_ok());
    }

    #[test]
    fn test_inactive_tracker_allows_everything() {
        let tracker = ArenaEscapeTracker::new();
        // Not activated — no arena seen
        assert!(tracker.check_return_escape("anything").is_ok());
    }

    // ====================================================================
    // Law II: Assignment Rule
    // ====================================================================

    #[test]
    fn test_local_ptr_into_arg_struct_fails() {
        let mut tracker = ArenaEscapeTracker::new();
        tracker.register_arg("ctx");            // depth 1 (argument)
        tracker.register_arena("local_arena");  // depth 2 (local)
        tracker.register_alloc("data", "local_arena"); // data: depth 2
        assert!(tracker.check_store_escape("data", "ctx").is_err());
    }

    #[test]
    fn test_arg_ptr_into_arg_struct_ok() {
        let mut tracker = ArenaEscapeTracker::new();
        tracker.register_arg("ctx");                // depth 1
        tracker.register_arena_arg("outer_arena");  // depth 1
        tracker.register_alloc("data", "outer_arena"); // depth 1
        assert!(tracker.check_store_escape("data", "ctx").is_ok());
    }

    #[test]
    fn test_same_depth_store_ok() {
        let mut tracker = ArenaEscapeTracker::new();
        tracker.register_arena("arena1");       // depth 2
        tracker.register_arena("arena2");       // depth 2
        tracker.register_alloc("p1", "arena1"); // depth 2
        tracker.register_alloc("p2", "arena2"); // depth 2
        assert!(tracker.check_store_escape("p1", "p2").is_ok());
    }

    // ====================================================================
    // Scope depth management
    // ====================================================================

    #[test]
    fn test_nested_scope_depth() {
        let mut tracker = ArenaEscapeTracker::new();
        assert_eq!(tracker.current_depth(), 2);
        tracker.push_scope();
        assert_eq!(tracker.current_depth(), 3);
        tracker.register_arena("inner_arena"); // depth 3
        tracker.register_alloc("p", "inner_arena"); // depth 3
        assert!(tracker.check_return_escape("p").is_err());
        tracker.pop_scope();
        assert_eq!(tracker.current_depth(), 2);
    }

    // ====================================================================
    // The "Output Parameter" pattern — the safe idiom
    // ====================================================================

    #[test]
    fn test_output_parameter_pattern() {
        // fn create_node(arena: Arena, val: i32) -> Ptr<Node>
        let mut tracker = ArenaEscapeTracker::new();
        tracker.register_arena_arg("arena");     // depth 1 (argument)
        tracker.register_alloc("n", "arena");    // n inherits depth 1
        // return n — depth 1 <= 1 ✅
        assert!(tracker.check_return_escape("n").is_ok());
    }

    // ====================================================================
    // Phase 5: Allocator-Aware Vec<T, A> Escape Analysis
    // ====================================================================

    // --- Provenance Chain: Arena → ArenaAllocator → Vec ---

    #[test]
    fn test_arena_allocator_inherits_arena_depth() {
        // let arena = Arena::new(4096);            // depth 2
        // let alloc = ArenaAllocator { arena: arena };  // depth 2 (inherits)
        let mut tracker = ArenaEscapeTracker::new();
        tracker.register_arena("arena");                           // depth 2
        tracker.register_arena_allocator("alloc", "arena");        // depth 2
        assert_eq!(tracker.get_depth("alloc"), Some(2));
    }

    #[test]
    fn test_arena_vec_inherits_allocator_depth() {
        // let arena = Arena::new(4096);
        // let alloc = ArenaAllocator { arena: arena };
        // let v = Vec::new(alloc, 8);  // v inherits depth 2 from alloc
        let mut tracker = ArenaEscapeTracker::new();
        tracker.register_arena("arena");                           // depth 2
        tracker.register_arena_allocator("alloc", "arena");        // depth 2
        tracker.register_vec_from_allocator("v", "alloc");         // depth 2
        assert_eq!(tracker.get_depth("v"), Some(2));
    }

    // --- Law I: Arena-backed Vec CANNOT be returned ---

    #[test]
    fn test_arena_vec_cannot_return() {
        // fn bad() -> Vec<i64, ArenaAllocator> {
        //     let arena = Arena::new(4096);
        //     let alloc = ArenaAllocator { arena: arena };
        //     let v = Vec::new(alloc, 8);
        //     return v;  // ❌ depth 2 > 1
        // }
        let mut tracker = ArenaEscapeTracker::new();
        tracker.register_arena("arena");
        tracker.register_arena_allocator("alloc", "arena");
        tracker.register_vec_from_allocator("v", "alloc");
        assert!(tracker.check_return_escape("v").is_err());
    }

    // --- Law I: Heap-backed Vec CAN be returned ---

    #[test]
    fn test_heap_vec_can_return() {
        // fn ok() -> Vec<i64, HeapAllocator> {
        //     let v = Vec::new(HeapAllocator{}, 8);
        //     return v;  // ✅ not tracked → allowed
        // }
        let mut tracker = ArenaEscapeTracker::new();
        tracker.activate(); // Arena might exist elsewhere in function
        // HeapAllocator is NOT registered → v is NOT registered → not tracked
        assert!(tracker.check_return_escape("v").is_ok());
    }

    // --- Law I: Argument arena → Vec CAN be returned ---

    #[test]
    fn test_arg_arena_vec_can_return() {
        // fn ok(arena: Arena) -> Vec<i64, ArenaAllocator> {
        //     let alloc = ArenaAllocator { arena: arena };
        //     let v = Vec::new(alloc, 8);
        //     return v;  // ✅ depth 1 ≤ 1
        // }
        let mut tracker = ArenaEscapeTracker::new();
        tracker.register_arena_arg("arena");                       // depth 1
        tracker.register_arena_allocator("alloc", "arena");        // depth 1
        tracker.register_vec_from_allocator("v", "alloc");         // depth 1
        assert!(tracker.check_return_escape("v").is_ok());
    }

    // --- Law II: Arena-backed Vec stored into arg struct → REJECTED ---

    #[test]
    fn test_arena_vec_store_into_arg_fails() {
        // fn bad(ctx: Context) {
        //     let arena = Arena::new(4096);
        //     let alloc = ArenaAllocator { arena: arena };
        //     let v = Vec::new(alloc, 8);
        //     ctx.data = v;  // ❌ depth 2 > depth 1
        // }
        let mut tracker = ArenaEscapeTracker::new();
        tracker.register_arg("ctx");                               // depth 1
        tracker.register_arena("arena");                           // depth 2
        tracker.register_arena_allocator("alloc", "arena");        // depth 2
        tracker.register_vec_from_allocator("v", "alloc");         // depth 2
        assert!(tracker.check_store_escape("v", "ctx").is_err());
    }

    // --- Arena-backed Vec used locally → no error ---

    #[test]
    fn test_arena_vec_local_use_ok() {
        // fn ok() {
        //     let arena = Arena::new(4096);
        //     let alloc = ArenaAllocator { arena: arena };
        //     let v = Vec::new(alloc, 8);
        //     v.push(42);
        //     v.free();
        //     // No return of v → no escape
        // }
        let mut tracker = ArenaEscapeTracker::new();
        tracker.register_arena("arena");
        tracker.register_arena_allocator("alloc", "arena");
        tracker.register_vec_from_allocator("v", "alloc");
        // No check_return_escape("v") called → no error
        // Just verify depth is tracked correctly
        assert_eq!(tracker.get_depth("v"), Some(2));
    }

    // --- Nested scope: deeper depth ---

    #[test]
    fn test_nested_arena_vec_deeper_depth() {
        // fn bad() -> Vec<i64, ArenaAllocator> {
        //     if condition {
        //         let arena = Arena::new(4096);  // depth 3 (inside if)
        //         let alloc = ArenaAllocator { arena: arena };
        //         let v = Vec::new(alloc, 8);
        //         return v;  // ❌ depth 3 > 1
        //     }
        // }
        let mut tracker = ArenaEscapeTracker::new();
        tracker.push_scope(); // depth 3 (inside if block)
        tracker.register_arena("arena");                           // depth 3
        tracker.register_arena_allocator("alloc", "arena");        // depth 3
        tracker.register_vec_from_allocator("v", "alloc");         // depth 3
        assert_eq!(tracker.get_depth("v"), Some(3));
        assert!(tracker.check_return_escape("v").is_err());
    }

    // --- Multiple Vecs from same arena ---

    #[test]
    fn test_multiple_vecs_same_arena() {
        let mut tracker = ArenaEscapeTracker::new();
        tracker.register_arena("arena");
        tracker.register_arena_allocator("alloc", "arena");
        tracker.register_vec_from_allocator("v1", "alloc");
        tracker.register_vec_from_allocator("v2", "alloc");
        assert!(tracker.check_return_escape("v1").is_err());
        assert!(tracker.check_return_escape("v2").is_err());
    }

    // --- Mixed: one arena Vec, one heap Vec ---

    #[test]
    fn test_mixed_arena_and_heap_vecs() {
        let mut tracker = ArenaEscapeTracker::new();
        tracker.register_arena("arena");
        tracker.register_arena_allocator("alloc", "arena");
        tracker.register_vec_from_allocator("arena_v", "alloc");
        // heap_v is NOT registered — not tracked
        assert!(tracker.check_return_escape("arena_v").is_err());
        assert!(tracker.check_return_escape("heap_v").is_ok());
    }

    // --- Unregistered allocator → Vec not tracked ---

    #[test]
    fn test_vec_from_unregistered_allocator() {
        // If Vec::new is called with an allocator we don't track,
        // the Vec shouldn't be tainted (conservative: allow).
        let mut tracker = ArenaEscapeTracker::new();
        tracker.activate();
        tracker.register_vec_from_allocator("v", "unknown_alloc");
        // unknown_alloc has no depth → v gets no depth → not tracked
        assert!(tracker.check_return_escape("v").is_ok());
    }
}
