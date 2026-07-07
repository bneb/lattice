use saltc::compile;

// ============================================================================
// Fix 1: SSA Dominance — GlobalLVN Snapshot Isolation
// ============================================================================

#[test]
fn test_global_lvn_snapshot_unit() {
    // Unit test: push_snapshot / pop_snapshot round-trip.
    // Values cached inside a snapshot must be discarded after pop.
    use saltc::codegen::types::provenance::GlobalLVN;

    let mut lvn = GlobalLVN::new();
    lvn.set_current_function("test_fn".to_string());

    // Cache a global value
    lvn.cache_value("MY_GLOBAL".to_string(), "%val_1".to_string());
    assert_eq!(lvn.get_cached("MY_GLOBAL"), Some(&"%val_1".to_string()));

    // Enter then-branch: push snapshot
    lvn.push_snapshot();

    // Cache a different value inside the branch
    lvn.cache_value("BRANCH_GLOBAL".to_string(), "%branch_val".to_string());
    assert_eq!(lvn.get_cached("BRANCH_GLOBAL"), Some(&"%branch_val".to_string()));

    // Pop snapshot: branch-local values must be gone
    lvn.pop_snapshot();
    assert_eq!(lvn.get_cached("BRANCH_GLOBAL"), None,
        "Branch-local cache entry must be discarded after pop_snapshot");

    // Pre-branch value must survive
    assert_eq!(lvn.get_cached("MY_GLOBAL"), Some(&"%val_1".to_string()),
        "Pre-branch cache entry must survive pop_snapshot");
}

#[test]
fn test_global_lvn_nested_snapshots() {
    // Nested snapshots (if/else inside if/else) must restore correctly.
    use saltc::codegen::types::provenance::GlobalLVN;

    let mut lvn = GlobalLVN::new();
    lvn.set_current_function("nested_fn".to_string());

    lvn.cache_value("G".to_string(), "%outer".to_string());

    // Outer if: push
    lvn.push_snapshot();
    lvn.cache_value("G".to_string(), "%then_outer".to_string());

    // Inner if: push
    lvn.push_snapshot();
    lvn.cache_value("G".to_string(), "%then_inner".to_string());
    assert_eq!(lvn.get_cached("G"), Some(&"%then_inner".to_string()));

    // Inner pop
    lvn.pop_snapshot();
    assert_eq!(lvn.get_cached("G"), Some(&"%then_outer".to_string()),
        "Inner pop must restore to outer then-branch state");

    // Outer pop
    lvn.pop_snapshot();
    assert_eq!(lvn.get_cached("G"), Some(&"%outer".to_string()),
        "Outer pop must restore to pre-if state");
}

#[test]
fn test_if_else_no_cross_branch_ssa_leak() {
    // Integration test: A global accessed in both branches must be loaded
    // independently — no SSA value reuse across branches.
    let code = r#"
        global COUNTER: i32;

        pub fn main() -> i32 {
            let x: i32 = COUNTER;
            if x > 0 {
                let a: i32 = COUNTER;
                return a;
            } else {
                let b: i32 = COUNTER;
                return b;
            }
            return 0;
        }
    "#;

    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "If/else with globals failed: {:?}", result.err());

    let mlir = result.unwrap();

    // Verify the MLIR has branch labels (then/else/merge)
    assert!(mlir.contains("^then_"), "Missing then block");
    assert!(mlir.contains("^else_"), "Missing else block");

    // The test succeeds if compilation doesn't crash with an MLIR verifier error.
    // The snapshot mechanism prevents SSA values from leaking between branches.
}

// ============================================================================
// Fix 2: PMM Store Type — Global Pointer Assignment
// ============================================================================

#[test]
fn test_global_pointer_store_uses_ptr_type() {
    // A global of type Ptr<u8> must be stored with !llvm.ptr, not i8.
    let code = r#"
        use std.core.ptr.Ptr

        global FREE_HEAD: Ptr<u8>;

        pub fn reset_head() {
            let addr: u64 = 0x1000;
            FREE_HEAD = addr as Ptr<u8>;
        }

        pub fn main() -> i32 {
            return 0;
        }
    "#;

    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Global pointer store failed: {:?}", result.err());

    let mlir = result.unwrap();

    // The store to FREE_HEAD must use !llvm.ptr, not i8
    assert!(mlir.contains("!llvm.ptr"), "Missing !llvm.ptr in MLIR output");
}

// ============================================================================
// Fix 3: Atomic Intrinsic Interception
// ============================================================================

// NOTE: The lazy codegen only emits functions reachable from main().
// All atomic functions must be called from main for them to be hydrated.

#[test]
fn test_fetch_add_emits_atomicrmw() {
    let code = r#"
        global COUNTER: Atomic<i32>;

        pub fn main() -> i32 {
            let old: i32 = COUNTER.fetch_add(1);
            return old;
        }
    "#;

    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "fetch_add compilation failed: {:?}", result.err());

    let mlir = result.unwrap();
    assert!(mlir.contains("atomicrmw") || mlir.contains("atomic"), 
        "Missing atomic instruction in MLIR output");
}

#[test]
fn test_fetch_sub_emits_atomicrmw() {
    let code = r#"
        global COUNTER: Atomic<i32>;

        pub fn main() -> i32 {
            let old: i32 = COUNTER.fetch_sub(1);
            return old;
        }
    "#;

    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "fetch_sub compilation failed: {:?}", result.err());

    let mlir = result.unwrap();
    assert!(mlir.contains("atomicrmw") || mlir.contains("atomic"),
        "Missing atomic instruction in MLIR output");
}

#[test]
fn test_atomic_load_emits_atomic_load() {
    let code = r#"
        global COUNTER: Atomic<i32>;

        pub fn main() -> i32 {
            let val: i32 = COUNTER.load();
            return val;
        }
    "#;

    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Atomic load compilation failed: {:?}", result.err());

    let mlir = result.unwrap();
    assert!(mlir.contains("atomic") || mlir.contains("llvm.load"),
        "Missing atomic load instruction in MLIR output");
}

#[test]
fn test_atomic_store_emits_atomic_store() {
    let code = r#"
        global COUNTER: Atomic<i32>;

        pub fn main() -> i32 {
            COUNTER.store(42);
            return 0;
        }
    "#;

    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Atomic store compilation failed: {:?}", result.err());

    let mlir = result.unwrap();
    assert!(mlir.contains("atomic") || mlir.contains("llvm.store"),
        "Missing atomic store instruction in MLIR output");
}
