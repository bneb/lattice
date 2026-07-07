use saltc::cli::run_cli;

#[test]
fn test_cli_usage() {
    // 1. No args -> Usage
    let args = vec!["salt-front".to_string()];
    let res = run_cli(args);
    assert!(res.is_ok()); // It OKs after printing usage
}

#[test]
fn test_cli_help() {
    // 2. Help flag
    let args = vec!["salt-front".to_string(), "--help".to_string()];
    let res = run_cli(args);
    assert!(res.is_ok());
}

#[test]
fn test_cli_file_error() {
    // 3. File not found
    let args = vec!["salt-front".to_string(), "nonexistent.salt".to_string()];
    let res = run_cli(args);
    assert!(res.is_err());
    let err = res.err().unwrap().to_string();
    assert!(err.contains("Failed to read source file"));
}

#[test]
fn test_cli_success() {
    // 4. Successful compilation
    // Use an existing simple file
    let args = vec!["salt-front".to_string(), "tests/cases/integers.salt".to_string()];
    let res = run_cli(args);
    assert!(res.is_ok());
}

#[test]
fn test_cli_release() {
    // 5. Release mode
    let args = vec![
        "salt-front".to_string(), 
        "tests/cases/integers.salt".to_string(),
        "--release".to_string()
    ];
    let res = run_cli(args);
    assert!(res.is_ok());
}
