//! TDD Tests for Pointer Safety Enforcement (Phase 3.6)
//!
//! Red Phase: These tests are designed to FAIL until enforcement is implemented.
//! - Tests expecting errors (deref_empty, deref_optional, scope_merge) will
//!   currently PASS compilation (no guard), so the test assertion fails.
//! - Tests expecting success (valid_box, narrowing) may fail for unrelated
//!   reasons (std lib resolution) — that's OK for Red phase.

#[cfg(test)]
mod tests {
    fn check_safety(name: &str, source: &str, expected_error: Option<&str>) {
        let result = crate::compile(source, false, None, true);

        match result {
            Ok(_) => {
                if let Some(err_msg) = expected_error {
                    panic!(
                        "Test '{}' should have failed with '{}', but compiled successfully.",
                        name, err_msg
                    );
                }
            }
            Err(e) => {
                let err_str = format!("{}", e);
                match expected_error {
                    Some(err_msg) => {
                        if !err_str.contains(err_msg) {
                            panic!(
                                "Test '{}' failed as expected, but with wrong message.\nExpected: {}\nActual: {}",
                                name, err_msg, err_str
                            );
                        }
                    }
                    None => {
                        panic!(
                            "Test '{}' should have compiled successfully, but failed: {}",
                            name, err_str
                        );
                    }
                }
            }
        }
    }

    // ========================================================================
    // Dereference Guards
    // ========================================================================

    /// Dereferencing Ptr::empty() must produce a compile error.
    #[test]
    fn test_deref_empty_should_fail() {
        let source = r#"
            package main
            use std.core.ptr.Ptr

            struct Node { val: i32 }

            fn main() -> i32 {
                let p = Ptr<Node>::empty();
                let x = p.val;
                return 0;
            }
        "#;
        check_safety("deref_empty", source, Some("Empty"));
    }

    /// Accessing .addr on Empty pointer must be ALLOWED (it's how you check).
    #[test]
    fn test_addr_access_on_empty_is_allowed() {
        let source = r#"
            package main
            use std.core.ptr.Ptr

            struct Node { val: i32 }

            fn main() -> i32 {
                let p = Ptr<Node>::empty();
                let a = p.addr;
                return 0;
            }
        "#;
        check_safety("addr_on_empty", source, None);
    }

    // ========================================================================
    // Flow-Sensitive Narrowing
    // ========================================================================

    /// `if p.addr != 0 { p.val }` must succeed (narrowed to Valid).
    #[test]
    fn test_narrowing_not_zero_should_pass() {
        let source = r#"
            package main
            use std.core.ptr.Ptr

            struct Node { val: i32 }

            fn check(p: Ptr<Node>) -> i32 {
                if p.addr != 0 {
                    return p.val;
                }
                return 0;
            }

            fn main() -> i32 {
                check(Ptr<Node>::empty());
                return 0;
            }
        "#;
        check_safety("narrowing_not_zero", source, None);
    }

    /// `if p.addr == 0 { } else { p.val }` must succeed (inverse narrowing).
    #[test]
    fn test_narrowing_else_zero_should_pass() {
        let source = r#"
            package main
            use std.core.ptr.Ptr

            struct Node { val: i32 }

            fn check(p: Ptr<Node>) -> i32 {
                if p.addr == 0 {
                    return 0;
                } else {
                    return p.val;
                }
            }

            fn main() -> i32 {
                check(Ptr<Node>::empty());
                return 0;
            }
        "#;
        check_safety("narrowing_else_zero", source, None);
    }

    /// Ptr::empty() narrowed to Valid inside `if p.addr != 0` must revert
    /// to Empty after the block — dereferencing outside must fail.
    #[test]
    fn test_empty_ptr_narrowing_does_not_persist() {
        let source = r#"
            package main
            use std.core.ptr.Ptr

            struct Node { val: i32 }

            fn main() -> i32 {
                let p = Ptr<Node>::empty();
                if p.addr != 0 {
                    let y = p.val;
                }
                let x = p.val;
                return x;
            }
        "#;
        check_safety("narrowing_scope_merge", source, Some("Empty"));
    }

    // ========================================================================
    // PointerStateTracker Unit Tests (these should pass immediately)
    // ========================================================================

    #[test]
    fn test_tracker_narrowing_merge_returns_optional() {
        use crate::codegen::verification::PointerStateTracker;
        use crate::codegen::verification::PointerState;

        let mut tracker = PointerStateTracker::new();
        tracker.mark_optional("p");

        // Enter if-branch: narrow to Valid
        tracker.push_scope();
        tracker.mark_valid("p");
        assert_eq!(tracker.get_state("p"), Some(PointerState::Valid));
        assert!(tracker.check_deref("p").is_ok());

        // Restore pre-if state
        let _saved = tracker.pop_scope().unwrap();

        // Build a "then" map to merge with
        let mut then_map = std::collections::HashMap::new();
        then_map.insert("p".to_string(), PointerState::Valid);

        // Restore tracker to saved state
        tracker.mark_optional("p"); // restored

        // Merge: Valid + Optional = Optional
        tracker.merge(&then_map);
        assert_eq!(tracker.get_state("p"), Some(PointerState::Optional));
        assert!(tracker.check_deref("p").is_err());
    }

    #[test]
    fn test_tracker_both_branches_valid_stays_valid() {
        use crate::codegen::verification::PointerStateTracker;
        use crate::codegen::verification::PointerState;

        let mut tracker = PointerStateTracker::new();
        tracker.mark_optional("p");

        let mut then_map = std::collections::HashMap::new();
        then_map.insert("p".to_string(), PointerState::Valid);

        tracker.mark_valid("p"); // else branch also Valid

        tracker.merge(&then_map);
        assert_eq!(tracker.get_state("p"), Some(PointerState::Valid));
    }

    // ========================================================================
    // Regression: Ptr::empty() in assignment must not leak state
    // ========================================================================

    /// Regression test: `node.children[i] = Ptr::empty()` must not leak
    /// Empty state to a later function's `let child = ...` binding.
    /// This was causing the trie benchmark to fail with:
    ///   "Cannot dereference 'Empty' pointer 'curr'"
    #[test]
    fn test_ptr_empty_in_assign_does_not_leak_to_later_fn() {
        let source = r#"
            package main
            use std.core.ptr.Ptr
            use std.core.arena

            struct TrieNode {
                children: [Ptr<TrieNode>; 4],
                is_word: u8
            }

            fn create_node() -> Ptr<TrieNode> {
                let node = arena::alloc(40) as Ptr<TrieNode>;
                for i in 0..4 {
                    node.children[i as i64] = Ptr::empty();
                }
                node.is_word = 0;
                return node;
            }

            fn search(root: Ptr<TrieNode>, idx: i32) -> bool {
                if root {
                    let child = root.children[idx as i64];
                    if child {
                        return child.is_word != 0;
                    }
                }
                return false;
            }

            fn main() -> i32 {
                let root = create_node();
                search(root, 0);
                return 0;
            }
        "#;
        // This should compile without error. Before the fix, the Empty state
        // from Ptr::empty() in create_node's array assignment leaked to
        // search's `let child` binding, causing a false "Empty pointer" error.
        check_safety("ptr_empty_assign_no_leak", source, None);
    }

    // ========================================================================
    // TS-01: Basic Affine Type Tracking
    // ========================================================================

    #[test]
    fn test_use_uninitialized_pointer_fails() {
        let source = r#"
            package main
            use std.core.ptr.Ptr

            fn main() -> i32 {
                let p: Ptr<i32>;
                // Use uninitialized pointer
                let x = p;
                return 0;
            }
        "#;
        check_safety("use_uninitialized", source, Some("Use of uninitialized pointer variable: p"));
    }

    #[test]
    fn test_use_freed_pointer_fails() {
        let source = r#"
            package main
            use std.core.ptr.Ptr
            
            extern fn free(p: Ptr<i32>);

            fn main() -> i32 {
                let p: Ptr<i32> = Ptr::<i32>::empty();
                // We use free which will mark it Freed
                free(p);
                // Cannot pass a freed pointer around
                let x = p;
                return 0;
            }
        "#;
        check_safety("use_freed", source, Some("Use of freed pointer variable: p"));
        check_safety("use_freed", source, Some("Use of freed pointer variable: p"));
    }

    #[test]
    fn test_custom_deallocator_ensures() {
        let source = r#"
            package main
            use std.core.ptr.Ptr
            
            extern fn get_valid_ptr() -> Ptr<i32> ensures valid(result);
            extern fn custom_free(p: Ptr<i32>) requires valid(p); ensures freed(p);

            fn main() -> i32 {
                let p = get_valid_ptr();
                custom_free(p);
                // Cannot pass a freed pointer
                let x = p;
                return 0;
            }
        "#;
        check_safety("custom_free", source, Some("Use of freed pointer variable: p"));
    }

    #[test]
    fn test_dynamic_check_fallback() {
        let source = r#"
            package main
            use std.core.ptr.Ptr
            
            extern fn get_valid_ptr() -> Ptr<i32>;
            extern fn opaque_function(p: Ptr<i32>);

            fn main() -> i32 {
                let p = get_valid_ptr();
                // opaque_function makes the pointer Optional (unprovable statically)
                opaque_function(p);
                
                // This would normally fail compile time:
                // let val = *p; 

                // But with dynamic check, it defers to runtime (compiles successfully!)
                @dynamic_check {
                    let val = *p;
                }
                return 0;
            }
        "#;
        // Because it's enclosed in @dynamic_check, it should COMPILE successfully,
        // and rely on the runtime Software Memory Tagging to trap.
        check_safety("dynamic_check", source, None);
    }
}
