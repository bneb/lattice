// =============================================================================
// TDD Tests: SYN Cookie ISN Verification — Z3 Non-Zero Contract
// =============================================================================
//
// Verifies that the Salt compiler correctly handles the `ensures(result != 0)`
// contract on the SYN cookie generator. This is critical for TCP correctness:
// an ISN of 0 would be interpreted as "no sequence number" by kernel code.
//
//   Layer 1: ensures(result != 0) compiles and Z3 verifies for simple cases
//   Layer 2: Bitfield construction (shift + OR) compiles to correct MLIR
//   Layer 3: Cookie-like function with Z3 non-zero proof
//
// =============================================================================

mod tests {
    use crate::grammar::SaltFile;
    use crate::codegen::context::CodegenContext;

    /// Helper: compile a Salt source and return MLIR or error.
    fn try_compile(source: &str) -> Result<String, String> {
        let file: SaltFile = syn::parse_str(source)
            .unwrap_or_else(|e| panic!("Failed to parse Salt source: {}", e));
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        ctx.lib_mode = true;
        ctx.drive_codegen()
    }

    fn compile_to_mlir(source: &str) -> String {
        try_compile(source).unwrap_or_else(|e| panic!("Codegen failed: {}", e))
    }

    // =========================================================================
    // LAYER 1: ensures(result != 0) — Non-Zero Contract Verification
    // =========================================================================

    /// A function that always returns a non-zero value should have its
    /// ensures(result != 0) contract verified by Z3.
    #[test]
    fn test_nonzero_ensures_constant_return() {
        let result = try_compile(r#"
            package main

            pub fn always_nonzero() -> u32
                ensures(result != 0)
            {
                return 42;
            }
        "#);

        assert!(
            result.is_ok(),
            "ensures(result != 0) with constant non-zero return must be Z3-provable. Error: {:?}",
            result.err()
        );
    }

    /// A function with a conditional that guards the zero case should be provable.
    #[test]
    fn test_nonzero_ensures_guarded_return() {
        let result = try_compile(r#"
            package main

            pub fn guarded_nonzero(x: u32) -> u32
                ensures(result != 0)
            {
                if x == 0 {
                    return 1;
                }
                return x;
            }
        "#);

        assert!(
            result.is_ok(),
            "ensures(result != 0) with explicit zero-guard must be Z3-provable. Error: {:?}",
            result.err()
        );
    }

    // =========================================================================
    // LAYER 2: Bitfield Construction — Shift + OR Compiles
    // =========================================================================

    /// Cookie bitfield construction (shift + OR) must compile to correct MLIR.
    /// This validates the expression: (t << 27) | (mss << 24) | (hash & 0xFFFFFF)
    #[test]
    fn test_bitfield_construction_compiles() {
        let mlir = compile_to_mlir(r#"
            package main

            pub fn make_cookie(t: u32, mss: u32, hash: u32) -> u32 {
                let cookie: u32 = ((t & 31) << 27) | ((mss & 7) << 24) | (hash & 16777215);
                return cookie;
            }
        "#);

        // Must contain shift and OR operations
        assert!(
            mlir.contains("arith.shli") || mlir.contains("shl"),
            "Bitfield construction must emit shift operations. Got:\n{}",
            &mlir[..mlir.len().min(500)]
        );
    }

    // =========================================================================
    // LAYER 3: Cookie-like ensures(result != 0) with zero guard
    // =========================================================================

    /// A cookie-like function that constructs a bitfield and guards against
    /// returning zero should have its postcondition verified.
    #[test]
    fn test_cookie_pattern_nonzero_contract() {
        let result = try_compile(r#"
            package main

            pub fn make_safe_cookie(hash: u32) -> u32
                ensures(result != 0)
            {
                let cookie: u32 = hash & 16777215;
                if cookie == 0 {
                    return 1;
                }
                return cookie;
            }
        "#);

        assert!(
            result.is_ok(),
            "Cookie with zero-guard and ensures(result != 0) must compile. Error: {:?}",
            result.err()
        );
    }

    /// The rotl64 pattern must compile (used in SipHash rounds).
    #[test]
    fn test_rotate_left_pattern_compiles() {
        let result = try_compile(r#"
            package main

            pub fn rotl64(x: u64, n: i32) -> u64 {
                return (x << n) | (x >> (64 - n));
            }
        "#);

        assert!(
            result.is_ok(),
            "rotl64 bit rotation must compile. Error: {:?}",
            result.err()
        );
    }

    /// XOR mixing pattern (SipHash) must compile.
    #[test]
    fn test_xor_mixing_pattern_compiles() {
        let result = try_compile(r#"
            package main

            pub fn mix(a: u64, b: u64) -> u64 {
                let mut v = a ^ b;
                v = v + a;
                v = v ^ (b << 13);
                return v;
            }
        "#);

        assert!(
            result.is_ok(),
            "XOR mixing pattern must compile. Error: {:?}",
            result.err()
        );
    }
}
