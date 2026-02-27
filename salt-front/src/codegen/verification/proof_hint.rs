// =============================================================================
// Proof-Hint Generation — "Seal and Verify" (Directive 2.1 + 2.3 Hardening)
// =============================================================================
//
// This module provides the cryptographic "seal" for Z3 alignment proofs.
// The compiler generates a 64-bit proof_hint via SipHash-2-4 keyed hash
// that is embedded in the binary's data plane as a constant.
//
// The NetD arbiter verifies this hint in O(1) cycles via bitwise comparison,
// completing the Sovereignty Gap bridge between compile-time Z3 verification
// and runtime trust.
//
// Algorithm: SipHash-2-4 — industry-standard keyed hash, collision-resistant,
// deterministic, and reproducible in both Rust (compiler) and Salt (runtime).
//
// Hardening Phase (v0.9.2): Upgraded from FNV-1a to SipHash-2-4 for stronger
// collision resistance and resistance to hash-flooding attacks.
// =============================================================================

/// SipHash-2-4 key derived from the Lattice sovereign identity.
/// These keys are compile-time constants shared between the compiler and
/// the Salt runtime. Changing them invalidates all existing proof_hints.
const SIPHASH_K0: u64 = 0x4c61747469636521; // "Lattice!" in ASCII
const SIPHASH_K1: u64 = 0x536f76657265696e; // "Soverein" in ASCII

/// SipHash-2-4 core round function (ARX: Add-Rotate-Xor).
#[inline(always)]
fn sip_round(v0: &mut u64, v1: &mut u64, v2: &mut u64, v3: &mut u64) {
    *v0 = v0.wrapping_add(*v1);
    *v2 = v2.wrapping_add(*v3);
    *v1 = v1.rotate_left(13);
    *v3 = v3.rotate_left(16);
    *v1 ^= *v0;
    *v3 ^= *v2;
    *v0 = v0.rotate_left(32);
    *v2 = v2.wrapping_add(*v1);
    *v0 = v0.wrapping_add(*v3);
    *v1 = v1.rotate_left(17);
    *v3 = v3.rotate_left(21);
    *v1 ^= *v2;
    *v3 ^= *v0;
    *v2 = v2.rotate_left(32);
}

/// SipHash-2-4 for a single 64-bit message word.
///
/// This is the primitive used for proof-hint generation. It hashes a single
/// u64 message `m` with the sovereign key pair (K0, K1) to produce a 64-bit
/// hash with strong collision resistance.
///
/// 2 compression rounds + 4 finalization rounds.
pub fn siphash24(k0: u64, k1: u64, m: u64) -> u64 {
    let mut v0 = k0 ^ 0x736f6d6570736575; // "somepseu"
    let mut v1 = k1 ^ 0x646f72616e646f6d; // "dorandom"
    let mut v2 = k0 ^ 0x6c7967656e657261; // "lygenera"
    let mut v3 = k1 ^ 0x7465646279746573; // "tedbytes"

    // Compress message
    v3 ^= m;
    sip_round(&mut v0, &mut v1, &mut v2, &mut v3); // Round 1
    sip_round(&mut v0, &mut v1, &mut v2, &mut v3); // Round 2
    v0 ^= m;

    // Finalization (4 rounds)
    v2 ^= 0xff;
    sip_round(&mut v0, &mut v1, &mut v2, &mut v3);
    sip_round(&mut v0, &mut v1, &mut v2, &mut v3);
    sip_round(&mut v0, &mut v1, &mut v2, &mut v3);
    sip_round(&mut v0, &mut v1, &mut v2, &mut v3);

    v0 ^ v1 ^ v2 ^ v3
}

/// Generate a 64-bit proof_hint from a struct's alignment contract.
///
/// The hint seals three properties from the Z3 alignment proof:
///   - `struct_id`: a hash of the struct name (identity)
///   - `offset`: the byte offset of the field within the struct (layout)
///   - `align`: the alignment requirement in bytes (contract)
///
/// Algorithm: Cascaded SipHash-2-4 — each component is mixed into the
/// hash state sequentially, producing a 64-bit seal.
///
/// This MUST produce identical results to the Salt runtime `hash_combine`.
pub fn hash_combine(struct_id: u64, offset: u64, align: u64) -> u64 {
    // Cascade: hash(struct_id) → hash(result ⊕ offset) → hash(result ⊕ align)
    let h1 = siphash24(SIPHASH_K0, SIPHASH_K1, struct_id);
    let h2 = siphash24(SIPHASH_K0, SIPHASH_K1, h1 ^ offset);
    let h3 = siphash24(SIPHASH_K0, SIPHASH_K1, h2 ^ align);
    h3
}

/// Generate a struct_id from a struct name.
///
/// Uses SipHash-2-4 on a packed representation of the name bytes.
/// For names ≤ 8 bytes, the name is packed into a single u64 and hashed.
/// For longer names, bytes are folded via XOR into a u64 first.
///
/// This MUST produce identical results to the Salt runtime's struct_name_to_id.
pub fn struct_name_to_id(name: &str) -> u64 {
    // Pack name bytes into a u64 (fold with XOR for names > 8 bytes)
    let mut packed: u64 = 0;
    for (i, byte) in name.bytes().enumerate() {
        packed ^= (byte as u64) << ((i % 8) * 8);
    }
    siphash24(SIPHASH_K0, SIPHASH_K1, packed)
}

/// Validate a descriptor in O(1) cycles.
///
/// This is the arbiter's fast-path check:
///   1. Mechanical check: pointer must be 64-byte aligned
///   2. Logical check: proof_hint must match authorized_hint (bitwise)
///
/// Returns true if the descriptor is valid, false if it should be rejected.
pub fn validate_descriptor(ptr: u64, proof_hint: u64, authorized_hint: u64) -> bool {
    // Gate 1: Mechanical alignment check (64-byte cache line)
    if (ptr & 0x3F) != 0 {
        return false;
    }

    // Gate 2: Proof-hint bitwise comparison (constant-time)
    proof_hint == authorized_hint
}

// =============================================================================
// Exported constants for Salt runtime parity
// =============================================================================
// The Salt runtime (ipc_arbiter.salt) must use these same key values.

/// SipHash key component 0 ("Lattice!" in ASCII)
pub const K0: u64 = SIPHASH_K0;
/// SipHash key component 1 ("Soverein" in ASCII)
pub const K1: u64 = SIPHASH_K1;
