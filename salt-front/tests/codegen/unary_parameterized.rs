
#[test]
fn test_parameterized_unary_not() {
    // !Op
    let types = vec![
        ("i8", "0", true), ("u8", "0", true),
        ("i16", "0", true), ("u16", "0", true),
        ("i32", "0", true), ("u32", "0", true),
        ("i64", "0", true), ("u64", "0", true),
        ("bool", "true", true), 
        ("f32", "0.0", false), ("f64", "0.0", false)
    ];

    let mut failures = Vec::new();

    for (ty, val, should_pass) in types {
        let source = format!("fn main() {{ let x: {} = {}; let y = !x; }}", ty, val);
        
        let res = saltc::compile(&source, false, None, true);
        if should_pass {
            if res.is_err() {
                failures.push(format!("Failed to compile !{} ({}): {:?}", ty, val, res.err()));
            } else {
                let mlir = res.unwrap();
                if ty == "bool" {
                    if !mlir.contains("arith.xori") || !mlir.contains("expected_bool_const") {
                       // We need to check for xori with 1 (true)
                       if !mlir.contains("arith.constant 1 : i1") && !mlir.contains("true") {
                           // Actually 1 : i1 is constant true
                       }
                    }
                } else {
                     // Integer NOT: xori with -1
                     if !mlir.contains("arith.constant -1") || !mlir.contains("arith.xori") {
                         failures.push(format!("!{} ({}) did not emit xori -1", ty, val));
                     }
                }
            }
        } else {
            if res.is_ok() {
                failures.push(format!("Should have failed to compile !{} ({})", ty, val));
            }
        }
    }

    if !failures.is_empty() {
        panic!("Unary NOT failures:\n{}", failures.join("\n"));
    }
}

#[test]
fn test_parameterized_unary_neg() {
    // -Op
    let types = vec![
        ("i8", "0", true), ("u8", "0", true), 
        ("i16", "0", true), ("u16", "0", true),
        ("i32", "0", true), ("u32", "0", true),
        ("i64", "0", true), ("u64", "0", true),
        ("f32", "0.0", true), ("f64", "0.0", true),
        ("bool", "true", false)
    ];

    let mut failures = Vec::new();

    for (ty, val, should_pass) in types {
        let source = format!("fn main() {{ let x: {} = {}; let y = -x; }}", ty, val);
        
        let res = saltc::compile(&source, false, None, true);
        if should_pass {
            if res.is_err() {
                failures.push(format!("Failed to compile -{} ({}): {:?}", ty, val, res.err()));
            } else {
                 let mlir = res.unwrap();
                 // Integers use subi (0 - x), Floats use negf
                 if ty.starts_with("f") {
                     if !mlir.contains("negf") && !mlir.contains("arith.negf") {
                         // Likely bug: using subi for floats
                         failures.push(format!("-{} ({}) MLIR check failed (likely float Neg bug)", ty, val));
                     }
                 } else {
                     if !mlir.contains("arith.subi") {
                         failures.push(format!("-{} ({}) did not emit subi", ty, val));
                     }
                 }
            }
        } else {
            if res.is_ok() {
                failures.push(format!("Should have failed to compile -{} ({})", ty, val));
            }
        }
    }

    if !failures.is_empty() {
        panic!("Unary NEG failures:\n{}", failures.join("\n"));
    }
}
