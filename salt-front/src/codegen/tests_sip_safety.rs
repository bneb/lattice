// =============================================================================
// TDD Tests: Z3 SIP Signature Verification Gate
// =============================================================================
//
// Mode B SIPs execute in Ring 0 without hardware isolation. The compiler
// is the security perimeter. These tests verify that the Z3 Formal Shadow
// enforces safety invariants on SIP binaries:
//
//   1. SIPs may only call extern fns from the approved kernel API
//   2. SIPs may not use unsafe pointer casts (integer → pointer)
//   3. SIPs that pass verification emit a `salt.sip_verified` marker in MLIR
//   4. Kernel code (lib_mode without sip_mode) is NOT gated by SIP safety
//
// TDD flow: RED (tests fail) → implement gate → GREEN (tests pass)
// =============================================================================

#[cfg(test)]
mod tests {
    use crate::grammar::SaltFile;
    use crate::codegen::context::CodegenContext;

    /// Helper: compile a Salt source in SIP mode (sip_mode=true, lib_mode=true).
    /// SIP mode enables all safety gates.
    fn compile_sip(source: &str) -> Result<String, String> {
        let file: SaltFile = syn::parse_str(source)
            .map_err(|e| format!("Parse error: {}", e))?;
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        ctx.lib_mode = true;
        ctx.sip_mode = true;
        ctx.drive_codegen()
    }

    /// Helper: compile a Salt source in kernel lib mode (lib_mode=true, sip_mode=false).
    /// Kernel mode allows inttoptr because kernel code legitimately manipulates
    /// hardware addresses, page tables, and MMIO regions.
    fn compile_kernel(source: &str) -> Result<String, String> {
        let file: SaltFile = syn::parse_str(source)
            .map_err(|e| format!("Parse error: {}", e))?;
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        ctx.lib_mode = true;
        ctx.sip_mode = false;  // Kernel code: NOT a SIP
        ctx.drive_codegen()
    }

    // =========================================================================
    // Test 1: Well-behaved SIP compiles successfully
    // =========================================================================
    // A SIP that only uses approved patterns (arithmetic, function calls to
    // declared externs, let bindings) should compile without errors.

    #[test]
    fn test_sip_safe_program_compiles() {
        let result = compile_sip(r#"
            package user::sip_test

            extern fn scheduler_yield_now();

            @no_mangle
            pub fn _start() -> u64 {
                let a: u64 = 42;
                let b: u64 = 58;
                let result = a + b;
                scheduler_yield_now();
                return result;
            }
        "#);

        assert!(
            result.is_ok(),
            "Well-behaved SIP must compile. Error: {:?}",
            result.err()
        );
    }

    // =========================================================================
    // Test 2: SIP with unsafe integer-to-pointer cast is rejected
    // =========================================================================
    // Mode B SIPs MUST NOT cast arbitrary integers to pointers — this would
    // bypass the compiler's safety guarantees and allow writing to any
    // kernel address. Z3 should reject this pattern in sip_mode.

    #[test]
    fn test_sip_rejects_raw_pointer_cast() {
        let result = compile_sip(r#"
            package user::sip_unsafe

            @no_mangle
            pub fn _start() -> u64 {
                let addr: u64 = 0xDEAD0000;
                let ptr = addr as &mut u64;
                ptr[0] = 0xBAADF00D;
                return 0;
            }
        "#);

        assert!(
            result.is_err(),
            "SIP with raw integer-to-pointer cast must be REJECTED by Z3. \
             Allowing this would let SIPs write to arbitrary kernel memory. \
             Got MLIR:\n{}",
            result.unwrap_or_default()
        );
    }

    // =========================================================================
    // Test 3: SIP verified marker has MLIR dialect prefix
    // =========================================================================
    // MLIR requires dialect-prefixed attributes on builtin.module ops.
    // The marker must be "salt.sip_verified", not bare "sip_verified".

    #[test]
    fn test_sip_emits_verified_marker() {
        let mlir = compile_sip(r#"
            package user::sip_verified

            extern fn scheduler_yield_now();

            @no_mangle
            pub fn _start() -> u64 {
                scheduler_yield_now();
                return 42;
            }
        "#).expect("Safe SIP must compile");

        assert!(
            mlir.contains("salt.sip_verified"),
            "SIP MLIR must contain dialect-prefixed 'salt.sip_verified' marker \
             (not bare 'sip_verified'). MLIR:\n{}",
            mlir
        );
    }

    // =========================================================================
    // Test 4: Kernel lib_mode allows inttoptr (NOT a SIP)
    // =========================================================================
    // Kernel code uses --lib for "no main entry point" but is NOT a SIP.
    // It legitimately needs integer-to-pointer casts for MMIO, page tables,
    // and physical address manipulation. The SIP safety gate MUST NOT fire.

    #[test]
    fn test_kernel_lib_mode_allows_inttoptr() {
        let result = compile_kernel(r#"
            package kernel::core::test_inttoptr

            @no_mangle
            pub fn read_mmio() -> u64 {
                let addr: u64 = 0xFEE00000;
                let ptr = addr as &u64;
                return *ptr;
            }
        "#);

        assert!(
            result.is_ok(),
            "Kernel code (lib_mode=true, sip_mode=false) MUST allow inttoptr. \
             The SIP safety gate should only fire when sip_mode=true. \
             Error: {:?}",
            result.err()
        );
    }

    // =========================================================================
    // Test 5: Kernel lib_mode does NOT emit sip_verified marker
    // =========================================================================
    // Only SIP compilations should emit the sip_verified marker.
    // Kernel modules are not SIPs and must not be tagged as such.

    #[test]
    fn test_kernel_does_not_emit_sip_marker() {
        let mlir = compile_kernel(r#"
            package kernel::core::test_no_marker

            @no_mangle
            pub fn init() -> u64 {
                return 0;
            }
        "#).expect("Kernel code must compile");

        assert!(
            !mlir.contains("sip_verified"),
            "Kernel lib_mode MUST NOT emit sip_verified marker. \
             Only SIP compilations (sip_mode=true) should. MLIR:\n{}",
            mlir
        );
    }
}

