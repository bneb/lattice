// =============================================================================
// Entry Point Synthesis
//
// Generates the _salt_main entry point MLIR that bootstraps the KeuOS
// runtime. Replaces crt0 initialization with a direct jump into the
// KeuOS executor loop.
//
// Synthesis steps:
// 1. Initialize x19 (deadline register) to max value
// 2. Allocate DMA arena via keuos_arena_alloc
// 3. Launch State 0 of the primary @pulse function
// =============================================================================

use super::io_backend::TargetPlatform;

/// Entry point configuration for KeuOS binary synthesis.
pub struct EntryPointConfig {
    /// Name of the primary @pulse function to launch
    pub pulse_fn_name: String,
    /// Target platform
    pub target: TargetPlatform,
    /// Arena size in bytes (default: 16GB)
    pub arena_size_bytes: u64,
}

impl Default for EntryPointConfig {
    fn default() -> Self {
        Self {
            pulse_fn_name: "handler".to_string(),
            target: TargetPlatform::default(),
            arena_size_bytes: 16 * 1024 * 1024 * 1024, // 16GB
        }
    }
}

/// Synthesize the `_salt_main` entry point MLIR.
/// This function is the KeuOS equivalent of `_start` / `main`.
pub fn emit_entry_point(config: &EntryPointConfig) -> String {
    let mut out = String::new();

    out.push_str("    // Synthesized entry point: _salt_main\n");
    out.push_str("    // Bypasses crt0 initialization for zero-overhead startup\n");

    // Target-specific I/O init comment
    let io_init = match config.target {
        TargetPlatform::Darwin => "kqueue",
        TargetPlatform::Linux => "io_uring",
    };
    out.push_str(&format!(
        "    // Target: {} (I/O backend: {})\n",
        match config.target {
            TargetPlatform::Darwin => "aarch64-apple-darwin",
            TargetPlatform::Linux => "aarch64-unknown-linux-gnu",
        },
        io_init
    ));

    out.push_str("    func.func @_salt_main() {\n");

    // Step 1: Initialize x19 (deadline register) to max value
    // On ARM64, we use llvm.inline_asm to set x19 directly
    out.push_str("      // Step 1: Initialize deadline register (x19 = MAX)\n");
    out.push_str("      %max_deadline = arith.constant -1 : i64\n");
    out.push_str("      llvm.inline_asm has_side_effects \"mov x19, $0\", \"r\" %max_deadline : (i64) -> ()\n");

    // Step 2: Allocate DMA arena
    out.push_str("      // Step 2: Allocate DMA arena\n");
    out.push_str(&format!(
        "      %arena_size = arith.constant {} : i64\n",
        config.arena_size_bytes
    ));
    out.push_str("      %arena_ptr = func.call @keuos_arena_alloc(%arena_size) : (i64) -> !llvm.ptr\n");

    // Step 3: Launch State 0 of the primary @pulse function
    out.push_str("      // Step 3: Launch State 0\n");
    out.push_str(&format!(
        "      func.call @{}_state_0(%arena_ptr) : (!llvm.ptr) -> ()\n",
        config.pulse_fn_name
    ));

    // Step 4: Enter the executor loop (platform-specific)
    out.push_str("      // Step 4: Enter KeuOS executor loop\n");
    match config.target {
        TargetPlatform::Darwin => {
            out.push_str("      func.call @salt_kqueue_executor_loop(%arena_ptr) : (!llvm.ptr) -> ()\n");
        }
        TargetPlatform::Linux => {
            out.push_str("      func.call @salt_uring_executor_loop(%arena_ptr) : (!llvm.ptr) -> ()\n");
        }
    }

    out.push_str("      func.return\n");
    out.push_str("    }\n");

    out
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_point_initializes_x19() {
        let config = EntryPointConfig::default();
        let mlir = emit_entry_point(&config);

        assert!(mlir.contains("mov x19"),
            "Entry point must initialize x19 deadline register");
        assert!(mlir.contains("-1 : i64"),
            "Initial deadline must be MAX (all bits set)");
    }

    #[test]
    fn test_entry_point_allocates_arena() {
        let config = EntryPointConfig::default();
        let mlir = emit_entry_point(&config);

        assert!(mlir.contains("keuos_arena_alloc"),
            "Entry point must allocate DMA arena");
        assert!(mlir.contains("arena_size"),
            "Entry point must specify arena size");
    }

    #[test]
    fn test_entry_point_launches_state_0() {
        let config = EntryPointConfig {
            pulse_fn_name: "echo_handler".to_string(),
            ..Default::default()
        };
        let mlir = emit_entry_point(&config);

        assert!(mlir.contains("echo_handler_state_0"),
            "Entry point must launch State 0 of the named pulse function");
    }

    #[test]
    fn test_entry_point_target_aware() {
        let darwin_config = EntryPointConfig {
            target: TargetPlatform::Darwin,
            ..Default::default()
        };
        let linux_config = EntryPointConfig {
            target: TargetPlatform::Linux,
            ..Default::default()
        };

        let darwin_mlir = emit_entry_point(&darwin_config);
        let linux_mlir = emit_entry_point(&linux_config);

        assert!(darwin_mlir.contains("kqueue"),
            "Darwin entry must reference kqueue executor");
        assert!(linux_mlir.contains("uring"),
            "Linux entry must reference io_uring executor");
    }
}
