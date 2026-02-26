// =============================================================================
// TDD Tests: @packed Struct Zero-Padding Verification
// =============================================================================
//
// The @packed attribute instructs the Z3 Formal Shadow to prove that a struct
// has ZERO implicit padding between fields. This is critical for hardware
// mailbox protocols (like the SMP trampoline mailbox) where the 16-bit
// assembly and 64-bit kernel must agree on exact byte offsets.
//
// Z3 Theorem: sizeof(struct) == sum(sizeof(field_i))
//   If this holds, the compiler guarantees no alignment padding was inserted.
//
// TDD flow: RED (tests fail) → implement @packed gate → GREEN (tests pass)
// =============================================================================

#[cfg(test)]
mod tests {
    use crate::grammar::SaltFile;
    use crate::codegen::context::CodegenContext;

    /// Helper: compile Salt source and return MLIR or error.
    fn compile(source: &str) -> Result<String, String> {
        let file: SaltFile = syn::parse_str(source)
            .map_err(|e| format!("Parse error: {}", e))?;
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        ctx.drive_codegen()
    }

    // =========================================================================
    // Test 1: @packed struct with natural alignment (no padding) compiles
    // =========================================================================
    // All fields are u64 (8 bytes, 8-byte aligned). Total = 4*8 = 32 bytes.
    // No padding is needed → @packed proof succeeds.

    #[test]
    fn test_packed_struct_no_padding_compiles() {
        let result = compile(r#"
            package test::packed_ok

            @packed
            struct Mailbox {
                cr3: u64,
                stack: u64,
                entry: u64,
                state: u64,
            }

            @no_mangle
            pub fn main() -> u64 {
                return 0;
            }
        "#);

        assert!(
            result.is_ok(),
            "@packed struct with no implicit padding must compile. Error: {:?}",
            result.err()
        );
    }

    // =========================================================================
    // Test 2: @packed struct WITH implicit padding is rejected
    // =========================================================================
    // Fields: u64 (8) + i32 (4) = 12 bytes unpadded.
    // ABI will pad to 16 bytes (i32 followed by 4 bytes padding before next u64).
    // Z3 must REJECT this: ABI_size (16) != sum_of_fields (12).

    #[test]
    fn test_packed_struct_with_padding_rejected() {
        let result = compile(r#"
            package test::packed_fail

            @packed
            struct BadMailbox {
                cr3: u64,
                state: i32,
            }

            @no_mangle
            pub fn main() -> u64 {
                return 0;
            }
        "#);

        assert!(
            result.is_err(),
            "@packed struct with implicit padding must be REJECTED by Z3. \
             The struct has u64 (8) + i32 (4) = 12 bytes, but ABI pads to 16. \
             Got MLIR:\n{}",
            result.unwrap_or_default()
        );

        let err = result.unwrap_err();
        assert!(
            err.contains("padding") || err.contains("PACKED") || err.contains("packed"),
            "Error message must mention padding violation. Got: {}",
            err
        );
    }

    // =========================================================================
    // Test 3: @packed + @atomic struct (both constraints satisfied)
    // =========================================================================
    // SmpMailbox: 5 u64 (40) + i32 (4) + i32 (4) = 48 bytes.
    // @packed: 48 == sum(fields) ✓ (all fields naturally aligned)
    // @atomic: 48 % 16 == 0 ✓ (cmpxchg16b stride-safe)

    #[test]
    fn test_packed_atomic_struct_both_satisfied() {
        let result = compile(r#"
            package test::packed_atomic

            @packed
            @atomic
            struct SmpMailbox {
                gdt_ptr: u64,
                cr3: u64,
                stack: u64,
                entry: u64,
                ap_state: u64,
                _pad: u64,
            }

            @no_mangle
            pub fn main() -> u64 {
                return 0;
            }
        "#);

        assert!(
            result.is_ok(),
            "@packed @atomic struct with 48 bytes (mod 16 == 0, no padding) must compile. \
             Error: {:?}",
            result.err()
        );
    }
}
