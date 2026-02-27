// =============================================================================
// TDD Tests: Proof-Hint Engine — "Seal and Verify" (Directive 2.1 + 2.3)
// =============================================================================
//
// The compiler "seals" Z3 alignment proofs into a 64-bit proof_hint via
// SipHash-2-4 keyed hash. The NetD arbiter "verifies" the seal in O(1).
//
// Directive 2.3 (Hardening): Upgraded from FNV-1a to SipHash-2-4 for
// stronger collision resistance and resistance to hash-flooding attacks.
//
// Test vectors:
//   Layer 1-3: hash_combine determinism, collision resistance, avalanche
//   Layer 4:   MLIR proof-hint emission
//   Layer 5-6: Malicious pointer rejection (alignment subversion, hint forgery)
//   Layer 7:   SipHash-2-4 internal consistency (reference vectors)
// =============================================================================

#[cfg(test)]
mod tests {
    use crate::codegen::verification::proof_hint::{hash_combine, siphash24, K0, K1};

    // =========================================================================
    // LAYER 1: hash_combine — Determinism [RED]
    // =========================================================================
    // The same (struct_id, offset, align) tuple must always produce the
    // same 64-bit proof_hint. This is the foundation of reproducibility
    // across the Rust compiler and Salt runtime.

    #[test]
    fn test_hash_combine_deterministic() {
        let hint1 = hash_combine(42, 0, 64);
        let hint2 = hash_combine(42, 0, 64);
        assert_eq!(
            hint1, hint2,
            "hash_combine must be deterministic: same inputs → same output"
        );

        // Non-zero offset
        let hint3 = hash_combine(42, 64, 64);
        let hint4 = hash_combine(42, 64, 64);
        assert_eq!(hint3, hint4);

        // Result must be non-zero (no degenerate hash)
        assert_ne!(hint1, 0, "hash_combine must produce non-zero hints");
        assert_ne!(hint3, 0, "hash_combine must produce non-zero hints");
    }

    // =========================================================================
    // LAYER 2: hash_combine — Collision Resistance [RED]
    // =========================================================================
    // Different inputs must produce different hints. We test at least 5 pairs
    // to ensure the hash spreads across the 64-bit space.

    #[test]
    fn test_hash_combine_collision_resistant() {
        let pairs = [
            (1u64, 0u64, 64u64),
            (1, 64, 64),
            (2, 0, 64),
            (1, 0, 32),
            (42, 128, 16),
            (99, 0, 64),
            (0, 0, 8),
        ];

        let hints: Vec<u64> = pairs
            .iter()
            .map(|&(s, o, a)| hash_combine(s, o, a))
            .collect();

        // Each hint must be unique (no collisions in this small set)
        for i in 0..hints.len() {
            for j in (i + 1)..hints.len() {
                assert_ne!(
                    hints[i], hints[j],
                    "Collision detected: hash_combine({:?}) == hash_combine({:?}) == 0x{:016X}",
                    pairs[i], pairs[j], hints[i]
                );
            }
        }
    }

    // =========================================================================
    // LAYER 3: hash_combine — Bit Mixing [RED]
    // =========================================================================
    // Verify the hash has good avalanche behavior: changing a single input
    // bit should flip roughly half the output bits.

    #[test]
    fn test_hash_combine_avalanche() {
        let base = hash_combine(100, 0, 64);
        let varied = hash_combine(101, 0, 64); // Change struct_id by 1

        let diff_bits = (base ^ varied).count_ones();
        // At least 10 bits should differ (out of 64) for a decent hash
        assert!(
            diff_bits >= 10,
            "Poor avalanche: changing struct_id by 1 only flipped {} bits (need ≥10). \
             base=0x{:016X}, varied=0x{:016X}",
            diff_bits, base, varied
        );
    }

    // =========================================================================
    // LAYER 4: MLIR Proof-Hint Emission [RED]
    // =========================================================================
    // After Z3 proves @align(64) fields, the compiler must emit a
    // `salt.proof_hints` attribute on the MLIR module.

    #[test]
    fn test_proof_hint_emitted_in_mlir() {
        use crate::grammar::SaltFile;
        use crate::codegen::context::CodegenContext;

        let source = r#"
            package main
            struct SpscRing {
                @align(64) head: u64,
                capacity: u64,
                @align(64) tail: u64,
            }
            fn main() -> i32 {
                return 0;
            }
        "#;

        let file: SaltFile = syn::parse_str(source)
            .unwrap_or_else(|e| panic!("Failed to parse: {}", e));
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        let mlir = ctx.drive_codegen()
            .unwrap_or_else(|e| panic!("Codegen failed: {}", e));

        // The MLIR output must contain proof hints for the @align(64) fields
        assert!(
            mlir.contains("salt.proof_hints") || mlir.contains("proof_hint"),
            "MLIR must contain proof_hints attribute after Z3 alignment proof. \
             Got MLIR:\n{}",
            &mlir[..mlir.len().min(500)]
        );
    }

    // =========================================================================
    // LAYER 5: Malicious Pointer — Alignment Subversion [RED]
    // =========================================================================
    // The arbiter must reject a descriptor whose ptr is not aligned to 64
    // bytes, even if the proof_hint is valid.

    #[test]
    fn test_arbiter_rejects_unaligned_pointer() {
        use crate::codegen::verification::proof_hint::{hash_combine, validate_descriptor};

        let struct_id = 42u64;
        let offset = 0u64;
        let align = 64u64;
        let valid_hint = hash_combine(struct_id, offset, align);

        // Aligned pointer: accepted
        let aligned_ptr = 0x1000u64; // 4096, divisible by 64
        assert!(
            validate_descriptor(aligned_ptr, valid_hint, valid_hint),
            "Arbiter must accept aligned pointer with valid hint"
        );

        // Unaligned pointer: rejected (shifted by 8 bytes)
        let unaligned_ptr = 0x1008u64; // NOT divisible by 64
        assert!(
            !validate_descriptor(unaligned_ptr, valid_hint, valid_hint),
            "Arbiter MUST reject unaligned pointer even with valid hint. \
             This is Vector A: Alignment Subversion."
        );
    }

    // =========================================================================
    // LAYER 6: Malicious Pointer — Hint Forgery [RED]
    // =========================================================================
    // The arbiter must reject a descriptor with a forged proof_hint,
    // even if the pointer is properly aligned.

    #[test]
    fn test_arbiter_rejects_forged_hint() {
        use crate::codegen::verification::proof_hint::{hash_combine, validate_descriptor};

        let struct_id = 42u64;
        let valid_hint = hash_combine(struct_id, 0, 64);

        let aligned_ptr = 0x1000u64; // Properly aligned
        let forged_hint = 0xDEADBEEF_CAFEBABEu64; // Attacker's guess

        assert!(
            !validate_descriptor(aligned_ptr, forged_hint, valid_hint),
            "Arbiter MUST reject forged proof_hint. \
             This is Vector B: Hint Forgery."
        );

        // Also test with a hint from a different struct (stolen hint)
        let other_struct_hint = hash_combine(99, 0, 64);
        assert!(
            !validate_descriptor(aligned_ptr, other_struct_hint, valid_hint),
            "Arbiter MUST reject stolen hint from a different struct."
        );
    }

    // =========================================================================
    // LAYER 7: SipHash-2-4 — Reference Vectors [RED → GREEN]
    // =========================================================================
    // Verify that SipHash-2-4 with the Lattice sovereign keys produces
    // consistent, non-degenerate output.

    #[test]
    fn test_siphash24_nondegenerate() {
        // SipHash with sovereign keys must produce non-zero, non-trivial output
        let h0 = siphash24(K0, K1, 0);
        let h1 = siphash24(K0, K1, 1);
        let h2 = siphash24(K0, K1, u64::MAX);

        assert_ne!(h0, 0, "siphash24(K0, K1, 0) must be non-zero");
        assert_ne!(h1, 0, "siphash24(K0, K1, 1) must be non-zero");
        assert_ne!(h0, h1, "Different messages must produce different hashes");
        assert_ne!(h1, h2, "Different messages must produce different hashes");

        // Verify determinism
        assert_eq!(siphash24(K0, K1, 42), siphash24(K0, K1, 42));
    }

    #[test]
    fn test_siphash24_key_sensitivity() {
        // Different keys must produce different hashes for same message
        let h_sovereign = siphash24(K0, K1, 42);
        let h_zero_key = siphash24(0, 0, 42);
        let h_swapped = siphash24(K1, K0, 42);

        assert_ne!(h_sovereign, h_zero_key, "Sovereign keys vs zero keys must differ");
        assert_ne!(h_sovereign, h_swapped, "Key order must matter");
    }

    #[test]
    fn test_hash_combine_order_dependent() {
        // hash_combine(A, B, C) != hash_combine(B, A, C) — order matters
        let h1 = hash_combine(42, 0, 64);
        let h2 = hash_combine(0, 42, 64);
        assert_ne!(h1, h2, "hash_combine must be order-dependent");

        let h3 = hash_combine(42, 64, 0);
        assert_ne!(h1, h3, "Swapping offset and align must produce different hint");
    }

    #[test]
    fn test_siphash24_avalanche_strong() {
        // SipHash-2-4 should have excellent avalanche (> 20 bits flip per 1-bit change)
        let base = siphash24(K0, K1, 0);
        let flipped = siphash24(K0, K1, 1); // 1-bit change in message

        let diff_bits = (base ^ flipped).count_ones();
        assert!(
            diff_bits >= 20,
            "SipHash-2-4 avalanche: only {} bits flipped (need ≥20 for single-bit message change). \
             base=0x{:016X}, flipped=0x{:016X}",
            diff_bits, base, flipped
        );
    }

    // =========================================================================
    // LAYER 8: Salt Runtime Parity — Compiler/Runtime Agreement [RED → GREEN]
    // =========================================================================
    // The Salt runtime (ipc_arbiter.salt) pre-computes the packed bytes of
    // "SpscRing" as 0x676E695263737053. This test verifies that the Rust
    // compiler's struct_name_to_id("SpscRing") produces the same value as
    // siphash24(K0, K1, 0x676E695263737053) — proving bit-level parity.

    #[test]
    fn test_salt_runtime_parity_spsc_ring() {
        use crate::codegen::verification::proof_hint::struct_name_to_id;

        // This is the packed u64 from ipc_arbiter.salt::struct_name_to_id_spsc()
        // "SpscRing" bytes XOR-folded little-endian into u64:
        //   S=0x53, p=0x70, s=0x73, c=0x63, R=0x52, i=0x69, n=0x6E, g=0x67
        let salt_packed: u64 = 0x676E695263737053;
        let salt_struct_id = siphash24(K0, K1, salt_packed);

        let rust_struct_id = struct_name_to_id("SpscRing");

        assert_eq!(
            rust_struct_id, salt_struct_id,
            "PARITY VIOLATION: Rust struct_name_to_id(\"SpscRing\") = 0x{:016X} but \
             Salt siphash24(K0, K1, packed) = 0x{:016X}. These MUST match for \
             proof-carrying IPC to work.",
            rust_struct_id, salt_struct_id
        );

        // Also verify the full proof_hint for SpscRing.head matches
        let rust_hint_head = hash_combine(rust_struct_id, 0, 64);
        let salt_hint_head = hash_combine(salt_struct_id, 0, 64);
        assert_eq!(
            rust_hint_head, salt_hint_head,
            "PARITY VIOLATION: head proof_hint mismatch"
        );

        // And SpscRing.tail
        let rust_hint_tail = hash_combine(rust_struct_id, 64, 64);
        let salt_hint_tail = hash_combine(salt_struct_id, 64, 64);
        assert_eq!(
            rust_hint_tail, salt_hint_tail,
            "PARITY VIOLATION: tail proof_hint mismatch"
        );
    }
}
