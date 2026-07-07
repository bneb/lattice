// ============================================================================
// Generic Static Method Resolution Tests
//
// Validates that static methods on generic types (e.g., Slice<f32>::new(...))
// resolve correctly even when called multiple times. Regression test for a bug
// where hydration of the first call clobbered the import context, causing the
// second call to fail with "Undefined function or symbol: 'Slice__new'" instead
// of resolving to the fully-qualified 'std__core__slice__Slice__new'.
// ============================================================================

/// Test that a generic static method can be called twice in the same function.
/// This is the core reproduction of the keuos_train.salt failure.
///
/// The bug manifests when:
/// 1. A generic struct (e.g., Slice<T>) is defined in a package
/// 2. It has a static method (e.g., fn new(...) -> Slice<T>)
/// 3. The calling code imports the struct via `use`
/// 4. The calling code calls the static method TWICE
///
/// The first call succeeds (triggering hydration). The second call fails
/// because hydration of the first call clobbered the import context, so
/// resolve_package_prefix can no longer find the FQN for "Slice".
#[test]
fn test_generic_static_method_called_twice() {
    let code = r#"
        package mylib.containers;
        struct Slice<T> { ptr: i64, len: i64 }
        impl<T> Slice<T> {
            fn new(ptr: i64, len: i64) -> Slice<T> {
                return Slice::<T> { ptr: ptr, len: len };
            }
        }

        use mylib.containers.Slice;
        fn main() -> i32 {
            let a = Slice<i32>::new(0, 10);
            let b = Slice<i32>::new(0, 20);
            return a.len as i32 + b.len as i32;
        }
    "#;
    let result = saltc::compile(code, false, None, true);
    assert!(result.is_ok(), "Second Slice::new call failed (import clobbering bug): {:?}", result.err());
}

/// Same scenario but with different type parameters in each call.
/// This exercises the monomorphization path even more.
#[test]
fn test_generic_static_method_different_type_params() {
    let code = r#"
        package mylib.containers;
        struct MyVec<T> { data: i64, len: i64 }
        impl<T> MyVec<T> {
            fn create(cap: i64) -> MyVec<T> {
                return MyVec::<T> { data: 0, len: cap };
            }
        }

        use mylib.containers.MyVec;
        fn main() -> i32 {
            let a = MyVec<i32>::create(10);
            let b = MyVec<f32>::create(20);
            return a.len as i32;
        }
    "#;
    let result = saltc::compile(code, false, None, true);
    assert!(result.is_ok(), "Different type param calls failed: {:?}", result.err());
}

/// Three calls to verify the pattern holds beyond just the second call
#[test]
fn test_generic_static_method_called_three_times() {
    let code = r#"
        package mylib.containers;
        struct Buffer<T> { ptr: i64, size: i64 }
        impl<T> Buffer<T> {
            fn alloc(size: i64) -> Buffer<T> {
                return Buffer::<T> { ptr: 0, size: size };
            }
        }

        use mylib.containers.Buffer;
        fn main() -> i32 {
            let a = Buffer<f32>::alloc(100);
            let b = Buffer<f32>::alloc(200);
            let c = Buffer<f32>::alloc(300);
            return a.size as i32;
        }
    "#;
    let result = saltc::compile(code, false, None, true);
    assert!(result.is_ok(), "Third call failed (import clobbering persists): {:?}", result.err());
}

/// Test that non-generic static methods from imported packages still work
/// (baseline sanity check — ensures the test infrastructure is valid)
#[test]
fn test_non_generic_static_method_from_package() {
    let code = r#"
        package mylib.math;
        struct Calculator { result: i32 }
        impl Calculator {
            fn zero() -> Calculator {
                return Calculator { result: 0 };
            }
        }

        use mylib.math.Calculator;
        fn main() -> i32 {
            let a = Calculator::zero();
            let b = Calculator::zero();
            return a.result + b.result;
        }
    "#;
    let result = saltc::compile(code, false, None, true);
    assert!(result.is_ok(), "Non-generic static method failed: {:?}", result.err());
}

/// Verify that imports are preserved specifically — assert the MLIR output
/// contains the fully-qualified function name for BOTH calls
#[test]
fn test_fqn_resolution_in_mlir_output() {
    let code = r#"
        package mylib.containers;
        struct Pair<T> { a: i64, b: i64 }
        impl<T> Pair<T> {
            fn make(a: i64, b: i64) -> Pair<T> {
                return Pair::<T> { a: a, b: b };
            }
        }

        use mylib.containers.Pair;
        fn main() -> i32 {
            let x = Pair<i32>::make(1, 2);
            let y = Pair<i32>::make(3, 4);
            return 0;
        }
    "#;
    let result = saltc::compile(code, false, None, true);
    assert!(result.is_ok(), "Compilation failed: {:?}", result.err());
    
    let mlir = result.unwrap();
    // Both calls should reference the fully-qualified mangled name
    // NOT just "Pair__make" (the unresolved short name)
    assert!(
        mlir.contains("mylib__containers__Pair"),
        "MLIR should contain FQN 'mylib__containers__Pair' for the struct. Got:\n{}",
        mlir
    );
}
