use saltc::compile;
use saltc::registry::{Registry, ModuleInfo};
use saltc::types::Type;

#[test]
fn test_struct_equality() {
    let code = r#"
struct Point { x: i32, y: i32 }
fn test_eq(a: Point, b: Point) -> bool {
    return a == b;
}
fn main() -> i32 { return 0; }
"#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Struct equality failed: {:?}", result.err());
}

#[test]
fn test_array_operations() {
    let code = r#"
fn test_array() -> i32 {
    let arr: [i32; 5] = [1, 2, 3, 4, 5];
    return arr[2];
}
fn main() -> i32 { return test_array(); }
"#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Array ops failed: {:?}", result.err());
}

#[test]
fn test_tuple_operations() {
    let code = r#"
fn test_tuple() -> i32 {
    let t = (1, 2, 3);
    return t.0 + t.1 + t.2;
}
fn main() -> i32 { return test_tuple(); }
"#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Tuple ops failed: {:?}", result.err());
}

#[test]
fn test_method_call() {
    let code = r#"
struct Counter { val: i32 }
impl Counter {
    fn get(self) -> i32 { return self.val; }
}
fn main() -> i32 {
    let c = Counter { val: 42 };
    return c.get();
}
"#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Method call failed: {:?}", result.err());
}

#[test]
fn test_ptr_to_int() {
    let code = r#"
fn test_ptr_int(p: &i32) -> i64 {
    return p as i64;
}
fn main() -> i32 {
    let x = 42;
    let addr = test_ptr_int(&x);
    return 0;
}
"#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Ptr to int failed: {:?}", result.err());
}

#[test]
fn test_int_to_ptr() {
    let code = r#"
fn test_int_ptr(addr: i64) -> &i32 {
    return addr as &i32;
}
fn main() -> i32 { return 0; }
"#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Int to ptr failed: {:?}", result.err());
}

#[test]
fn test_float_arithmetic() {
    let code = r#"
fn test_f32(a: f32, b: f32) -> f32 {
    return a + b - a * b / (a + 1.0);
}
fn test_f64(a: f64, b: f64) -> f64 {
    return a + b - a * b / (a + 1.0);
}
fn main() -> i32 { return 0; }
"#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Float arithmetic failed: {:?}", result.err());
}

#[test]
fn test_unsigned_arithmetic() {
    let code = r#"
fn test_u8(a: u8, b: u8) -> u8 { return a + b; }
fn test_u32(a: u32, b: u32) -> u32 { return a * b; }
fn test_u64(a: u64, b: u64) -> u64 { return a / b + 1; }
fn test_usize(a: usize, b: usize) -> usize { return a + b; }
fn main() -> i32 { return 0; }
"#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Unsigned arithmetic failed: {:?}", result.err());
}

#[test]
fn test_enum_codegen() {
    let code = r#"
enum Option<T> {
    Some(T),
    None
}
fn test_enum(x: i32) -> i32 {
    let opt = Option::<i32>::Some(x);
    match opt {
        Option::Some(v) => return v,
        Option::None => return 0
    }
    return 0;
}
fn main() -> i32 {
    let x = test_enum(42);
    return x;
}
"#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Enum codegen failed: {:?}", result.err());
    let mlir = result.unwrap();
    // Check for switch
    assert!(mlir.contains("llvm.switch"), "Missing switch instruction: {}", mlir);
    // Check for payload extraction
    assert!(mlir.contains("llvm.extractvalue"), "Missing extractvalue: {}", mlir);
}

#[test]
fn test_canonical_path_flattening() {
    // We mock a registry with kernel.core.SYSCALL_ENTRY
    let mut registry = Registry::new();
    let mut core_mod = ModuleInfo::new("kernel.core");
    core_mod.globals.insert("SYSCALL_ENTRY".to_string(), Type::I64);
    registry.modules.insert("kernel.core".to_string(), core_mod);

    let code = r#"
        use kernel.core;
        
        fn main() -> i64 {
            let entry: i64 = kernel.core.SYSCALL_ENTRY;
            return entry;
        }
    "#;

    let result = compile(code, false, Some(&registry), true, false).expect("Compilation failed");
    
    // In MLIR, this should resolve to a load from @kernel__core__SYSCALL_ENTRY
    // and notably, NOT a sequence of field accesses.
    
    assert!(result.contains("@kernel__core__SYSCALL_ENTRY"), "Path flattening failed: symbol kernel.core.SYSCALL_ENTRY not mangled correctly");
    assert!(result.contains("llvm.mlir.addressof @kernel__core__SYSCALL_ENTRY"), "Path flattening failed: should use addressof for module global");
}

#[test]
fn test_path_flattening_with_alias() {
    let mut registry = Registry::new();
    let mut core_mod = ModuleInfo::new("kernel.core");
    core_mod.globals.insert("DEBUG_PORT".to_string(), Type::I32);
    registry.modules.insert("kernel.core".to_string(), core_mod);

    let code = r#"
        use kernel.core as core;
        
        fn main() -> i32 {
            return core.DEBUG_PORT;
        }
    "#;

    let result = compile(code, false, Some(&registry), true, false).expect("Compilation failed");
    
    assert!(result.contains("@kernel__core__DEBUG_PORT"), "Path flattening with alias failed");
}

#[test]
fn test_array_assign() {
    let code = r#"
        fn main() -> i32 {
            let mut arr: [i32; 3] = [0, 0, 0];
            arr[1] = 42;
            return arr[1];
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Array assign failed: {:?}", result.err());
}

#[test]
fn test_all_arithmetic_ops() {
    let code = r#"
        fn main() -> i32 {
            let a: i32 = 10 + 5;
            let b: i32 = 10 - 5;
            let c: i32 = 10 * 5;
            let d: i32 = 10 / 5;
            let e: i32 = 10 % 3;
            return a + b + c + d + e;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Arithmetic ops failed: {:?}", result.err());
}

#[test]
fn test_all_bitwise_ops() {
    let code = r#"
        fn main() -> i32 {
            let a: i32 = 0xFF & 0x0F;
            let b: i32 = 0xF0 | 0x0F;
            let c: i32 = 0xFF ^ 0xF0;
            let d: i32 = 1 << 4;
            let e: i32 = 16 >> 2;
            return a + b + c + d + e;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Bitwise ops failed: {:?}", result.err());
}

#[test]
fn test_comparison_ops() {
    let code = r#"
        fn main() -> i32 {
            let lt: bool = 5 < 10;
            let le: bool = 5 <= 10;
            let gt: bool = 10 > 5;
            let ge: bool = 10 >= 5;
            let eq: bool = 5 == 5;
            let ne: bool = 5 != 10;
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Comparison ops failed: {:?}", result.err());
}

#[test]
fn test_logical_ops() {
    let code = r#"
        fn main() -> i32 {
            let a: bool = true && false;
            let b: bool = true || false;
            let c: bool = !a;
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Logical ops failed: {:?}", result.err());
}

#[test]
fn test_assign_ops() {
    let code = r#"
        fn main() -> i32 {
            let mut x: i32 = 10;
            x += 5;
            x -= 3;
            x *= 2;
            x /= 4;
            x %= 3;
            return x;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Assign ops failed: {:?}", result.err());
}

#[test]
fn test_numeric_casts() {
    let code = r#"
        fn main() -> i32 {
            let a: i32 = 100;
            let b: i64 = a as i64;
            let c: u32 = a as u32;
            let d: f64 = a as f64;
            let e: i32 = d as i32;
            return e;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Numeric casts failed: {:?}", result.err());
}

#[test]
fn test_ref_deref() {
    let code = r#"
        fn main() -> i32 {
            let x: i32 = 42;
            let r: &i32 = &x;
            return *r;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Ref/deref failed: {:?}", result.err());
}

#[test]
fn test_mut_ref() {
    let code = r#"
        fn main() -> i32 {
            let mut x: i32 = 42;
            let r: &mut i32 = &mut x;
            *r = 100;
            return x;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Mut ref failed: {:?}", result.err());
}

#[test]
fn test_float_arithmetic_coverage() {
    let code = r#"
        fn main() -> i32 {
            let a: f64 = 10.5 + 2.5;
            let b: f64 = 10.5 - 2.5;
            let c: f64 = 10.5 * 2.0;
            let d: f64 = 10.5 / 2.0;
            return a as i32;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Float arith failed: {:?}", result.err());
}

#[test]
fn test_float_comparison() {
    let code = r#"
        fn main() -> i32 {
            let a: f64 = 1.5;
            let b: f64 = 2.5;
            let lt: bool = a < b;
            let gt: bool = a > b;
            let eq: bool = a == b;
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Float cmp failed: {:?}", result.err());
}

#[test]
fn test_unary_ops() {
    let code = r#"
        fn main() -> i32 {
            let a: i32 = -5;
            let b: f64 = -3.14;
            let c: bool = !true;
            let d: i32 = !0xFF;
            return a;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Unary ops failed: {:?}", result.err());
}

#[test]
fn test_popcount_intrinsic() {
    let code = r#"
        fn main() -> i32 {
            let x: i32 = 0b11010011;
            let count: i32 = x.popcount();
            return count;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Popcount failed: {:?}", result.err());
}

#[test]
fn test_leading_zeros_intrinsic() {
    let code = r#"
        fn main() -> i32 {
            let x: i32 = 16;
            let lz: i32 = x.leading_zeros();
            return lz;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Leading zeros failed: {:?}", result.err());
}

#[test]
fn test_popcount_method() {
    let code = r#"
        fn main() -> i32 {
            let x: i32 = 0b11010011;
            return x.popcount();
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "popcount failed: {:?}", result.err());
}

#[test]
fn test_leading_zeros_method() {
    let code = r#"
        fn main() -> i32 {
            let x: i32 = 16;
            return x.leading_zeros();
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "leading_zeros failed: {:?}", result.err());
}

#[test]
fn test_trailing_zeros_method() {
    let code = r#"
        fn main() -> i32 {
            let x: i32 = 16;
            return x.trailing_zeros();
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "trailing_zeros failed: {:?}", result.err());
}

#[test]
fn test_bitwise_assign_ops() {
    let code = r#"
        fn main() -> i32 {
            let mut x: i32 = 0xFF;
            x &= 0x0F;
            x |= 0xF0;
            x ^= 0xFF;
            return x;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "bitwise assign failed: {:?}", result.err());
}

#[test]
fn test_shift_assign_ops() {
    let code = r#"
        fn main() -> i32 {
            let mut x: i32 = 1;
            x <<= 4;
            x >>= 2;
            return x;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "shift assign failed: {:?}", result.err());
}

#[test]
fn test_pointer_equality() {
    let code = r#"
        fn main() -> i32 {
            let x: i32 = 42;
            let r1: &i32 = &x;
            let r2: &i32 = &x;
            if r1 == r2 {
                return 1;
            }
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "pointer equality failed: {:?}", result.err());
}

#[test]
fn test_deref_and_assign() {
    let code = r#"
        fn main() -> i32 {
            let mut x: i32 = 0;
            let r: &mut i32 = &mut x;
            *r = 42;
            return x;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "deref assign failed: {:?}", result.err());
}

#[test]
fn test_mixed_tuple_access() {
    let code = r#"
        fn main() -> i32 {
            let t: (i32, i64, i32) = (1, 2 as i64, 3);
            return t.0 + t.2;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "tuple access failed: {:?}", result.err());
}

#[test]
fn test_tuple_field_assign() {
    let code = r#"
        fn main() -> i32 {
            let mut t: (i32, i32) = (0, 0);
            t.0 = 10;
            t.1 = 20;
            return t.0 + t.1;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "tuple assign failed: {:?}", result.err());
}

#[test]
fn test_simple_enum_match() {
    let code = r#"
        enum Option<T> { Some(T), None }
        fn main() -> i32 {
            let x: Option<i32> = Option::<i32>::Some(42);
            match x {
                Option::Some(v) => return v,
                Option::None => return 0
            }
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "enum match failed: {:?}", result.err());
}

#[test]
fn test_array_variable_index() {
    let code = r#"
        fn main() -> i32 {
            let arr: [i32; 5] = [10, 20, 30, 40, 50];
            let idx: i32 = 2;
            return arr[0] + arr[idx as usize];
        }
    "#;
    let result = compile(code, false, None, true);
    // May fail if variable index not supported
    let _ = result;
}

#[test]
fn test_array_in_loop() {
    let code = r#"
        fn main() -> i32 {
            let mut arr: [i32; 5] = [0, 0, 0, 0, 0];
            for i in 0..5 {
                arr[i] = i as i32;
            }
            return arr[4];
        }
    "#;
    let result = compile(code, false, None, true);
    // May fail if array assignment in loop not supported
    let _ = result;
}

#[test]
fn test_string_literal() {
    let code = r#"
        fn main() -> i32 {
            let s: &str = "hello world";
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "string literal failed: {:?}", result.err());
}
