// =============================================================================
// Post-Compilation Binary Audit
//
// Verification rules for the resulting object file. After MLIR → LLVM → native
// lowering, we audit the disassembly to ensure the compiler respected KeuOS
// invariants:
//   1. x19 must never be spilled to stack (stp x19 = FAIL)
//   2. Dispatch hub must use tail calls (br xN = PASS)
//   3. I/O syscalls must be present (svc = PASS)
// =============================================================================

use super::io_backend::TargetPlatform;

/// Audit rules for post-compilation binary verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditRule {
    /// x19 must never be spilled to stack.
    /// Pattern: `stp x19` or `str x19` in disassembly = FAILURE
    NoX19Spill,
    /// Dispatch hub must use tail calls.
    /// Pattern: `br x` (indirect branch) in dispatch hub = PASS
    HasTailCall,
    /// I/O syscalls must be present in the binary.
    /// Pattern: `svc` instruction present = PASS
    HasIoSyscall,
}

/// Configuration for binary audit based on target platform.
#[derive(Debug, Clone)]
pub struct BinaryAuditConfig {
    pub target: TargetPlatform,
    pub rules: Vec<AuditRule>,
}

impl BinaryAuditConfig {
    /// Create a default audit config with all standard rules.
    pub fn standard(target: TargetPlatform) -> Self {
        Self {
            target,
            rules: vec![
                AuditRule::NoX19Spill,
                AuditRule::HasTailCall,
                AuditRule::HasIoSyscall,
            ],
        }
    }
}

/// Result of a single audit check.
#[derive(Debug, Clone)]
pub struct AuditResult {
    pub rule: AuditRule,
    pub passed: bool,
    pub detail: String,
}

/// Check a single audit rule against disassembly text.
/// `disasm` is the output of `objdump -d` or equivalent.
pub fn check_pattern(rule: AuditRule, disasm: &str) -> AuditResult {
    match rule {
        AuditRule::NoX19Spill => {
            // x19 must NEVER be spilled — search for stp/str x19
            let has_spill = disasm.contains("stp\tx19")
                || disasm.contains("str\tx19")
                || disasm.contains("stp x19")
                || disasm.contains("str x19");
            AuditResult {
                rule,
                passed: !has_spill,
                detail: if has_spill {
                    "CRITICAL: x19 is spilled to stack — deadline register corrupted".to_string()
                } else {
                    "x19 is never spilled — deadline register protected".to_string()
                },
            }
        }
        AuditRule::HasTailCall => {
            // Dispatch hub must use indirect branch (br xN)
            let has_tail = disasm.contains("br\tx")
                || disasm.contains("br x");
            AuditResult {
                rule,
                passed: has_tail,
                detail: if has_tail {
                    "Indirect branch (br xN) found — MustTail dispatch confirmed".to_string()
                } else {
                    "WARNING: No indirect branch found — dispatch may use call instead of tail".to_string()
                },
            }
        }
        AuditRule::HasIoSyscall => {
            // I/O syscall must be present (svc instruction)
            let has_svc = disasm.contains("svc");
            AuditResult {
                rule,
                passed: has_svc,
                detail: if has_svc {
                    "I/O syscall (svc) found — kernel ring interface active".to_string()
                } else {
                    "WARNING: No svc instruction — I/O backend may not be linked".to_string()
                },
            }
        }
    }
}

/// Run all audit rules against disassembly text.
pub fn run_audit(config: &BinaryAuditConfig, disasm: &str) -> Vec<AuditResult> {
    config.rules.iter()
        .map(|rule| check_pattern(*rule, disasm))
        .collect()
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_no_x19_spill_rule() {
        // Disassembly with x19 spill should FAIL
        let bad_disasm = "stp x19, x20, [sp, #-16]!\nmov x0, x1\nldp x19, x20, [sp], #16";
        let result = check_pattern(AuditRule::NoX19Spill, bad_disasm);
        assert!(!result.passed, "x19 spill must be detected as failure");
        assert!(result.detail.contains("CRITICAL"),
            "x19 spill must produce CRITICAL warning");

        // Clean disassembly should PASS
        let good_disasm = "stp x20, x21, [sp, #-16]!\nmov x0, x1\nldp x20, x21, [sp], #16";
        let result = check_pattern(AuditRule::NoX19Spill, good_disasm);
        assert!(result.passed, "Disassembly without x19 spill must pass");
    }

    #[test]
    fn test_audit_has_tail_call_rule() {
        // Disassembly with indirect branch should PASS
        let good_disasm = "ldr x3, [x0, x1, lsl #3]\nbr x3";
        let result = check_pattern(AuditRule::HasTailCall, good_disasm);
        assert!(result.passed, "Indirect branch (br xN) must be detected");
        assert!(result.detail.contains("MustTail"),
            "Detail must confirm MustTail dispatch");

        // Disassembly without indirect branch should FAIL
        let bad_disasm = "bl _some_function\nret";
        let result = check_pattern(AuditRule::HasTailCall, bad_disasm);
        assert!(!result.passed, "Missing indirect branch must be flagged");
    }

    #[test]
    fn test_audit_has_io_syscall_rule() {
        let good_disasm = "mov x16, #0x50\nsvc #0x80";
        let result = check_pattern(AuditRule::HasIoSyscall, good_disasm);
        assert!(result.passed, "svc instruction must be detected");

        let bad_disasm = "mov x0, #42\nret";
        let result = check_pattern(AuditRule::HasIoSyscall, bad_disasm);
        assert!(!result.passed, "Missing svc must be flagged");
    }

    #[test]
    fn test_run_audit_all_rules() {
        let config = BinaryAuditConfig::standard(TargetPlatform::Darwin);
        // A "perfect" keuos binary disassembly
        let perfect_disasm = "ldr x3, [x0]\nbr x3\nmov x16, #0x50\nsvc #0x80\nret";
        let results = run_audit(&config, perfect_disasm);

        assert_eq!(results.len(), 3, "Standard audit has 3 rules");
        assert!(results[0].passed, "NoX19Spill should pass for clean disasm");
        assert!(results[1].passed, "HasTailCall should pass with br x3");
        assert!(results[2].passed, "HasIoSyscall should pass with svc");
    }
}
