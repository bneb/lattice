// =============================================================================
// Hardware Target Abstraction
//
// Hardware-agnostic trait modeling per-phase cycle costs for KeuOS packet
// processing. Each hardware target implements the trait with its specific
// microarchitecture timings. The C10M validator uses this interface to
// determine if a target can sustain 10M concurrent connections.
//
// Supported targets:
//   - M4Target (Apple M4, aarch64, kqueue/io_uring)
//   - Future: SapphireRapidsTarget, RiscVTarget, etc.
// =============================================================================

use super::silicon_ingest::m4_timing;

/// Hardware-agnostic trait for KeuOS packet flow cycle modeling.
///
/// Each method returns the cycle cost for one phase of the KeuOS
/// packet pipeline on the implementing hardware target.
pub trait HardwareTarget {
    /// Human-readable name of the hardware target.
    fn name(&self) -> &str;

    /// Clock frequency in GHz (e.g., 4.4 for M4 P-core).
    fn clock_ghz(&self) -> f64;

    /// L1D cache size in bytes.
    fn l1d_bytes(&self) -> u64;

    /// TaskFrame size in bytes on this target.
    fn task_frame_bytes(&self) -> u64;

    /// Ingress phase: I/O reap from kernel ring (batch amortized).
    /// M4: io_uring CQE harvest + SQE prep = ~170 cycles per batch element
    fn ingress_cycles(&self) -> u64;

    /// Dispatch phase: Jump table GEP + MustTail indirect branch.
    /// M4: 25 cycles (stackless context swap)
    fn dispatch_cycles(&self) -> u64;

    /// Safety phase: bounds/null checks.
    /// Salt: 0 (aspirational — Z3 elision for provable subset)
    fn safety_cycles(&self) -> u64;

    /// Processing phase: SIMD parsing + body execution.
    /// M4: NEON-accelerated header scan + path extraction
    fn processing_cycles(&self) -> u64;

    /// Egress phase: I/O submit to kernel ring.
    /// M4: io_uring SQE submit = ~10 cycles
    fn egress_cycles(&self) -> u64;

    /// Total per-packet cycle budget (sum of all phases).
    fn packet_budget(&self) -> u64 {
        self.ingress_cycles()
            + self.dispatch_cycles()
            + self.safety_cycles()
            + self.processing_cycles()
            + self.egress_cycles()
    }

    /// Maximum packets/sec this target can sustain (single core).
    fn max_packets_per_sec(&self) -> f64 {
        let cycles_per_sec = self.clock_ghz() * 1e9;
        cycles_per_sec / self.packet_budget() as f64
    }

    /// Number of TaskFrames that fit in L1D cache.
    fn tasks_in_l1d(&self) -> u64 {
        self.l1d_bytes() / self.task_frame_bytes()
    }
}

// =============================================================================
// Apple M4 Target
// =============================================================================

/// Apple M4 (aarch64) hardware target.
/// P-core @ 4.4GHz, 64KB L1D, FEAT_LSE, NEON SIMD.
pub struct M4Target;

impl HardwareTarget for M4Target {
    fn name(&self) -> &str { "Apple M4 (aarch64)" }
    fn clock_ghz(&self) -> f64 { 4.4 }
    fn l1d_bytes(&self) -> u64 { m4_timing::L1D_SIZE }
    fn task_frame_bytes(&self) -> u64 { m4_timing::TASK_FRAME_SIZE_SALT }

    fn ingress_cycles(&self) -> u64 {
        // Batch reap: io_uring_enter amortized + CQE harvest + buffer handling
        // Modeled from silicon_ingest: SQE_PREP(5) + CQE_HARVEST(5) + ENTER(2)
        // Full batch: 170 cycles amortized across 256-element batch
        // Per-packet amortized: ~12 cycles (matches silicon_ingest io_cycles)
        m4_timing::IOURING_ENTER_AMORTIZED
            + m4_timing::IOURING_SQE_PREP
            + m4_timing::IOURING_CQE_HARVEST
    }

    fn dispatch_cycles(&self) -> u64 {
        // MustTail GEP + indirect branch: stackless context swap
        m4_timing::STACKLESS_SWAP
    }

    fn safety_cycles(&self) -> u64 {
        // Aspirational: Z3 elision for provable subset; unprovable formulas hit 100ms timeout
        0
    }

    fn processing_cycles(&self) -> u64 {
        // NEON SIMD header scan (200 bytes) + path extraction (12 bytes)
        // + method dispatch + view creation + response assembly
        // Uses the same functions as silicon_ingest
        let header_bytes: u64 = 200;
        let path_bytes: u64 = 12;

        // SIMD header scan: 13 iterations × 5 cycles/iter = 65 cycles
        let header_iters = header_bytes.div_ceil(16);
        let header = header_iters * (m4_timing::NEON_LD1_L1 + m4_timing::NEON_CMEQ);

        // Path extraction: 12 bytes × 5 cycles/byte = 60 cycles
        let path = path_bytes * (m4_timing::SCALAR_LOAD_L1 + m4_timing::ALU_SIMPLE);

        // Method dispatch + view + response
        let method = m4_timing::SCALAR_LOAD_L1 + 3 * m4_timing::BRANCH_PREDICTED;
        let view = 2 * m4_timing::STORE_L1 + m4_timing::ALU_SIMPLE;
        let response = 3 * m4_timing::ALU_SIMPLE + 2 * m4_timing::STORE_L1;

        header + path + method + view + response
    }

    fn egress_cycles(&self) -> u64 {
        // io_uring submit: SQE prep + enter (amortized)
        m4_timing::IOURING_SQE_PREP + m4_timing::IOURING_ENTER_AMORTIZED
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_m4_target_packet_budget() {
        let m4 = M4Target;
        let budget = m4.packet_budget();

        // Budget should be in the range of the silicon_ingest model (~233 total)
        // Our per-phase decomposition may differ slightly but should be < 300
        assert!(budget > 50, "M4 budget should be > 50 cycles, got {}", budget);
        assert!(budget < 300, "M4 budget should be < 300 cycles, got {}", budget);
    }

    #[test]
    fn test_m4_ingress_uses_uring_reap() {
        let m4 = M4Target;
        let ingress = m4.ingress_cycles();

        // Ingress = io_uring batch reap, should be ~12 cycles (amortized)
        assert_eq!(ingress,
            m4_timing::IOURING_ENTER_AMORTIZED
                + m4_timing::IOURING_SQE_PREP
                + m4_timing::IOURING_CQE_HARVEST,
            "Ingress must use io_uring reap components"
        );
    }

    #[test]
    fn test_m4_dispatch_25_cycles() {
        let m4 = M4Target;
        let dispatch = m4.dispatch_cycles();

        // Dispatch = stackless swap = 25 cycles
        assert_eq!(dispatch, m4_timing::STACKLESS_SWAP,
            "Dispatch must equal STACKLESS_SWAP (25 cycles)");
    }

    #[test]
    fn test_m4_safety_zero_cycles() {
        let m4 = M4Target;
        assert_eq!(m4.safety_cycles(), 0,
            "Safety must be 0 cycles (aspirational — Z3 elision for provable subset)");
    }

    #[test]
    fn test_hardware_target_is_trait() {
        // Verify the trait can be used as a trait object
        fn validate_target(t: &dyn HardwareTarget) -> u64 {
            t.packet_budget()
        }
        let m4 = M4Target;
        let budget = validate_target(&m4);
        assert!(budget > 0, "Trait object dispatch must work");
    }
}
