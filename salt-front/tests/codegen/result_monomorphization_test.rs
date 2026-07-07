// ============================================================================
// Result<T,E> Monomorphization Integration Tests (Phase 4.1)
//
// Tests that generic enum variant constructors are correctly generated
// when called from within generic method contexts.
//
// Root Cause (keuos_train): File::mmap<T> calls Result::Err(IOError)
// but emit_fn emits the body with empty type_map, leaving T unresolved.
// resolve_path_to_enum then rejects the enum variant construction.
// ============================================================================

use saltc::compile;

fn check_compiles(name: &str, source: &str, expected_error: Option<&str>) {
    match compile(source, false, None, true) {
        Ok(_) => {
            if let Some(err_msg) = expected_error {
                panic!("Test '{}' should have failed with '{}', but compiled successfully.", name, err_msg);
            }
        },
        Err(e) => {
            let e_str = e.to_string();
            match expected_error {
                Some(err_msg) => {
                    if !e_str.contains(err_msg) {
                        panic!("Test '{}' failed as expected, but with wrong message.\nExpected: {}\nActual: {}", name, err_msg, e_str);
                    }
                },
                None => {
                    panic!("Test '{}' should have compiled successfully, but failed: {}", name, e_str);
                }
            }
        }
    }
}

/// Tests that a generic method returning Result<T, E> can construct Ok/Err variants.
/// This is the minimal reproduction of the keuos_train mmap<T> failure.
#[test]
fn test_generic_method_result_ok_construction() {
    let source = r#"
    enum Result<T, E> {
        Ok(T),
        Err(E),
    }

    struct IOError { code: i32 }

    struct File { fd: i32 }

    impl File {
        fn get_result<T>(&self) -> Result<i32, IOError> {
            return Result::Ok(42);
        }
    }

    fn main() -> i32 {
        let f = File { fd: 1 };
        let r = f.get_result();
        return 0;
    }
    "#;
    check_compiles("generic_method_result_ok", source, None);
}

/// Tests that Result::Err is generated inside a generic method.
/// This is the exact pattern that fails in mmap<T>.
#[test]
fn test_generic_method_result_err_construction() {
    let source = r#"
    enum Result<T, E> {
        Ok(T),
        Err(E),
    }

    struct IOError { code: i32 }

    struct File { fd: i32 }

    impl File {
        fn try_something<T>(&self) -> Result<i32, IOError> {
            if self.fd < 0 {
                return Result::Err(IOError { code: 1 });
            }
            return Result::Ok(self.fd);
        }
    }

    fn main() -> i32 {
        let f = File { fd: 1 };
        let r = f.try_something();
        return 0;
    }
    "#;
    check_compiles("generic_method_result_err", source, None);
}

/// Tests `if ptr` sugar: pointer conditions should be accepted.
/// GREEN PHASE: Pointer truthiness is now implemented.
#[test]
fn test_ptr_truthiness_sugar() {
    let source = r#"
    fn main() -> i32 {
        let p: Ptr<i32> = 0 as Ptr<i32>;
        if p {
            return 1;
        }
        return 0;
    }
    "#;
    check_compiles("ptr_truthiness", source, None);
}

// ============================================================================
// Bidirectional Type Inference Integration Tests
//
// These test the FULL pipeline: bidirectional inference resolves T from the
// let-binding's type annotation, then the specialization bridge generates
// the correct mangled function name for MLIR emission.
// ============================================================================

/// Tests that a generic method's T is inferred from a Result<i32, E> type annotation.
/// Without bidirectional inference, this would fail with "does not reference a valid function".
#[test]
fn test_bidir_infer_from_result_type_annotation() {
    let source = r#"
    enum Result<T, E> {
        Ok(T),
        Err(E),
    }

    struct MyError { code: i32 }
    struct Service { id: i32 }

    impl Service {
        fn fetch<T>(&self) -> Result<i32, MyError> {
            return Result::Ok(self.id);
        }
    }

    fn main() -> i32 {
        let s = Service { id: 42 };
        let r: Result<i32, MyError> = s.fetch();
        return 0;
    }
    "#;
    check_compiles("bidir_infer_result_annotation", source, None);
}

/// Tests bidirectional inference with Ptr<T> — the core of the mmap scenario.
/// T should be inferred as f32 from the Ptr<f32> annotation.
#[test]
fn test_bidir_infer_ptr_return_type() {
    let source = r#"
    struct File { fd: i32 }

    impl File {
        fn load_raw<T>(&self, size: i64) -> Ptr<T> {
            return 0 as Ptr<T>;
        }
    }

    fn main() -> i32 {
        let f = File { fd: 3 };
        let p: Ptr<f32> = f.load_raw(100);
        return 0;
    }
    "#;
    check_compiles("bidir_infer_ptr_return_type", source, None);
}

/// Tests that two generic params (T, E) are inferred from a single type annotation.
#[test]
fn test_bidir_infer_two_params() {
    let source = r#"
    enum Result<T, E> {
        Ok(T),
        Err(E),
    }

    struct Parser { pos: i32 }
    struct ParseError { msg: i32 }

    impl Parser {
        fn parse<T, E>(&self) -> Result<i32, ParseError> {
            return Result::Ok(self.pos);
        }
    }

    fn main() -> i32 {
        let p = Parser { pos: 0 };
        let r: Result<i32, ParseError> = p.parse();
        return 0;
    }
    "#;
    check_compiles("bidir_infer_two_params", source, None);
}

