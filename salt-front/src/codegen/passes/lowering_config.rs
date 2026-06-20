// =============================================================================
// LLVM Lowering Configuration
//
// Generates llc flags and MLIR function attributes for KeuOS binaries.
// Ensures x19 is reserved (hardware deadline register), LSE atomics are
// enabled, and state functions are marked noinline to prevent register
// allocator interference.
// =============================================================================

use super::io_backend::TargetPlatform;

/// LLVM lowering configuration for KeuOS binary production.
/// Holds target-specific settings that control register reservation,
/// feature flags, and function attributes during MLIR → native lowering.
#[derive(Debug, Clone)]
pub struct LoweringConfig {
    pub target: TargetPlatform,
}

impl LoweringConfig {
    pub fn new(target: TargetPlatform) -> Self {
        Self { target }
    }

    /// Generate `llc` command-line flags for KeuOS binary lowering.
    /// Critical flags:
    /// - `-reserved-reg=aarch64:x19`: Protects the hardware deadline register
    /// - `--mattr=+lse`: Enables FEAT_LSE atomics (CAS, LDADD) for M4
    /// - `--frame-pointer=none`: Eliminates frame pointer overhead
    pub fn emit_llc_flags(&self) -> Vec<String> {
        let mut flags = Vec::new();

        // Target triple
        let triple = match self.target {
            TargetPlatform::Darwin => "aarch64-apple-darwin",
            TargetPlatform::Linux => "aarch64-unknown-linux-gnu",
        };
        flags.push(format!("-mtriple={}", triple));

        // Reserve x19 — the KeuOS deadline register
        flags.push("-reserved-reg=aarch64:x19".to_string());

        // Enable LSE atomics (FEAT_LSE: CAS, LDADD, SWP)
        flags.push("--mattr=+lse".to_string());

        // No frame pointer — saves a register and cycle
        flags.push("--frame-pointer=none".to_string());

        // Optimization level
        flags.push("-O3".to_string());

        flags
    }

    /// Generate MLIR `passthrough` attributes for state functions.
    /// State functions are marked `noinline` to prevent the optimizer from
    /// merging them (which would destroy the jump table dispatch pattern).
    pub fn emit_state_fn_attributes(&self) -> String {
        "attributes { passthrough = [\"noinline\"] }".to_string()
    }

    /// Generate the `llc` command string for the given MLIR input file.
    pub fn emit_llc_command(&self, input_file: &str, output_file: &str) -> String {
        let flags = self.emit_llc_flags();
        format!("llc {} {} -o {}", flags.join(" "), input_file, output_file)
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llc_flags_reserves_x19() {
        let config = LoweringConfig::new(TargetPlatform::Darwin);
        let flags = config.emit_llc_flags();
        let joined = flags.join(" ");

        assert!(joined.contains("-reserved-reg=aarch64:x19"),
            "llc flags must reserve x19 to protect deadline register");
    }

    #[test]
    fn test_llc_flags_enables_lse() {
        let config = LoweringConfig::new(TargetPlatform::Darwin);
        let flags = config.emit_llc_flags();
        let joined = flags.join(" ");

        assert!(joined.contains("+lse"),
            "llc flags must enable LSE atomics for M4 CAS/LDADD");
    }

    #[test]
    fn test_fn_attributes_include_noinline() {
        let config = LoweringConfig::new(TargetPlatform::Darwin);
        let attrs = config.emit_state_fn_attributes();

        assert!(attrs.contains("noinline"),
            "State function attributes must include noinline");
        assert!(attrs.contains("passthrough"),
            "Attributes must use MLIR passthrough format");
    }

    #[test]
    fn test_lowering_config_target_aware() {
        let darwin = LoweringConfig::new(TargetPlatform::Darwin);
        let linux = LoweringConfig::new(TargetPlatform::Linux);

        let darwin_flags = darwin.emit_llc_flags().join(" ");
        let linux_flags = linux.emit_llc_flags().join(" ");

        assert!(darwin_flags.contains("aarch64-apple-darwin"),
            "Darwin config must use apple-darwin triple");
        assert!(linux_flags.contains("aarch64-unknown-linux-gnu"),
            "Linux config must use linux-gnu triple");

        // Both must reserve x19
        assert!(darwin_flags.contains("-reserved-reg=aarch64:x19"),
            "Darwin must reserve x19");
        assert!(linux_flags.contains("-reserved-reg=aarch64:x19"),
            "Linux must reserve x19");
    }
}
