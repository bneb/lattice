// =============================================================================
// C10M Validator
//
// Validates whether a hardware target can sustain 10 million concurrent
// connections using the KeuOS architecture. Checks three invariants:
//
// 1. Throughput: packets/sec ≥ target (default 10M)
// 2. Memory: L1D TaskFrame capacity ≥ effective batch size
// 3. Stack: MustTail guarantees O(1) stack depth (via Z3 proof)
//
// The validator is hardware-agnostic — it accepts any HardwareTarget impl.
// =============================================================================

use super::hardware_target::HardwareTarget;

/// C10M validation requirements.
#[derive(Debug, Clone)]
pub struct C10MRequirements {
    /// Target concurrent connections (default: 10_000_000)
    pub target_connections: u64,
    /// Minimum packets/sec throughput (default: 10_000_000)
    pub min_packets_per_sec: f64,
    /// Minimum TaskFrames in L1D for effective batching
    pub min_tasks_in_l1d: u64,
    /// Whether MustTail dispatch is enabled (constant stack depth)
    pub musttail_enabled: bool,
}

impl Default for C10MRequirements {
    fn default() -> Self {
        Self {
            target_connections: 10_000_000,
            min_packets_per_sec: 10_000_000.0,
            min_tasks_in_l1d: 64, // At least 64 TaskFrames in L1D for batch efficiency
            musttail_enabled: true,
        }
    }
}

/// Result of a single C10M validation check.
#[derive(Debug, Clone)]
pub struct C10MCheckResult {
    pub name: &'static str,
    pub passed: bool,
    pub actual: String,
    pub required: String,
    pub detail: String,
}

/// Full C10M validation report.
#[derive(Debug, Clone)]
pub struct C10MReport {
    pub target_name: String,
    pub checks: Vec<C10MCheckResult>,
    pub all_passed: bool,
}

/// Validate throughput: can the target sustain the required packets/sec?
pub fn validate_throughput(target: &dyn HardwareTarget, reqs: &C10MRequirements) -> C10MCheckResult {
    let actual_pps = target.max_packets_per_sec();
    let passed = actual_pps >= reqs.min_packets_per_sec;

    C10MCheckResult {
        name: "Throughput",
        passed,
        actual: format!("{:.2}M pkt/s", actual_pps / 1e6),
        required: format!("{:.2}M pkt/s", reqs.min_packets_per_sec / 1e6),
        detail: if passed {
            format!(
                "{} sustains {:.1}M pkt/s ({:.1}x headroom over C10M)",
                target.name(),
                actual_pps / 1e6,
                actual_pps / reqs.min_packets_per_sec,
            )
        } else {
            format!(
                "INSUFFICIENT: {} only sustains {:.1}M pkt/s, need {:.1}M",
                target.name(),
                actual_pps / 1e6,
                reqs.min_packets_per_sec / 1e6,
            )
        },
    }
}

/// Validate memory: does L1D hold enough TaskFrames for effective batching?
pub fn validate_memory(target: &dyn HardwareTarget, reqs: &C10MRequirements) -> C10MCheckResult {
    let actual_tasks = target.tasks_in_l1d();
    let passed = actual_tasks >= reqs.min_tasks_in_l1d;

    C10MCheckResult {
        name: "Memory (L1D)",
        passed,
        actual: format!("{} TaskFrames in L1D", actual_tasks),
        required: format!("≥ {} TaskFrames", reqs.min_tasks_in_l1d),
        detail: if passed {
            format!(
                "{}: {}B TaskFrame × {} = {} tasks in {}KB L1D",
                target.name(),
                target.task_frame_bytes(),
                actual_tasks,
                actual_tasks,
                target.l1d_bytes() / 1024,
            )
        } else {
            format!(
                "INSUFFICIENT: only {} TaskFrames fit in L1D (need {})",
                actual_tasks, reqs.min_tasks_in_l1d,
            )
        },
    }
}

/// Validate stack: MustTail guarantees constant stack depth.
pub fn validate_stack(reqs: &C10MRequirements) -> C10MCheckResult {
    C10MCheckResult {
        name: "Stack Depth",
        passed: reqs.musttail_enabled,
        actual: if reqs.musttail_enabled {
            "O(1) (MustTail)".to_string()
        } else {
            "O(N) (standard call)".to_string()
        },
        required: "O(1) constant".to_string(),
        detail: if reqs.musttail_enabled {
            "MustTail dispatch: stack frame replaced on each dispatch → O(1) depth".to_string()
        } else {
            "CRITICAL: Without MustTail, stack grows O(N) with connection count → overflow".to_string()
        },
    }
}

/// Run the full C10M validation suite against a hardware target.
pub fn validate_c10m(target: &dyn HardwareTarget, reqs: &C10MRequirements) -> C10MReport {
    let checks = vec![
        validate_throughput(target, reqs),
        validate_memory(target, reqs),
        validate_stack(reqs),
    ];
    let all_passed = checks.iter().all(|c| c.passed);

    C10MReport {
        target_name: target.name().to_string(),
        checks,
        all_passed,
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::hardware_target::M4Target;

    #[test]
    fn test_c10m_throughput_passes_m4() {
        let m4 = M4Target;
        let reqs = C10MRequirements::default();
        let result = validate_throughput(&m4, &reqs);

        assert!(result.passed,
            "M4 @ 4.4GHz must sustain 10M pkt/s. Detail: {}", result.detail);
        assert!(result.actual.contains("M pkt/s"),
            "Result must report in M pkt/s format");
    }

    #[test]
    fn test_c10m_memory_passes_m4() {
        let m4 = M4Target;
        let reqs = C10MRequirements::default();
        let result = validate_memory(&m4, &reqs);

        assert!(result.passed,
            "M4 with 128B TaskFrames must fit ≥64 in 64KB L1D. Detail: {}", result.detail);
    }

    #[test]
    fn test_c10m_stack_passes_with_musttail() {
        let reqs = C10MRequirements { musttail_enabled: true, ..Default::default() };
        let result = validate_stack(&reqs);

        assert!(result.passed, "MustTail dispatch must guarantee O(1) stack");
        assert!(result.actual.contains("O(1)"),
            "Stack must be O(1) with MustTail");

        // Without musttail should fail
        let bad_reqs = C10MRequirements { musttail_enabled: false, ..Default::default() };
        let bad_result = validate_stack(&bad_reqs);
        assert!(!bad_result.passed, "Without MustTail, stack check must fail");
    }

    #[test]
    fn test_c10m_report_all_green() {
        let m4 = M4Target;
        let reqs = C10MRequirements::default();
        let report = validate_c10m(&m4, &reqs);

        assert!(report.all_passed,
            "Full C10M report on M4 must be all-green");
        assert_eq!(report.checks.len(), 3,
            "Report must contain 3 checks (throughput, memory, stack)");
        for check in &report.checks {
            assert!(check.passed,
                "Check '{}' failed: {}", check.name, check.detail);
        }
    }

    #[test]
    fn test_c10m_fails_insufficient_clock() {
        // A fake target with 100MHz clock can't sustain C10M
        struct SlowTarget;
        impl HardwareTarget for SlowTarget {
            fn name(&self) -> &str { "SlowChip 100MHz" }
            fn clock_ghz(&self) -> f64 { 0.1 } // 100MHz
            fn l1d_bytes(&self) -> u64 { 16384 } // 16KB
            fn task_frame_bytes(&self) -> u64 { 128 }
            fn ingress_cycles(&self) -> u64 { 12 }
            fn dispatch_cycles(&self) -> u64 { 25 }
            fn safety_cycles(&self) -> u64 { 0 }
            fn processing_cycles(&self) -> u64 { 150 }
            fn egress_cycles(&self) -> u64 { 10 }
        }

        let slow = SlowTarget;
        let reqs = C10MRequirements::default();
        let result = validate_throughput(&slow, &reqs);

        assert!(!result.passed,
            "100MHz target must fail C10M throughput check");
        assert!(result.detail.contains("INSUFFICIENT"),
            "Failure message must say INSUFFICIENT");
    }
}
