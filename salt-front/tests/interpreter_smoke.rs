// Smoke tests for the Salt interpreter — basic expression evaluation,
// control flow, and variable binding.
use saltc::grammar::SaltFile;
use saltc::interpreter::{Interpreter, Value};
use syn::parse::Parse;

fn run_salt(source: &str) -> Result<Value, String> {
    let file = syn::parse_str::<SaltFile>(source)
        .map_err(|e| format!("Parse error: {}", e))?;
    let mut interp = Interpreter::new();
    interp.run(&file)
}

#[test]
fn test_literal_i64() {
    let result = run_salt("fn main() -> i64 { return 42; }").unwrap();
    assert_eq!(result.as_i64(), 42);
}

#[test]
fn test_literal_bool_true() {
    let result = run_salt("fn main() -> bool { return true; }").unwrap();
    assert!(result.as_bool());
}

#[test]
fn test_literal_bool_false() {
    let result = run_salt("fn main() -> bool { return false; }").unwrap();
    assert!(!result.as_bool());
}

#[test]
fn test_arithmetic_add() {
    let result = run_salt("fn main() -> i64 { return 2 + 3; }").unwrap();
    assert_eq!(result.as_i64(), 5);
}

#[test]
fn test_arithmetic_sub() {
    let result = run_salt("fn main() -> i64 { return 10 - 3; }").unwrap();
    assert_eq!(result.as_i64(), 7);
}

#[test]
fn test_arithmetic_mul() {
    let result = run_salt("fn main() -> i64 { return 4 * 7; }").unwrap();
    assert_eq!(result.as_i64(), 28);
}

#[test]
fn test_variable_let() {
    let result = run_salt("fn main() -> i64 { let x: i64 = 10; return x; }").unwrap();
    assert_eq!(result.as_i64(), 10);
}

#[test]
fn test_if_true_branch() {
    let result = run_salt("fn main() -> i64 { if true { return 1; } else { return 0; } }").unwrap();
    assert_eq!(result.as_i64(), 1);
}

#[test]
fn test_if_false_branch() {
    let result = run_salt("fn main() -> i64 { if false { return 1; } else { return 0; } }").unwrap();
    assert_eq!(result.as_i64(), 0);
}

#[test]
fn test_comparison_lt() {
    let result = run_salt("fn main() -> bool { return 3 < 7; }").unwrap();
    assert!(result.as_bool());
}

#[test]
fn test_comparison_eq_false() {
    let result = run_salt("fn main() -> bool { return 5 == 9; }").unwrap();
    assert!(!result.as_bool());
}

#[test]
fn test_negation() {
    let result = run_salt("fn main() -> i64 { let x: i64 = 42; return -x; }").unwrap();
    assert_eq!(result.as_i64(), -42);
}
