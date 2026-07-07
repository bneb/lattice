// Tests for MLIR Builder Pattern Helpers in CodegenContext
#![allow(clippy::approx_constant)]

use saltc::codegen::context::CodegenContext;
use saltc::grammar::SaltFile;
use saltc::types::Type;

// Helper macro for tests - injects Z3 context properly with correct lifetime
macro_rules! test_ctx {
    ($name:ident, $test:block) => {
        #[test]
        fn $name() {
            let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap();
            let z3_cfg = z3::Config::new();
            let z3_ctx = z3::Context::new(&z3_cfg);
            let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
            let mut out = String::new();
            $test
        }
    };
}

// =============================================================================
// emit_binop tests
// =============================================================================

#[test]
fn test_emit_binop_addi() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_binop(&mut out, "%result", "arith.addi", "%a", "%b", "i32");
    
    assert_eq!(out, "    %result = arith.addi %a, %b : i32\n");
}

#[test]
fn test_emit_binop_muli() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_binop(&mut out, "%prod", "arith.muli", "%x", "%y", "i64");
    
    assert_eq!(out, "    %prod = arith.muli %x, %y : i64\n");
}

#[test]
fn test_emit_binop_addf() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_binop(&mut out, "%sum", "arith.addf", "%f1", "%f2", "f64");
    
    assert_eq!(out, "    %sum = arith.addf %f1, %f2 : f64\n");
}

// =============================================================================
// emit_const tests
// =============================================================================

#[test]
fn test_emit_const_int() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_const_int(&mut out, "%c42", 42, "i32");
    
    assert_eq!(out, "    %c42 = arith.constant 42 : i32\n");
}

#[test]
fn test_emit_const_int_negative() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_const_int(&mut out, "%neg", -100, "i64");
    
    assert_eq!(out, "    %neg = arith.constant -100 : i64\n");
}

#[test]
fn test_emit_const_float() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_const_float(&mut out, "%pi", 3.14159, "f64");
    
    // Uses scientific notation
    assert!(out.contains("%pi = arith.constant"));
    assert!(out.contains("f64"));
}

// =============================================================================
// emit_load and emit_store tests
// =============================================================================

#[test]
fn test_emit_load() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_load(&mut out, "%val", "%ptr", "i32");
    
    assert_eq!(out, "    %val = llvm.load %ptr : !llvm.ptr -> i32\n");
}

#[test]
fn test_emit_store() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_store(&mut out, "%val", "%ptr", "i32");
    
    assert_eq!(out, "    llvm.store %val, %ptr : i32, !llvm.ptr\n");
}

// =============================================================================
// emit_alloca tests
// =============================================================================

#[test]
fn test_emit_alloca() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_alloca(&mut out, "%stack", "i32");
    
    assert!(ctx.alloca_out().contains("%stack = llvm.alloca %c1_i64 x i32 : (i64) -> !llvm.ptr"));
}

// =============================================================================
// emit_gep_field tests
// =============================================================================

#[test]
fn test_emit_gep_field() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_gep_field(&mut out, "%field_ptr", "%struct_ptr", 2, "!llvm.struct<\"Point\", (i32, i32, i32)>");
    
    assert!(out.contains("%field_ptr = llvm.getelementptr %struct_ptr[0, 2]"));
}

// =============================================================================
// emit_extractvalue and emit_insertvalue tests
// =============================================================================

#[test]
fn test_emit_extractvalue() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_extractvalue(&mut out, "%elem", "%tuple", 1, "!llvm.struct<(i32, i64)>");
    
    assert_eq!(out, "    %elem = llvm.extractvalue %tuple[1] : !llvm.struct<(i32, i64)>\n");
}

#[test]
fn test_emit_insertvalue() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_insertvalue(&mut out, "%new_tuple", "%val", "%tuple", 0, "!llvm.struct<(i32, i32)>");
    
    assert_eq!(out, "    %new_tuple = llvm.insertvalue %val, %tuple[0] : !llvm.struct<(i32, i32)>\n");
}

// =============================================================================
// emit_cmp tests
// =============================================================================

#[test]
fn test_emit_cmp_int() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_cmp(&mut out, "%cmp", "arith.cmpi", "slt", "%a", "%b", "i32");
    
    assert_eq!(out, "    %cmp = arith.cmpi \"slt\", %a, %b : i32\n");
}

#[test]
fn test_emit_cmp_float() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_cmp(&mut out, "%fcmp", "arith.cmpf", "olt", "%f1", "%f2", "f64");
    
    assert_eq!(out, "    %fcmp = arith.cmpf \"olt\", %f1, %f2 : f64\n");
}

// =============================================================================
// emit_cast and emit_trunc tests
// =============================================================================

#[test]
fn test_emit_cast_extsi() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_cast(&mut out, "%ext", "arith.extsi", "%val", "i32", "i64");
    
    assert_eq!(out, "    %ext = arith.extsi %val : i32 to i64\n");
}

#[test]
fn test_emit_trunc() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_trunc(&mut out, "%small", "%big", "i64", "i32");
    
    assert_eq!(out, "    %small = arith.trunci %big : i64 to i32\n");
}

// =============================================================================
// emit_br and emit_cond_br tests
// =============================================================================

#[test]
fn test_emit_br() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_br(&mut out, "loop_body");
    
    assert_eq!(out, "    llvm.br ^loop_body\n");
}

#[test]
fn test_emit_cond_br() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_cond_br(&mut out, "%cond", "then_block", "else_block");
    
    assert_eq!(out, "    llvm.cond_br %cond, ^then_block, ^else_block\n");
}

// =============================================================================
// emit_label tests
// =============================================================================

#[test]
fn test_emit_label() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_label(&mut out, "entry");
    
    assert_eq!(out, "  ^entry:\n");
}

// =============================================================================
// emit_return tests
// =============================================================================

#[test]
fn test_emit_return() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_return(&mut out, "%result", "i32");
    
    assert_eq!(out, "    llvm.return %result : i32\n");
}

#[test]
fn test_emit_return_void() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_return_void(&mut out);
    
    assert_eq!(out, "    llvm.return\n");
}

#[test]
fn test_emit_load_logical() {
    // We don't need actual files for builder tests, just a valid context
    let mut file: SaltFile = syn::parse_str("fn main() {}").unwrap();
    let z3_cfg = z3::Config::new();
    let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    // Simulate loading a boolean (i8 pointer)
    let res_name = "%result_val";
    ctx.emit_load_logical(&mut out, res_name, "%bool_ptr", &Type::Bool).unwrap();
    
    // Should see a load followed by truncation (arith.trunci for i8 -> i1)
    assert!(out.contains("llvm.load"));
    assert!(out.contains("arith.trunci"));
}

#[test]
fn test_emit_store_logical() {
    let mut file: SaltFile = syn::parse_str("fn main() {}").unwrap();
    let z3_cfg = z3::Config::new();
    let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    
    let mut out = String::new();
    
    // Simulate storing a logical boolean (i1) to storage (i8*)
    ctx.emit_store_logical(&mut out, "%val_i1", "%ptr", &Type::Bool).unwrap();
    
    assert!(out.contains("arith.extui")); // Extension i1 -> i8 (unsigned)
    assert!(out.contains("llvm.store"));
}

#[test]
fn test_emit_addressof_variants() {
    let mut file: SaltFile = syn::parse_str("fn main() {}").unwrap();
    let z3_cfg = z3::Config::new();
    let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    
    let mut out = String::new();
    
    // Test Global Symbol
    let _ = ctx.emit_addressof(&mut out, "%res_global", "MyGlobal");
    assert!(out.contains("llvm.mlir.addressof @MyGlobal"));
    
    // Test Variable Symbol (stack alloc)
    // NOTE: This requires the symbol to exist in `ctx.locals` or `ctx.args`, 
    // which is hard to mock without full setup. 
    // We'll trust the global case covers the instruction emission logic.
}

#[test]
fn test_emit_atomicrmw_ops() {
    let mut file: SaltFile = syn::parse_str("fn main() {}").unwrap();
    let z3_cfg = z3::Config::new();
    let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    
    let ops = vec!["add", "sub", "max", "min", "xchg"];
    
    for op in ops {
        let mut out = String::new();
        ctx.emit_atomicrmw(&mut out, "%res", op, "%ptr", "%val", "i32");
        assert!(out.contains(&format!("llvm.atomicrmw {} %ptr, %val seq_cst : !llvm.ptr, i32", op)));
    }
}

// =============================================================================
// Integration test - building a simple function body
// =============================================================================

#[test]
fn test_emit_sequence() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    // Build: let x = 10 + 20; return x;
    ctx.emit_const_int(&mut out, "%c10", 10, "i32");
    ctx.emit_const_int(&mut out, "%c20", 20, "i32");
    ctx.emit_binop(&mut out, "%sum", "arith.addi", "%c10", "%c20", "i32");
    ctx.emit_return(&mut out, "%sum", "i32");
    
    let expected = "    %c10 = arith.constant 10 : i32\n    %c20 = arith.constant 20 : i32\n    %sum = arith.addi %c10, %c20 : i32\n    llvm.return %sum : i32\n";
    assert_eq!(out, expected);
}

// =============================================================================
// emit_load_exclusive tests (salt.access = "exclusive")
// =============================================================================

#[test]
fn test_emit_load_exclusive() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_load_exclusive(&mut out, "%owned_val", "%owned_ptr", "i32");
    
    assert!(out.contains("%owned_val = \"llvm.load\"(%owned_ptr)"));
    assert!(out.contains("salt.access = \"exclusive\""));
    assert!(out.contains("(!llvm.ptr) -> i32"));
}

// =============================================================================
// emit_load_atomic, emit_store_atomic, emit_atomicrmw tests
// =============================================================================

#[test]
fn test_emit_load_atomic() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_load_atomic(&mut out, "%atomic_val", "%atomic_ptr", "i64");
    
    assert!(out.contains("%atomic_val = llvm.load %atomic_ptr"));
    assert!(out.contains("atomic_memory_order = #llvm.atomic_memory_order<acquire>"));
    assert!(out.contains("!llvm.ptr -> i64"));
}

#[test]
fn test_emit_store_atomic() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_store_atomic(&mut out, "%val", "%atomic_ptr", "i32");
    
    assert!(out.contains("llvm.store %val, %atomic_ptr"));
    assert!(out.contains("atomic_memory_order = #llvm.atomic_memory_order<release>"));
    assert!(out.contains("i32, !llvm.ptr"));
}

#[test]
fn test_emit_atomicrmw() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_atomicrmw(&mut out, "%old_val", "add", "%ptr", "%delta", "i64");
    
    assert!(out.contains("%old_val = llvm.atomicrmw add %ptr, %delta seq_cst"));
    assert!(out.contains(": !llvm.ptr, i64"));
}

// =============================================================================
// emit_call tests
// =============================================================================

#[test]
fn test_emit_call_with_result() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_call(&mut out, Some("%result"), "my_func", "%arg1, %arg2", "i32, i64", "i32");
    
    assert!(out.contains("%result = func.call @my_func(%arg1, %arg2)"));
    assert!(out.contains("(i32, i64) -> i32"));
}

#[test]
fn test_emit_call_void() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_call(&mut out, None, "print_hello", "%msg", "!llvm.ptr", "void");
    
    assert!(out.contains("func.call @print_hello(%msg)"));
    assert!(out.contains("(!llvm.ptr) -> ()"));
}

// =============================================================================
// emit_addressof tests
// =============================================================================

#[test]
fn test_emit_addressof() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    let _ = ctx.emit_addressof(&mut out, "%global_ptr", "my_global");
    
    assert_eq!(out, "    %global_ptr = llvm.mlir.addressof @my_global : !llvm.ptr\n");
}

// =============================================================================
// emit_inttoptr tests
// =============================================================================

#[test]
fn test_emit_inttoptr() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_inttoptr(&mut out, "%ptr", "%addr", "i64");
    
    assert_eq!(out, "    %ptr = llvm.inttoptr %addr : i64 to !llvm.ptr\n");
}

// =============================================================================
// emit_verify tests
// =============================================================================

#[test]
fn test_emit_verify() {
    let mut file: SaltFile = syn::parse_str("fn main() -> i32 { return 0; }").unwrap(); let z3_cfg = z3::Config::new(); let z3_ctx = z3::Context::new(&z3_cfg);
    let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
    let mut out = String::new();
    
    ctx.emit_verify(&mut out, "%cond", "array index in bounds");
    
    assert_eq!(out, "    salt.verify %cond, \"array index in bounds\"\n");
}
