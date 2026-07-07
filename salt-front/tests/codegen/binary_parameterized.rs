
fn check_compile(source: &str, should_pass: bool, error_pattern: Option<&str>, mlir_pattern: Option<&str>) -> Option<String> {
    let res = saltc::compile(source, false, None, true);
    if should_pass {
        if let Err(e) = res {
            return Some(format!("Expected success but failed: {}", e));
        }
        let mlir = res.unwrap();
        if let Some(pat) = mlir_pattern {
            if !mlir.contains(pat) {
                return Some(format!("MLIR output missing pattern '{}'", pat));
            }
        }
    } else {
        if res.is_ok() {
            return Some("Expected failure but succeeded".to_string());
        }
        if let Some(pat) = error_pattern {
            let err = res.err().unwrap().to_string();
            if !err.contains(pat) {
                return Some(format!("Error message '{}' missing pattern '{}'", err, pat));
            }
        }
    }
    None
}

#[test]
fn test_arithmetic_ops() {
    // Ops: + - * / %
    // Types: integers, floats
    let ops = vec!["+", "-", "*", "/", "%"];
    let types = vec!["i8", "u8", "i32", "f32"]; 
    
    let mut failures = Vec::new();

    for op in ops {
        for ty in &types {
            let (v1, v2) = if ty.starts_with("f") { ("2.0", "1.0") } else { ("2", "1") };
            
            // Basic valid case: same types
            let source = format!("fn main() {{ let a: {} = {}; let b: {} = {}; let c = a {} b; }}", ty, v1, ty, v2, op);
            
            // % on float is usually invalid or frem. Salt support? 
            // emit_binary uses arith.remui/remsi. remf for floats?
            // emit_binary: 
            // "/" -> divsi/divui/divf
            // "%" -> remsi/remui/remf
            
            let expected_pass = true; // Assumed valid for all? 
            // Warning: % on float might need remf
            
            let mlir_expect = match op {
                "+" => if ty.starts_with("f") { "arith.addf" } else { "arith.addi" },
                "-" => if ty.starts_with("f") { "arith.subf" } else { "arith.subi" },
                "*" => if ty.starts_with("f") { "arith.mulf" } else { "arith.muli" },
                "/" => if ty.starts_with("f") { "arith.divf" } else if ty.starts_with("u") { "arith.divui" } else { "arith.divsi" },
                "%" => if ty.starts_with("f") { "arith.remf" } else if ty.starts_with("u") { "arith.remui" } else { "arith.remsi" },
                _ => ""
            };

            if let Some(err) = check_compile(&source, expected_pass, None, Some(mlir_expect)) {
                failures.push(format!("Op {} Type {}: {}", op, ty, err));
            }
        }
    }
    
    // Mixed types (should strictly fail or implicit promote?)
    // Salt currently does SOME promotion but it's complex.
    // Let's test a known failure: bool + i32
    let source = "fn main() { let a: bool = true; let b: i32 = 1; let c = a + b; }";
    if let Some(err) = check_compile(source, false, Some("Arithmetic operator requires numeric"), None) {
       failures.push(format!("Mixed bool + i32: {}", err));
    }

    if !failures.is_empty() {
        panic!("Arithmetic Failures:\n{}", failures.join("\n"));
    }
}

#[test]
fn test_bitwise_ops() {
    // Ops: & | ^ << >>
    let ops = vec!["&", "|", "^", "<<", ">>"];
    let types = vec!["i32", "u32", "f32"]; // integers ok, float fail

    let mut failures = Vec::new();
    
    for op in ops {
        for ty in &types {
            let is_float = ty.starts_with("f");
            let (v1, v2) = if is_float { ("2.0", "1.0") } else { ("2", "1") };
            let source = format!("fn main() {{ let a: {} = {}; let b: {} = {}; let c = a {} b; }}", ty, v1, ty, v2, op);
            
            let should_pass = !is_float;
            
            let mlir_expect = if should_pass {
               match op {
                   "&" => "arith.andi",
                   "|" => "arith.ori",
                   "^" => "arith.xori",
                   "<<" => "arith.shli",
                   ">>" => if ty.starts_with("u") { "arith.shrui" } else { "arith.shrsi" },
                   _ => ""
               }
            } else { "" };
            
            if let Some(err) = check_compile(&source, should_pass, if !should_pass { Some("integer operands") } else { None }, if should_pass { Some(mlir_expect) } else { None }) {
                 // For floats, error output might vary.
                 // Just check pass/fail
                 if is_float {
                      if check_compile(&source, false, None, None).is_some() {
                           failures.push(format!("Float {} {} passed but should fail", ty, op));
                      }
                 } else {
                     failures.push(format!("Op {} Type {}: {}", op, ty, err));
                 }
            }
        }
    }
    
    if !failures.is_empty() {
        panic!("Bitwise Failures:\n{}", failures.join("\n"));
    }
}
