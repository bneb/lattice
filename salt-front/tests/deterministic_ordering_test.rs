// Unit tests for deterministic generic argument ordering.
//
// These tests verify that MonomorphizationTask creation and mangle_specialization
// always produce generic argument suffixes in the struct's DECLARED parameter order
// (e.g., Vec<T, A> → _T_A), never in non-deterministic HashMap iteration order.
//
// Regression tests for: Non-deterministic Vec<T, A> mangling producing
// _ArenaAllocator_i64 instead of _i64_ArenaAllocator.

use salt_front::grammar::SaltFile;

/// Helper: compile Salt source and return MLIR output
fn compile(src: &str) -> Result<String, String> {
    let file: SaltFile = syn::parse_str(src)
        .map_err(|e| format!("Parse error: {}", e))?;
    salt_front::codegen::emit_mlir(&file, false, None, false, true, false, false, false, false, "")
}

// ─── Two-Parameter Struct: Declaration Order ───────────────────────────────

#[test]
fn test_two_param_struct_method_mangling_order() {
    // Vec<T, A> declares T first, A second.
    // new() with <i64, HeapAlloc> must mangle as _i64_HeapAlloc, never _HeapAlloc_i64.
    let src = r#"
        struct HeapAlloc { dummy: i64 }
        struct Vec<T, A> { len: i64, cap: i64 }
        impl<T, A> Vec<T, A> {
            fn new(alloc: A, cap: i64) -> Vec<T, A> {
                Vec::<T, A> { len: 0, cap: cap }
            }
        }
        fn main() {
            let a = HeapAlloc { dummy: 0 };
            let v = Vec::<i64, HeapAlloc>::new(a, 16);
        }
    "#;
    let mlir = compile(src).expect("Compilation failed");

    // Correct order: T=i64 first, A=HeapAlloc second
    assert!(
        mlir.contains("Vec__new_i64_HeapAlloc"),
        "Expected Vec__new_i64_HeapAlloc (T then A), but not found in MLIR:\n{}",
        mlir
    );
    // Must NOT contain wrong order
    assert!(
        !mlir.contains("Vec__new_HeapAlloc_i64"),
        "Found WRONG order Vec__new_HeapAlloc_i64 (A then T) in MLIR:\n{}",
        mlir
    );
}

#[test]
fn test_two_param_struct_multiple_methods_consistent() {
    // All methods on Vec<T, A> must use the same T-then-A order.
    let src = r#"
        struct Alloc { dummy: i64 }
        struct Vec<T, A> { len: i64 }
        impl<T, A> Vec<T, A> {
            fn new(a: A) -> Vec<T, A> {
                Vec::<T, A> { len: 0 }
            }
            fn len(self) -> i64 {
                self.len
            }
        }
        fn main() {
            let a = Alloc { dummy: 0 };
            let v = Vec::<bool, Alloc>::new(a);
            let n = v.len();
        }
    "#;
    let mlir = compile(src).expect("Compilation failed");

    // Both methods must have bool_Alloc suffix (T=bool, A=Alloc)
    // Static methods: Struct__method_TypeArgs
    // Instance methods: SpecializedStruct__method
    assert!(
        mlir.contains("Vec__new_bool_Alloc"),
        "Expected Vec__new_bool_Alloc in MLIR:\n{}",
        mlir
    );
    assert!(
        mlir.contains("Vec_bool_Alloc__len"),
        "Expected Vec_bool_Alloc__len in MLIR:\n{}",
        mlir
    );
    // Wrong order must not appear
    assert!(
        !mlir.contains("Vec__new_Alloc_bool"),
        "Found wrong order Vec__new_Alloc_bool in MLIR"
    );
    assert!(
        !mlir.contains("Vec_Alloc_bool__len"),
        "Found wrong order Vec_Alloc_bool__len in MLIR"
    );
}

// ─── Three-Parameter Struct: Stress Test ───────────────────────────────────

#[test]
fn test_three_param_struct_preserves_declaration_order() {
    // Map<K, V, H> declares K, V, H in that order.
    let src = r#"
        struct DefaultHash { seed: i64 }
        struct Map<K, V, H> { size: i64 }
        impl<K, V, H> Map<K, V, H> {
            fn new(hasher: H) -> Map<K, V, H> {
                Map::<K, V, H> { size: 0 }
            }
        }
        fn main() {
            let h = DefaultHash { seed: 42 };
            let m = Map::<i64, bool, DefaultHash>::new(h);
        }
    "#;
    let mlir = compile(src).expect("Compilation failed");

    // Must be K_V_H order: i64_bool_DefaultHash
    assert!(
        mlir.contains("Map__new_i64_bool_DefaultHash"),
        "Expected Map__new_i64_bool_DefaultHash (K, V, H order) in MLIR:\n{}",
        mlir
    );
}

// ─── Single-Parameter Struct: Baseline ─────────────────────────────────────

#[test]
fn test_single_param_struct_unaffected() {
    // Single generic param should work the same as before.
    let src = r#"
        struct Box<T> { val: T }
        impl<T> Box<T> {
            fn new(v: T) -> Box<T> {
                Box::<T> { val: v }
            }
            fn get(self) -> T {
                self.val
            }
        }
        fn main() {
            let b = Box::<i64>::new(42);
            let v = b.get();
        }
    "#;
    let mlir = compile(src).expect("Compilation failed");

    assert!(
        mlir.contains("Box__new_i64"),
        "Expected Box__new_i64 in MLIR:\n{}",
        mlir
    );
    // Instance methods use SpecializedStruct__method convention
    assert!(
        mlir.contains("Box_i64__get"),
        "Expected Box_i64__get in MLIR:\n{}",
        mlir
    );
}

// ─── Deterministic Across Runs ─────────────────────────────────────────────

#[test]
fn test_deterministic_across_multiple_compilations() {
    // Compile the same source 5 times and verify identical MLIR output.
    // This catches non-determinism from HashMap iteration order varying between runs.
    let src = r#"
        struct Pool { id: i64 }
        struct Container<T, A> { count: i64 }
        impl<T, A> Container<T, A> {
            fn create(pool: A) -> Container<T, A> {
                Container::<T, A> { count: 0 }
            }
        }
        fn main() {
            let p = Pool { id: 1 };
            let c = Container::<bool, Pool>::create(p);
        }
    "#;

    let first_mlir = compile(src).expect("First compilation failed");
    for i in 1..5 {
        let mlir = compile(src).expect(&format!("Compilation {} failed", i + 1));
        assert_eq!(
            first_mlir, mlir,
            "Non-deterministic output on compilation {}! Diff in mangled names.",
            i + 1
        );
    }

    // Also verify correct order
    assert!(
        first_mlir.contains("Container__create_bool_Pool"),
        "Expected Container__create_bool_Pool (T then A) in MLIR:\n{}",
        first_mlir
    );
}
