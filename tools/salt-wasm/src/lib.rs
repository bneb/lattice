//! WebAssembly bridge for the Salt compiler.
//!
//! Exposes Salt compilation to JavaScript via `wasm-bindgen`. The browser gives
//! us a string of Salt code, and we return structured JSON with:
//! - Parse/type/codegen diagnostics (for Monaco red squiggles)
//! - Compiled MLIR output (Salt Intermediate Representation)
//!
//! Z3 verification is disabled in Wasm builds (`no_verify = true`).
//! Heavyweight formal proofs are deferred to the native CI toolchain.

use wasm_bindgen::prelude::*;
use serde::Serialize;

/// Initialize panic hook for better Wasm error messages.
/// Called automatically on first compilation.
fn ensure_panic_hook() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        console_error_panic_hook::set_once();
    });
}

/// Structured compilation result returned to JavaScript as JSON.
#[derive(Serialize)]
pub struct CompileResult {
    /// Whether compilation succeeded
    pub success: bool,
    /// The compiled MLIR output (empty string on failure)
    pub mlir: String,
    /// Diagnostic messages (errors, warnings)
    pub diagnostics: Vec<Diagnostic>,
}

/// A single diagnostic message.
#[derive(Serialize)]
pub struct Diagnostic {
    /// "error", "warning", or "info"
    pub severity: String,
    /// The diagnostic message text
    pub message: String,
    /// Optional line number (1-indexed, 0 if unknown)
    pub line: usize,
}

/// Compile Salt source code and return structured JSON.
///
/// # Arguments
/// * `source` - Salt source code string
///
/// # Returns
/// JSON string with shape `{ success: bool, mlir: string, diagnostics: [...] }`
#[wasm_bindgen]
pub fn compile(source: &str) -> String {
    ensure_panic_hook();

    // Preprocess Salt syntax extensions into syn-parseable Rust
    let processed = saltc::preprocess(source);

    // Parse into AST using salt-front's re-exported syn
    let parse_result: Result<saltc::grammar::SaltFile, _> = syn::parse_str(&processed);
    let mut file = match parse_result {
        Ok(f) => f,
        Err(e) => {
            return to_json(&CompileResult {
                success: false,
                mlir: String::new(),
                diagnostics: vec![Diagnostic {
                    severity: "error".into(),
                    message: format!("Parse error: {}", e),
                    line: e.span().start().line,
                }],
            });
        }
    };

    // Compile AST to MLIR with Z3 disabled
    match saltc::compile_ast(
        &mut file,
        false,    // release_mode
        None,     // registry
        false,    // skip_scan
        false,    // vverify
        true,     // disable_alias_scopes (cleaner MLIR for display)
        true,     // no_verify (Z3 is stubbed — skip all verification)
        false,    // lib_mode
        false,    // sip_mode
        false,    // debug_info
        "<repl>", // source_file
    ) {
        Ok(mlir) => to_json(&CompileResult {
            success: true,
            mlir,
            diagnostics: vec![],
        }),
        Err(e) => {
            let msg = format!("{}", e);
            to_json(&CompileResult {
                success: false,
                mlir: String::new(),
                diagnostics: vec![Diagnostic {
                    severity: "error".into(),
                    message: msg.clone(),
                    line: extract_line_number(&msg),
                }],
            })
        }
    }
}

/// Parse-only check: validates Salt syntax without full compilation.
/// Faster than `compile()` — use for real-time editor feedback on keystroke.
#[wasm_bindgen]
pub fn check(source: &str) -> String {
    let processed = saltc::preprocess(source);
    match syn::parse_str::<saltc::grammar::SaltFile>(&processed) {
        Ok(_) => to_json(&CompileResult {
            success: true,
            mlir: String::new(),
            diagnostics: vec![],
        }),
        Err(e) => to_json(&CompileResult {
            success: false,
            mlir: String::new(),
            diagnostics: vec![Diagnostic {
                severity: "error".into(),
                message: format!("Parse error: {}", e),
                line: e.span().start().line,
            }],
        }),
    }
}

/// Structured run result returned to JavaScript as JSON.
#[derive(Serialize)]
pub struct RunResult {
    pub success: bool,
    pub stdout: String,
    pub exit_code: i32,
    pub error: String,
}

/// Run a Salt program and return its stdout output.
///
/// Parses the source, then executes it via the AST interpreter.
/// Returns JSON with shape `{ success: bool, stdout: string, exit_code: number, error: string }`
#[wasm_bindgen]
pub fn run(source: &str) -> String {
    ensure_panic_hook();

    let processed = saltc::preprocess(source);

    let file: saltc::grammar::SaltFile = match syn::parse_str(&processed) {
        Ok(f) => f,
        Err(e) => {
            let result = RunResult {
                success: false,
                stdout: String::new(),
                exit_code: 1,
                error: format!("Parse error: {}", e),
            };
            return serde_json::to_string(&result).unwrap_or_default();
        }
    };

    let mut interp = saltc::interpreter::Interpreter::new();
    match interp.run(&file) {
        Ok(val) => {
            let exit_code = val.as_i32();
            let result = RunResult {
                success: true,
                stdout: interp.stdout,
                exit_code,
                error: String::new(),
            };
            serde_json::to_string(&result).unwrap_or_default()
        }
        Err(e) => {
            let result = RunResult {
                success: false,
                stdout: interp.stdout,
                exit_code: 1,
                error: e,
            };
            serde_json::to_string(&result).unwrap_or_default()
        }
    }
}

/// Return the compiler version string.
#[wasm_bindgen]
pub fn version() -> String {
    format!("Salt Compiler (Wasm) v{}", env!("CARGO_PKG_VERSION"))
}

fn to_json(result: &CompileResult) -> String {
    serde_json::to_string(result).unwrap_or_else(|_| r#"{"success":false,"mlir":"","diagnostics":[]}"#.into())
}

fn extract_line_number(msg: &str) -> usize {
    if let Some(pos) = msg.to_lowercase().find("line ") {
        let after = &msg[pos + 5..];
        if let Some(end) = after.find(|c: char| !c.is_ascii_digit()) {
            if let Ok(n) = after[..end].parse::<usize>() {
                return n;
            }
        }
    }
    0
}
