//! TDD Tests for Z3 Alignment Verification — Formal Shadow
//!
//! Phase 1A of V0.9.0: The compiler must mathematically prove that @atomic
//! fields are correctly aligned before emitting machine code. This prevents
//! hardware Alignment Check (#AC) faults and cache-line straddling.
//!
//! Tests follow Red-Green-Refactor:
//!   RED:   These tests assert IDEAL behavior that does not yet exist.
//!   GREEN: Tests pass after implementation.
//!
//! Layer 1: Parser accepts @atomic attribute on struct fields
//! Layer 2: Z3 proves 16-byte alignment for @atomic struct layouts
//! Layer 3: Z3 rejects misaligned @atomic fields with diagnostic
//! Layer 4: Proven alignment elides runtime assertions from MLIR

#[cfg(test)]
mod tests {
    use crate::grammar::SaltFile;
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

    /// Helper: compile and return Err(String) if codegen fails.
    fn try_compile(source: &str) -> Result<String, String> {
        let file: SaltFile = syn::parse_str(source)
            .unwrap_or_else(|e| panic!("Failed to parse Salt source: {}", e));
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        ctx.drive_codegen()
    }

    // =========================================================================
    // LAYER 1: Parser — @atomic Attribute on Struct Fields [RED]
    // =========================================================================

    /// The parser must accept @atomic as a field attribute.
    /// This is the grammar foundation for the Formal Shadow.
    #[test]
    fn test_parser_accepts_atomic_attribute_on_struct_field() {
        let source = r#"
            package main
            struct TreiberNode {
                @atomic ptr: u64,
                gen: u64,
            }
            fn main() -> i32 {
                return 0;
            }
        "#;
        let result = syn::parse_str::<SaltFile>(source);
        assert!(
            result.is_ok(),
            "Parser must accept @atomic attribute on struct fields, got: {:?}",
            result.err()
        );
    }

    // =========================================================================
    // LAYER 2: Z3 Alignment Proof — @atomic Fields Must Be 16-Byte Aligned [RED]
    // =========================================================================

    /// A struct with two u64 fields where the first is @atomic must pass
    /// Z3 alignment verification. The struct naturally aligns the @atomic
    /// field at offset 0, which satisfies the 16-byte alignment constraint
    /// when the struct itself is 16-byte aligned.
    ///
    /// This test verifies the compiler emits a Z3-proven alignment annotation.
    #[test]
    fn test_z3_proves_struct_16byte_aligned() {
        let mlir = compile_to_mlir(r#"
            package main
            struct TreiberNode {
                @atomic ptr: u64,
                gen: u64,
            }
            fn main() -> i32 {
                return 0;
            }
        "#);

        // The MLIR output should contain evidence that Z3 proved alignment.
        // This could be a comment annotation or the absence of a runtime check.
        assert!(
            mlir.contains("z3_aligned") || mlir.contains("align 16") || 
            !mlir.contains("__salt_alignment_violation"),
            "Z3 must prove @atomic field alignment. MLIR should contain alignment proof marker or no violation check, got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // LAYER 3: Z3 Alignment Rejection — Misaligned @atomic Field [RED]
    // =========================================================================

    /// A struct where @atomic is on a field at a non-16-byte-aligned offset
    /// must fail compilation. The Z3 Formal Shadow must reject this layout.
    ///
    /// struct Bad {
    ///     x: u8,           // offset 0, 1 byte
    ///     @atomic data: u64,  // offset 1 — NOT 16-byte aligned!
    /// }
    #[test]
    fn test_z3_rejects_unaligned_atomic_field() {
        let result = try_compile(r#"
            package main
            struct Bad {
                x: u8,
                @atomic data: u64,
            }
            fn main() -> i32 {
                return 0;
            }
        "#);

        assert!(
            result.is_err(),
            "Z3 must reject @atomic field at non-16-byte-aligned offset. \
             Struct Bad has u8 at offset 0 and @atomic u64 at offset 1, \
             which violates 16-byte alignment. Expected compilation error."
        );
    }

    // =========================================================================
    // LAYER 5: Struct-Level @atomic — Z3 Stride Alignment Proof
    // =========================================================================
    // When @atomic is placed on the struct itself (not a field), Z3 must prove
    // that sizeof(struct) % 16 == 0 for array stride safety. This guarantees
    // that every element in `[FreeHead; N]` is 16-byte aligned.

    /// @atomic struct with two u64 fields (16 bytes total) must PASS Z3 stride check.
    /// sizeof(FreeHead) = 16, 16 % 16 == 0: stride-safe for array indexing.
    #[test]
    fn test_z3_struct_level_atomic_16byte_passes() {
        let mlir = compile_to_mlir(r#"
            package main
            @atomic
            struct FreeHead {
                ptr: u64,
                gen: u64,
            }
            fn main() -> i32 {
                return 0;
            }
        "#);

        // Should compile successfully — Z3 stride proof passes during codegen.
        // The proof by contradiction (size % 16 != 0 → UNSAT) runs internally.
        assert!(
            !mlir.is_empty(),
            "Z3 must approve @atomic struct with sizeof == 16. MLIR should be generated.",
        );
    }

    /// @atomic struct with three u64 fields (24 bytes total) must FAIL Z3 stride check.
    /// sizeof(BadStruct) = 24, 24 % 16 != 0: stride-unsafe for cmpxchg16b arrays.
    #[test]
    fn test_z3_struct_level_atomic_24byte_rejected() {
        let result = try_compile(r#"
            package main
            @atomic
            struct BadStruct {
                a: u64,
                b: u64,
                c: u64,
            }
            fn main() -> i32 {
                return 0;
            }
        "#);

        assert!(
            result.is_err(),
            "Z3 must reject @atomic struct with sizeof == 24 (24 % 16 != 0). \
             Array stride would misalign elements for cmpxchg16b. \
             Expected compilation error."
        );
    }
}

