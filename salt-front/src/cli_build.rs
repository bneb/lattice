use std::path::PathBuf;
use std::str::FromStr;

use crate::cli::CliConfig;

pub(crate) fn emit_sir_file(file: &crate::grammar::SaltFile, module_name: &str, output_path: Option<&str>) {
    use crate::codegen::sir::types::*;
    use crate::codegen::sir::sir_emit::*;

    let sir_module = extract_sir_from_ast(file, module_name);
    let sir_json = sir_module.to_json();
    let sir_path = output_path
        .map(|p| format!("{}.sir.json", p.trim_end_matches(".mlir")))
        .unwrap_or_else(|| format!("{}.sir.json", module_name));

    if let Err(e) = std::fs::write(&sir_path, &sir_json) {
        eprintln!("[E008] SIR emission failed: {}", e);
    } else {
        eprintln!("SIR emitted: {} ({} structs, {} functions, v{})",
            sir_path, sir_module.structs.len(), sir_module.functions.len(), SIR_VERSION);
    }
}

pub(crate) fn handle_binary_synthesis(mlir: &str, basename: &str, config: &CliConfig) {
    let build_dir = std::env::temp_dir().join("salt-build");
    let mut driver = crate::driver::SaltDriver::new(build_dir);
    if let Some(ref t) = config.target_name {
        let t_parsed = crate::driver::DriverTarget::from_str(t)
            .unwrap_or_else(|e| {
                eprintln!("[E005] {}", e);
                std::process::exit(1);
            });
        driver = driver.with_target(t_parsed);
    }

    let mut output_bin = config.output_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(basename));
    // Windows executables need .exe extension
    if driver.target.exe_suffix() == ".exe" && output_bin.extension().is_none_or(|e| e != "exe") {
        output_bin.set_extension("exe");
    }

    // Compile runtime if not already present
    let rt_src = crate::driver::SaltDriver::runtime_source(&driver.target);
    if !driver.runtime_obj.exists() {
        let rt_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rt_src);
        if rt_path.exists() {
            let mut compile_cmd = std::process::Command::new(&driver.toolchain.clang);
            compile_cmd.arg("-c").arg(&rt_path).arg("-o").arg(&driver.runtime_obj);
            if driver.target.exe_suffix() == ".exe" {
                compile_cmd.arg("-target").arg("x86_64-pc-windows-msvc");
            }
            if let Err(e) = compile_cmd.status() {
                eprintln!("[E005] Failed to compile {}: {}", rt_src, e);
                std::process::exit(1);
            }
        }
    }

    eprintln!("[KeuOS] Driving MLIR -> native binary...");
    eprintln!("    Target: {:?}", driver.target);

    let is_keuos = matches!(driver.target,
        crate::driver::DriverTarget::KeuOSArm64 |
        crate::driver::DriverTarget::KeuOSX86_64
    );

    let compile_result = if is_keuos {
        eprintln!("    Linker: ld.lld (freestanding ELF)");
        driver.compile_keuos_binary(mlir, basename)
    } else {
        eprintln!("    Runtime: {:?}", driver.runtime_obj);
        driver.compile(mlir, basename)
    };

    match compile_result {
        Ok(produced_path) => {
            if produced_path != output_bin {
                if let Err(e) = std::fs::copy(&produced_path, &output_bin) {
                    eprintln!("[E005] Failed to copy binary to {:?}: {}", output_bin, e);
                    std::process::exit(1);
                }
            }

            eprintln!("[KeuOS] Running KeuOS Audit...");
            if let Ok(output) = std::process::Command::new("otool").arg("-tV").arg(&output_bin).output() {
                let disasm = String::from_utf8_lossy(&output.stdout);
                let audit_config = crate::codegen::passes::binary_audit::BinaryAuditConfig::standard(
                    crate::codegen::passes::io_backend::TargetPlatform::Darwin
                );
                let results = crate::codegen::passes::binary_audit::run_audit(&audit_config, &disasm);
                let mut all_passed = true;
                for res in results {
                    if !res.passed {
                        all_passed = false;
                        eprintln!("    Rule failed: {:?}", res.rule);
                        eprintln!("       {}", res.detail);
                    }
                }
                if all_passed {
                    eprintln!("    Audit passed.");
                } else {
                    eprintln!("    Audit found violations.");
                }
            } else {
                eprintln!("    Could not run otool to audit binary.");
            }
            eprintln!("[KeuOS] Binary synthesized: {:?}", output_bin);
            eprintln!("    Pipeline: mlir-opt -> mlir-translate -> llc (x19 reserved) -> clang (-nostdlib)");
        }
        Err(e) => {
            eprintln!("[E005] Binary synthesis failed: {}", e);
            eprintln!("    Ensure LLVM tools are installed at /opt/homebrew/opt/llvm/bin/");
            eprintln!("    Ensure keuos_rt.o is built (cd keuos_rt && make)");
            std::process::exit(1);
        }
    }
}

pub(crate) fn handle_object_synthesis(mlir: &str, basename: &str, config: &CliConfig) {
    let output_obj = config.output_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("{}.o", basename)));

    let build_dir = std::env::temp_dir().join("salt-build");
    let mut driver = crate::driver::SaltDriver::new(build_dir)
        .with_debug_info(config.debug_info);
    if let Some(ref t) = config.target_name {
        let t_parsed = crate::driver::DriverTarget::from_str(t)
            .unwrap_or_else(|e| {
                eprintln!("[E006] {}", e);
                std::process::exit(1);
            });
        driver = driver.with_target(t_parsed);
    }

    eprintln!("[Object] Compiling to .o...");

    match driver.compile_object(mlir, basename) {
        Ok(produced_path) => {
            if produced_path != output_obj {
                if let Err(e) = std::fs::copy(&produced_path, &output_obj) {
                    eprintln!("[E006] Failed to copy object to {:?}: {}", output_obj, e);
                    std::process::exit(1);
                }
            }
            eprintln!("Object file: {:?}", output_obj);
        }
        Err(e) => {
            eprintln!("[E006] Object compilation failed: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn explain_error_code(code: &str) {
    match code {
        "E001" => println!("\
[E001] File I/O Error
  The compiler could not read or write a file. This usually means:
  - The source file does not exist at the specified path
  - The output directory is not writable
  - The file is not valid UTF-8 text

  Example: `saltc nonexistent.salt -o out.mlir`"),
        "E002" => println!("\
[E002] Syntax Error
  The source code could not be parsed. Check for:
  - Missing semicolons, braces, or parentheses
  - Invalid Salt syntax

  Example: a missing closing brace or an unclosed string literal."),
        "E003" => println!("\
[E003] Compilation Error
  The compiler could not generate valid MLIR from the source code.
  This can be caused by type errors, unresolved symbols, or verification failures.

  Example: Z3 contract violation like calling safe_div(100, 0) with requires(b != 0)."),
        "E004" => println!("\
[E004] CLI Usage Error
  An invalid flag or argument was provided on the command line.
  Run `saltc --help` for a full list of options.

  Example: `saltc --invalid-flag source.salt` or missing output path."),
        "E005" => println!("\
[E005] Binary Synthesis Error
  The MLIR-to-native-binary pipeline failed. This usually means:
  - LLVM tools (mlir-opt, mlir-translate, llc) are not installed
  - The target triple is not supported
  - A linker or runtime object is missing

  Example: running `saltc --target keuos` without the KeuOS runtime toolchain."),
        "E006" => println!("\
[E006] Object Compilation Error
  The MLIR-to-object-file pipeline failed.
  Check that LLVM toolchain is correctly installed.

  Example: missing LLVM tools (llc, mlir-translate) in PATH."),
        "E007" => println!("\
[E007] Internal Compiler Error
  This is a bug in the Salt compiler. Please report it at:
  https://github.com/kevin/salt/issues

  Please report this bug with the source file and the exact saltc command."),
        "E008" => println!("\
[E008] Import / Module Error
  An imported module could not be found or parsed.

  Example: importing a module that does not exist or has a misspelled type name."),
        "E009" => println!("\
[E009] Verification Error
  A Z3 contract or ownership verification check failed.

  Example: a Z3 contract violation such as dividing by zero without a precondition."),
        "E010" => println!("\
[E010] Target Triple Error
  The specified target triple is not recognized or supported.
  Supported targets: macos, linux-arm64, keuos, keuos-x86_64

  Example: `saltc --target unsupported-target source.salt`."),
        _ => println!("Unknown error code: {}\n\nUse `saltc --explain E001` through `saltc --explain E010` for known codes.", code),
    }
}
