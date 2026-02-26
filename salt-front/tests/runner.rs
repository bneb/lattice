use salt_front::fuzz_ast::FuzzSaltFile;
use arbitrary::{Arbitrary, Unstructured};

#[test]
#[ignore = "Integration test requires external MLIR toolchain (mlir-translate, clang)"]
fn test_compiler_cases() {
    let cases_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases");
    let mut files: Vec<_> = std::fs::read_dir(cases_dir)
        .unwrap()
        .map(|r| r.unwrap().path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "salt"))
        .collect();
    files.sort();

    for file in files {
        let name = file.file_name().unwrap().to_str().unwrap();
        // Skip hidden files or known non-test files
        if name.starts_with(".") { continue; }

        println!("Running Test Case: {}", name);
        let code = std::fs::read_to_string(&file).unwrap();
        
        // Parse Expected Output
        let mut expected_output = String::new();
        for line in code.lines() {
            if let Some(rest) = line.trim().strip_prefix("// OUTPUT:") {
                expected_output.push_str(rest.trim());
                expected_output.push('\n');
            }
        }

        let result = salt_front::compile(&code, false, None, true, false);
        
        if name.starts_with("fail_") {
             if result.is_ok() {
                 panic!("Expected compilation failure for case {}, but it succeeded.", name);
             }
             continue;
        }

        match result {
            Ok(mlir) => {
                 // 1. Validate with existing salt-opt (if available)
                let root_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
                // Adjust path as needed for user environment
                let _salt_opt = root_dir.join("salt/build/salt-opt");
                
                // 2. Link & Run Pipeline
                // We need mlir-translate and clang
                // We'll write MLIR to a temp file
                use std::process::Command;

                // Simple temp file mechanism
                let temp_mlir = std::env::temp_dir().join(format!("test_{}.mlir", name));
                let temp_ll = std::env::temp_dir().join(format!("test_{}.ll", name));
                let temp_exe = std::env::temp_dir().join(format!("test_{}.exe", name));
                
                std::fs::write(&temp_mlir, &mlir).expect("Failed to write temp MLIR");

                // A. mlir-translate
                let translate_out = Command::new("mlir-translate")
                    .arg("--mlir-to-llvmir")
                    .arg(&temp_mlir)
                    .output();
                
                match translate_out {
                    Ok(out) => {
                        if !out.status.success() {
                            panic!("mlir-translate failed for {}:\n{}\nMLIR:\n{}", name, String::from_utf8_lossy(&out.stderr), mlir);
                        }
                        std::fs::write(&temp_ll, &out.stdout).expect("Failed to write LLVM IR");
                    },
                    Err(e) => {
                        println!("Skipping execution test for {} (mlir-translate not found): {}", name, e);
                        continue;
                    }
                }

                // B. Clang
                // We link with -Wno-override-module to suppress target triple warnings
                let clang_out = Command::new("clang")
                    .arg("-x").arg("ir")
                    .arg(&temp_ll)
                    .arg("-Wno-override-module")
                    .arg("-o").arg(&temp_exe)
                    .output();

                match clang_out {
                    Ok(out) => {
                         if !out.status.success() {
                            panic!("clang failed for {}:\n{}", name, String::from_utf8_lossy(&out.stderr));
                        }
                    },
                     Err(e) => {
                        println!("Skipping execution test for {} (clang not found): {}", name, e);
                        continue;
                    }
                }

                // C. Execute
                let run_out = Command::new(&temp_exe).output();
                 match run_out {
                     Ok(out) => {
                         if !out.status.success() {
                             panic!("Runtime failure for case {}:\nStderr: {}", name, String::from_utf8_lossy(&out.stderr));
                         }
                         
                         let raw_stdout = String::from_utf8_lossy(&out.stdout);
                         // Normalize output (trim whitespace)
                         let actual = raw_stdout.trim();
                         let expected = expected_output.trim();
                         
                         if !expected.is_empty() && actual != expected {
                             panic!("Assertion Failed for {}.\nExpected:\n---\n{}\n---\nActual:\n---\n{}\n---", name, expected, actual);
                         } else if expected.is_empty() {
                             // If no OUTPUT tag, we just ensure it didn't crash (checked by status.success)
                         }
                     },
                      Err(e) => panic!("Failed to run executable for {}: {}", name, e)
                 }

            }
            Err(e) => {
                panic!("Failed to compile case {}: {:?}", name, e);
            }
        }
    }
}

#[test]
fn test_fuzz_smoke() {
    // Run multiple iterations to cover more AST variants
    for i in 0..50 {
        // Deterministic seed based on iteration
        let mut data = vec![0u8; 8192];
        for (j, item) in data.iter_mut().enumerate() {
            *item = (j.wrapping_mul(33).wrapping_add(i)) as u8;
        }

        let mut u = Unstructured::new(&data);
        
        // Attempt to generate AST. If we run out of bytes, that's fine, we just want to exercise the code.
        if let Ok(fuzz_file) = FuzzSaltFile::arbitrary(&mut u) {
            let mut salt_file = fuzz_file.to_salt();
            // We expect this might fail compilation due to semantic rules (e.g. variable usage),
            // but it should NOT panic.
            let result = salt_front::compile_ast(&mut salt_file, false, None, true, false, false, false, false, false, false, "<test>");
             if result.is_ok() {} // valid AST
             // invalid semantics, still covered parsing/codegen paths
        }
    }
}
