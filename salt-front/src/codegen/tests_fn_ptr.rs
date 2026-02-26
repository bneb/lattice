//! TDD Tests for First-Class Function Pointer Types
//!
//! Tests the full pipeline for `fn(T1, T2) -> R` types:
//!
//!   Layer 1 (Parser):   `fn(u64, u64) -> u64` type syntax acceptance
//!   Layer 2 (Type):     SynType -> Type::Fn bridge
//!   Layer 3 (Codegen):  func.constant for fn ptr assignment
//!   Layer 4 (Codegen):  func.call_indirect for indirect calls
//!   Layer 5 (Intrinsic): fn_addr() -> llvm.ptrtoint
//!   Layer 6 (Negative): Invalid uses must fail
//!   Layer 7 (Integration): Assign + call + addr composition
//!
//! TDD: all tests written BEFORE implementation.

#[cfg(test)]
mod tests {
    use crate::grammar::{SaltFile, SynType};
    use crate::codegen::context::CodegenContext;

    /// Helper: compile a Salt source string and return the MLIR output.
    fn compile_to_mlir(source: &str) -> String {
        let file: SaltFile = syn::parse_str(source)
            .unwrap_or_else(|e| panic!("Failed to parse Salt source: {}", e));
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        ctx.drive_codegen()
            .unwrap_or_else(|e| panic!("Codegen failed: {}", e))
    }

    /// Helper: compile and return Err(String) if codegen fails (for negative tests).
    fn try_compile(source: &str) -> Result<String, String> {
        let file: SaltFile = syn::parse_str(source)
            .unwrap_or_else(|e| panic!("Failed to parse Salt source: {}", e));
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        ctx.drive_codegen()
    }

    // =========================================================================
    // LAYER 1: Parser — fn(T1, T2) -> R Type Syntax
    // =========================================================================

    /// The parser must accept `fn(u64, u64) -> u64` as a type in let bindings.
    /// This is the entry point: if you can't parse it, nothing else works.
    #[test]
    fn test_parser_accepts_fn_ptr_type_in_let() {
        let source = r#"
            package main
            fn add(a: u64, b: u64) -> u64 {
                return a + b;
            }
            fn main() -> i32 {
                let f: fn(u64, u64) -> u64 = add;
                return 0;
            }
        "#;
        let result = syn::parse_str::<SaltFile>(source);
        assert!(
            result.is_ok(),
            "Parser must accept fn(u64, u64) -> u64 as a type, got: {:?}",
            result.err()
        );
    }

    /// The parser must accept `fn(u64) -> u64` (single argument).
    #[test]
    fn test_parser_accepts_fn_ptr_single_arg() {
        let source = r#"
            package main
            fn inc(x: u64) -> u64 {
                return x + 1 as u64;
            }
            fn main() -> i32 {
                let f: fn(u64) -> u64 = inc;
                return 0;
            }
        "#;
        let result = syn::parse_str::<SaltFile>(source);
        assert!(
            result.is_ok(),
            "Parser must accept fn(u64) -> u64, got: {:?}",
            result.err()
        );
    }

    /// The parser must accept `fn()` as a void-returning, void-taking function pointer.
    #[test]
    fn test_parser_accepts_fn_ptr_no_args_no_return() {
        let source = r#"
            package main
            fn noop() {
                return;
            }
            fn main() -> i32 {
                let f: fn() = noop;
                return 0;
            }
        "#;
        let result = syn::parse_str::<SaltFile>(source);
        assert!(
            result.is_ok(),
            "Parser must accept fn() as a type, got: {:?}",
            result.err()
        );
    }

    /// The parser must accept fn ptr types in struct fields.
    /// This is critical for SIP dispatch tables.
    #[test]
    fn test_parser_accepts_fn_ptr_in_struct_field() {
        let source = r#"
            package main
            struct DispatchEntry {
                handler: fn(u64, u64) -> u64,
            }
            fn main() -> i32 {
                return 0;
            }
        "#;
        let result = syn::parse_str::<SaltFile>(source);
        assert!(
            result.is_ok(),
            "Parser must accept fn ptr types in struct fields, got: {:?}",
            result.err()
        );
    }

    /// The parser must accept fn ptr types in function arguments.
    #[test]
    fn test_parser_accepts_fn_ptr_as_argument() {
        let source = r#"
            package main
            fn apply(f: fn(u64) -> u64, x: u64) -> u64 {
                return f(x);
            }
            fn main() -> i32 {
                return 0;
            }
        "#;
        let result = syn::parse_str::<SaltFile>(source);
        assert!(
            result.is_ok(),
            "Parser must accept fn ptr types as function arguments, got: {:?}",
            result.err()
        );
    }

    // =========================================================================
    // LAYER 2: Type Bridge — SynType -> Type::Fn
    // =========================================================================

    /// Type::Fn must correctly represent fn(u64, u64) -> u64.
    #[test]
    fn test_type_fn_has_correct_structure() {
        use crate::types::Type;
        let fn_type = Type::Fn(
            vec![Type::U64, Type::U64],
            Box::new(Type::U64),
        );

        // Fn type is pointer-sized (8 bytes on x86-64)
        assert!(fn_type.k_is_ptr_type(), "Type::Fn must be a pointer type");
        assert_eq!(
            fn_type.to_mlir_type_simple(), "!llvm.ptr",
            "Type::Fn must map to !llvm.ptr in MLIR"
        );
    }

    /// Type::Fn must mangle uniquely for monomorphization.
    #[test]
    fn test_type_fn_mangle_suffix() {
        use crate::types::Type;
        let fn_type = Type::Fn(
            vec![Type::U64, Type::U64],
            Box::new(Type::U64),
        );
        let suffix = fn_type.mangle_suffix();
        assert!(
            suffix.contains("Fn") && suffix.contains("u64"),
            "Fn type mangle suffix must contain Fn and arg types, got: {}",
            suffix
        );
    }

    /// Type::from_syn must correctly parse fn(u64) -> u64 from SynType.
    /// This test verifies the SynType::FnPtr -> Type::Fn bridge.
    #[test]
    fn test_type_from_syn_fn_ptr() {
        use crate::types::Type;
        // Construct SynType::FnPtr manually to test the bridge
        // (This will fail until SynType::FnPtr variant is added)
        let syn_type_str = "fn(u64) -> u64";
        // Parse "fn(u64) -> u64" as a syn::Type first, then convert
        let std_ty: syn::Type = syn::parse_str(syn_type_str)
            .expect("syn must parse fn(u64) -> u64");
        let syn_ty = SynType::from_std(std_ty)
            .expect("SynType::from_std must handle bare fn types");

        let ty = Type::from_syn(&syn_ty);
        assert!(
            ty.is_some(),
            "Type::from_syn must produce Some for fn ptr type"
        );
        let ty = ty.unwrap();
        match ty {
            Type::Fn(args, ret) => {
                assert_eq!(args.len(), 1, "fn(u64) -> u64 must have 1 arg");
                assert_eq!(args[0], Type::U64, "Arg must be U64");
                assert_eq!(*ret, Type::U64, "Return must be U64");
            }
            other => panic!("Expected Type::Fn, got {:?}", other),
        }
    }

    // =========================================================================
    // LAYER 3: Codegen — Function Pointer Assignment
    // =========================================================================

    /// `let f: fn(u64) -> u64 = some_fn` must emit func.constant.
    /// func.constant takes a function symbol and produces a function pointer value.
    #[test]
    fn test_fn_ptr_assignment_emits_func_constant() {
        let mlir = compile_to_mlir(r#"
            package main
            fn add_one(x: u64) -> u64 {
                return x + 1 as u64;
            }
            fn main() -> i32 {
                let f: fn(u64) -> u64 = add_one;
                return 0;
            }
        "#);

        assert!(
            mlir.contains("func.constant @") || mlir.contains("constant @"),
            "fn ptr assignment must emit func.constant, got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // LAYER 4: Codegen — Indirect Call Through Function Pointer
    // =========================================================================

    /// Calling through a function pointer variable must emit `llvm.call %ptr(args)`.
    /// The codegen uses LLVM's indirect call via a loaded !llvm.ptr value.
    /// This is the core of SIP dispatch — the 5-cycle IPC target.
    #[test]
    fn test_fn_ptr_call_emits_indirect_llvm_call() {
        let mlir = compile_to_mlir(r#"
            package main
            fn add_one(x: u64) -> u64 {
                return x + 1 as u64;
            }
            fn main() -> i32 {
                let f: fn(u64) -> u64 = add_one;
                let result = f(42 as u64);
                return 0;
            }
        "#);

        // Salt emits `llvm.call %loaded_ptr(args) : !llvm.ptr, (argtys) -> retty`
        // for indirect calls through fn pointer variables
        assert!(
            mlir.contains("llvm.call %"),
            "fn ptr call must emit llvm.call with a value operand (indirect call), got:\n{}",
            mlir
        );
    }

    /// The indirect call pattern must include !llvm.ptr as the callee type.
    /// This distinguishes indirect llvm.call %ptr from direct func.call @name.
    #[test]
    fn test_fn_ptr_call_uses_llvm_ptr_callee_type() {
        let mlir = compile_to_mlir(r#"
            package main
            fn add_one(x: u64) -> u64 {
                return x + 1 as u64;
            }
            fn main() -> i32 {
                let f: fn(u64) -> u64 = add_one;
                let result = f(42 as u64);
                return 0;
            }
        "#);

        // The indirect call must specify !llvm.ptr as the callee type
        assert!(
            mlir.contains(": !llvm.ptr,"),
            "Indirect call must annotate callee as !llvm.ptr, got:\n{}",
            mlir
        );
    }

    /// Indirect call must produce the correct return type.
    #[test]
    fn test_fn_ptr_call_has_correct_return_type() {
        let mlir = compile_to_mlir(r#"
            package main
            fn square(x: u64) -> u64 {
                return x * x;
            }
            fn main() -> i32 {
                let f: fn(u64) -> u64 = square;
                let result = f(7 as u64);
                return 0;
            }
        "#);

        // The call_indirect should have -> i64 (u64 maps to i64 in MLIR)
        assert!(
            mlir.contains("-> i64"),
            "Indirect call must return the correct type (i64), got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // LAYER 5: Intrinsic — fn_addr()
    // =========================================================================

    /// fn_addr(f) must convert a function pointer to its raw u64 address.
    /// This is needed for ELF symbol tables and IDT vector setup.
    #[test]
    fn test_fn_addr_emits_ptrtoint() {
        let mlir = compile_to_mlir(r#"
            package main
            fn handler(a: u64, b: u64) -> u64 {
                return a + b;
            }
            fn main() -> i32 {
                let f: fn(u64, u64) -> u64 = handler;
                let addr = fn_addr(f);
                return 0;
            }
        "#);

        assert!(
            mlir.contains("llvm.ptrtoint") || mlir.contains("ptrtoint"),
            "fn_addr must emit ptrtoint to convert fn ptr to u64, got:\n{}",
            mlir
        );
    }

    /// fn_addr result must be i64 (u64 → i64 in MLIR).
    #[test]
    fn test_fn_addr_returns_i64() {
        let mlir = compile_to_mlir(r#"
            package main
            fn noop() {
                return;
            }
            fn main() -> i32 {
                let f: fn() = noop;
                let addr = fn_addr(f);
                return 0;
            }
        "#);

        assert!(
            mlir.contains("-> i64") || mlir.contains("i64"),
            "fn_addr must return i64, got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // LAYER 6: Negative Tests
    // =========================================================================

    /// fn_addr() with no arguments must fail.
    #[test]
    fn test_fn_addr_no_args_fails() {
        let result = try_compile(r#"
            package main
            fn main() -> i32 {
                let addr = fn_addr();
                return 0;
            }
        "#);

        assert!(
            result.is_err(),
            "fn_addr() with no args must fail compilation"
        );
    }

    // =========================================================================
    // LAYER 7: Integration — SIP Dispatch Table Pattern
    // =========================================================================

    /// A realistic SIP dispatch table: array of function pointers, indexed call.
    /// This is the load-bearing pattern for Mode B IPC.
    #[test]
    fn test_sip_dispatch_table_pattern() {
        let mlir = compile_to_mlir(r#"
            package main
            fn sip_add(a: u64, b: u64) -> u64 {
                return a + b;
            }
            fn sip_sub(a: u64, b: u64) -> u64 {
                return a - b;
            }
            fn main() -> i32 {
                let entry: fn(u64, u64) -> u64 = sip_add;
                let result = entry(1 as u64, 2 as u64);
                let addr: u64 = fn_addr(entry);
                return 0;
            }
        "#);

        // All three patterns must compose:
        // 1. Function pointer assignment
        assert!(
            mlir.contains("constant @") || mlir.contains("func.constant"),
            "Dispatch table must contain func.constant for assignment"
        );
        // 2. Indirect call (llvm.call %ptr pattern)
        assert!(
            mlir.contains("llvm.call %"),
            "Dispatch table must contain llvm.call indirect for dispatch"
        );
        // 3. Address extraction
        assert!(
            mlir.contains("ptrtoint"),
            "Dispatch table must contain ptrtoint for addr extraction"
        );
    }

    /// Higher-order function: pass a function pointer as an argument.
    #[test]
    fn test_higher_order_function_call() {
        let mlir = compile_to_mlir(r#"
            package main
            fn double(x: u64) -> u64 {
                return x * 2 as u64;
            }
            fn apply(f: fn(u64) -> u64, x: u64) -> u64 {
                return f(x);
            }
            fn main() -> i32 {
                let result = apply(double, 21 as u64);
                return 0;
            }
        "#);

        // The apply function must emit llvm.call with a %value operand for f(x)
        assert!(
            mlir.contains("llvm.call %"),
            "Higher-order function must use llvm.call indirect, got:\n{}",
            mlir
        );
    }
}
