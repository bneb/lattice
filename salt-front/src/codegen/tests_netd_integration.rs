// =============================================================================
// TDD Tests: NetD SYN Cookie Integration — Codegen Verification
// =============================================================================
// Verifies that the integration of SYN cookies into netd_tcp.salt compiles
// correctly and the cookie-based handshake pattern produces valid MLIR.
// =============================================================================

#[cfg(test)]
mod tests {

    fn compile_to_mlir(salt_code: &str) -> Result<String, String> {
        let full = format!("package main\n{}", salt_code);
        let processed = crate::preprocess(&full);
        let mut file: crate::grammar::SaltFile = syn::parse_str(&processed)
            .map_err(|e| format!("Parse error: {}", e))?;
        crate::compile_ast(&mut file, false, None, true, false, false, false, false, false, false, "test.salt")
            .map_err(|e| format!("Codegen error: {}", e))
    }

    // =========================================================================
    // Test 1: handle_syn pattern — cookie generation without TCB allocation
    // =========================================================================
    #[test]
    fn test_handle_syn_no_alloc_pattern() {
        let code = r#"
            fn generate_cookie(src_ip: u32, dst_ip: u32) -> u32 {
                let tuple: u64 = ((src_ip as u64) << 32) | (dst_ip as u64);
                let hash: u32 = (tuple & 0x00FFFFFF) as u32;
                if hash == 0 { return 1; }
                return hash;
            }

            fn handle_syn(src_ip: u32, dst_ip: u32) -> u32 {
                // Stateless — no pool_alloc() call
                return generate_cookie(src_ip, dst_ip);
            }

            fn main() -> i32 {
                let cookie = handle_syn(0x0A000001, 0xC0A80001);
                if cookie != 0 { return 0; }
                return 1;
            }
        "#;

        let mlir = compile_to_mlir(code).expect("handle_syn pattern must compile");
        // Must contain the handle_syn function
        assert!(mlir.contains("handle_syn"), "MLIR must contain handle_syn function");
        // Must contain the generate_cookie call
        assert!(mlir.contains("generate_cookie"), "MLIR must contain generate_cookie");
        // Must NOT contain pool_alloc (the whole point of stateless SYN)
        assert!(!mlir.contains("pool_alloc"), "handle_syn must NOT call pool_alloc");
    }

    // =========================================================================
    // Test 2: handle_ack pattern — validate then alloc
    // =========================================================================
    #[test]
    fn test_handle_ack_validate_then_alloc() {
        let code = r#"
            fn validate_cookie(cookie: u32, src_ip: u32) -> bool {
                let expected = src_ip & 0x00FFFFFF;
                return (cookie & 0x00FFFFFF) == expected;
            }

            const MAX_SOCKETS: u64 = 64;
            const CLOSED: u8 = 0;
            const ESTABLISHED: u8 = 4;

            global STATES: [u8; 64] = [0 as u8; 64];

            fn pool_alloc() -> u64 {
                let mut i: u64 = 0;
                while i < MAX_SOCKETS {
                    if STATES[i] == CLOSED { return i; }
                    i = i + 1;
                }
                return 0xFFFFFFFF;
            }

            fn handle_ack(cookie: u32, src_ip: u32) -> u64 {
                if !validate_cookie(cookie, src_ip) {
                    return 0xFFFFFFFF;
                }
                let slot = pool_alloc();
                if slot == 0xFFFFFFFF { return 0xFFFFFFFF; }
                STATES[slot] = ESTABLISHED;
                return slot;
            }

            fn main() -> i32 {
                let slot = handle_ack(0x0A000001, 0x0A000001);
                if slot != 0xFFFFFFFF { return 0; }
                return 1;
            }
        "#;

        let mlir = compile_to_mlir(code).expect("handle_ack pattern must compile");
        assert!(mlir.contains("handle_ack"), "MLIR must contain handle_ack");
        assert!(mlir.contains("validate_cookie"), "handle_ack must call validate_cookie");
        assert!(mlir.contains("pool_alloc"), "handle_ack must call pool_alloc after validation");
    }

    // =========================================================================
    // Test 3: Bitfield extraction from cookie (timestamp, MSS, hash)
    // =========================================================================
    #[test]
    fn test_cookie_bitfield_extraction() {
        let code = r#"
            fn extract_timestamp(cookie: u32) -> u32 {
                return (cookie >> 27) & 31;
            }

            fn extract_mss_index(cookie: u32) -> u32 {
                return (cookie >> 24) & 7;
            }

            fn extract_hash(cookie: u32) -> u32 {
                return cookie & 0x00FFFFFF;
            }

            fn main() -> i32 {
                let cookie: u32 = 0xABCDEF12;
                let ts = extract_timestamp(cookie);
                let mss = extract_mss_index(cookie);
                let hash = extract_hash(cookie);
                if ts > 31 { return 1; }
                if mss > 7 { return 1; }
                return 0;
            }
        "#;

        let mlir = compile_to_mlir(code).expect("Bitfield extraction must compile");
        // Must use shift-right for extraction
        assert!(mlir.contains("arith.shrui"), "Bitfield extraction must use unsigned right shift");
    }

    // =========================================================================
    // Test 4: SYN flood loop compiles (1000 iterations, no alloc)
    // =========================================================================
    #[test]
    fn test_syn_flood_loop_compiles() {
        let code = r#"
            fn generate_cookie(src_ip: u32) -> u32 {
                return src_ip & 0x00FFFFFF;
            }

            fn main() -> i32 {
                let mut i: u32 = 0;
                let mut total: u32 = 0;
                while i < 1000 {
                    let c = generate_cookie(0x0A000000 + i);
                    total = total + c;
                    i = i + 1;
                }
                if total > 0 { return 0; }
                return 1;
            }
        "#;

        let mlir = compile_to_mlir(code).expect("SYN flood loop must compile");
        // Must contain loop structure
        assert!(mlir.contains("scf.while") || mlir.contains("scf.for") || mlir.contains("cf.br"),
            "SYN flood loop must compile to loop construct");
    }
}
