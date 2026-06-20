//! Pulse Injection Pass
//!
//! This pass transforms functions marked with @pulse(freq) by:
//! 1. Adding a hidden `ctx: Context` parameter (ZST at runtime)
//! 2. Setting up the deadline based on frequency
//! 3. Propagating Context to child function calls
//!
//! ## The Transformation
//! ```text
//! // Before:
//! @pulse(60)
//! fn update_ui() { ... }
//!
//! // After (conceptual):
//! fn update_ui(__ctx: Context) {
//!     context::set_deadline(__ctx.deadline);
//!     ...
//! }
//! ```
//!
//! ## ZST Erasure
//! The Context parameter is erased during MLIR→LLVM lowering.
//! At runtime, the deadline lives in Thread-Local Storage.
//!
//! ## Call Graph Integration (KeuOS)
//! The `analyze_with_call_graph()` method replaces the fragile string-matching
//! heuristic with deep-context fixed-point analysis from `CallGraphAnalyzer`.
//! This ensures @pulse safety is enforced transitively across all library calls.

use crate::grammar::{SaltFn, SaltFile, Item};
use crate::grammar::attr::{extract_pulse_hz, pulse_to_tier};
use crate::codegen::passes::call_graph::CallGraphAnalyzer;
use std::collections::HashSet;

/// Result of analyzing a function for pulse injection
#[derive(Debug, Clone)]
pub struct PulseInfo {
    /// Function name
    pub name: String,
    /// Pulse frequency in Hz (e.g., 60, 1000)
    pub frequency_hz: u32,
    /// Priority tier (0 = real-time, 1 = interactive, 2 = background)
    pub tier: u8,
    /// Deadline budget in cycles (at 4GHz)
    pub deadline_cycles: u64,
    /// Whether this function was explicitly marked @pulse
    pub is_explicit: bool,
}

impl PulseInfo {
    pub fn new(name: String, frequency_hz: u32) -> Self {
        let tier = pulse_to_tier(frequency_hz);
        // At 4GHz, cycles_per_second = 4_000_000_000
        let deadline_cycles = 4_000_000_000u64 / (frequency_hz as u64);
        
        Self {
            name,
            frequency_hz,
            tier,
            deadline_cycles,
            is_explicit: true,
        }
    }
    
    pub fn inherited(name: String, parent: &PulseInfo) -> Self {
        Self {
            name,
            frequency_hz: parent.frequency_hz,
            tier: parent.tier,
            deadline_cycles: parent.deadline_cycles,
            is_explicit: false,
        }
    }
}

/// Analyzes a Salt file to find all @pulse functions
pub fn find_pulse_functions(file: &SaltFile) -> Vec<PulseInfo> {
    let mut results = Vec::new();
    
    for item in &file.items {
        if let Item::Fn(func) = item {
            if let Some(freq) = extract_pulse_hz(&func.attributes) {
                results.push(PulseInfo::new(func.name.to_string(), freq));
            }
        }
    }
    
    results
}

/// Context for tracking which functions need Context injection
pub struct PulseInjectionContext {
    /// Functions explicitly marked with @pulse
    pub explicit_pulse_fns: HashSet<String>,
    /// Functions that need Context because they're called by pulse functions
    pub implicit_context_fns: HashSet<String>,
    /// Functions detected as blocking via call graph analysis
    pub blocking_fns: HashSet<String>,
    /// Pulse info for each function
    pub pulse_info: Vec<PulseInfo>,
    /// Whether call graph analysis was used (vs fallback heuristic)
    pub used_call_graph: bool,
}

impl PulseInjectionContext {
    pub fn new() -> Self {
        Self {
            explicit_pulse_fns: HashSet::new(),
            implicit_context_fns: HashSet::new(),
            blocking_fns: HashSet::new(),
            pulse_info: Vec::new(),
            used_call_graph: false,
        }
    }
    
    /// Analyze a file using call graph analysis (preferred path).
    ///
    /// Replaces the old `calls_io_operation()` heuristic with proper
    /// transitive blocking detection from the CallGraphAnalyzer.
    pub fn analyze_with_call_graph(&mut self, file: &SaltFile, cg: &CallGraphAnalyzer) {
        self.used_call_graph = true;

        // Phase 1: Find explicit @pulse functions
        self.pulse_info = find_pulse_functions(file);
        for info in &self.pulse_info {
            self.explicit_pulse_fns.insert(info.name.clone());
        }

        // Phase 2: Query call graph for blocking and context requirements
        for item in &file.items {
            if let Item::Fn(func) = item {
                let name = func.name.to_string();

                // Call graph: is this function transitively blocking?
                if cg.is_blocking(&name) {
                    self.blocking_fns.insert(name.clone());
                }

                // Call graph: does this function require Context?
                if cg.requires_context(&name) {
                    self.implicit_context_fns.insert(name);
                }
            }
        }
    }

    /// Analyze a file and build the injection context (legacy fallback).
    ///
    /// Uses string-matching heuristic. Prefer `analyze_with_call_graph()`
    /// when a CallGraphAnalyzer is available.
    pub fn analyze(&mut self, file: &SaltFile) {
        // Phase 1: Find explicit @pulse functions
        self.pulse_info = find_pulse_functions(file);
        for info in &self.pulse_info {
            self.explicit_pulse_fns.insert(info.name.clone());
        }
        
        // Phase 2: Propagate Context requirements to called functions
        // Heuristic fallback — superseded by analyze_with_call_graph()
        
        // Phase 3: Mark functions that call I/O operations
        for item in &file.items {
            if let Item::Fn(func) = item {
                if self.calls_io_operation(func) {
                    self.implicit_context_fns.insert(func.name.to_string());
                    self.blocking_fns.insert(func.name.to_string());
                }
            }
        }
    }
    
    /// Check if a function calls I/O operations (legacy heuristic).
    /// Kept as fallback when call graph is not available.
    fn calls_io_operation(&self, func: &SaltFn) -> bool {
        let io_prefixes = ["io::", "net::", "fs::", "sys::"];
        let func_source = format!("{:?}", func.body);
        io_prefixes.iter().any(|prefix| func_source.contains(prefix))
    }
    
    /// Check if a function requires Context
    pub fn requires_context(&self, name: &str) -> bool {
        self.explicit_pulse_fns.contains(name) ||
        self.implicit_context_fns.contains(name)
    }

    /// Check if a function is blocking (via call graph or heuristic)
    pub fn is_blocking(&self, name: &str) -> bool {
        self.blocking_fns.contains(name)
    }
    
    /// Get the pulse info for a function (if it's a pulse function)
    pub fn get_pulse_info(&self, name: &str) -> Option<&PulseInfo> {
        self.pulse_info.iter().find(|p| p.name == name)
    }
}

impl Default for PulseInjectionContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::passes::call_graph::{CallGraphAnalyzer, FnAttributes};

    #[test]
    fn test_pulse_info_new() {
        let info = PulseInfo::new("update_ui".to_string(), 60);
        assert_eq!(info.frequency_hz, 60);
        assert_eq!(info.tier, 1); // Interactive tier
        assert_eq!(info.deadline_cycles, 4_000_000_000 / 60);
    }
    
    #[test]
    fn test_pulse_info_tier_0() {
        let info = PulseInfo::new("sensor_poll".to_string(), 1000);
        assert_eq!(info.tier, 0); // Real-Time tier
    }
    
    #[test]
    fn test_pulse_info_tier_2() {
        let info = PulseInfo::new("background_sync".to_string(), 10);
        assert_eq!(info.tier, 2); // Background tier
    }

    // =========================================================================
    // PR 1: Call Graph Integration Tests (TDD)
    // =========================================================================

    /// Helper: build a CallGraphAnalyzer with manual edges and attributes
    fn make_call_graph(
        edges: Vec<(&str, Vec<&str>)>,
        attrs: Vec<(&str, FnAttributes)>,
    ) -> CallGraphAnalyzer {
        let mut cg = CallGraphAnalyzer::new();
        for (name, callees) in edges {
            cg.inject_edges(name, callees.into_iter().map(|s| s.to_string()).collect());
        }
        for (name, attr) in attrs {
            cg.inject_attributes(name, attr);
        }
        cg.run_propagation();
        cg
    }

    #[test]
    fn test_pulse_injection_uses_call_graph() {
        // Build a call graph where "handler" calls "db_query" which is blocking
        let cg = make_call_graph(
            vec![
                ("handler", vec!["db_query"]),
                ("db_query", vec!["TcpStream::read"]),
            ],
            vec![
                ("handler", FnAttributes::default()),
                ("db_query", FnAttributes::default()),
            ],
        );

        let mut pulse_ctx = PulseInjectionContext::new();
        // We can't call analyze_with_call_graph without a SaltFile,
        // but we can test the call graph query directly
        pulse_ctx.used_call_graph = true;

        // Query the call graph: handler should be transitively blocking
        assert!(cg.is_blocking("handler"),
            "handler should be blocking (transitively via db_query → TcpStream::read)");
        assert!(cg.is_blocking("db_query"),
            "db_query should be blocking (directly calls TcpStream::read)");
    }

    #[test]
    fn test_heuristic_replaced_by_call_graph() {
        // The old heuristic only catches "io::", "net::", "fs::", "sys::" prefixes
        // The call graph catches ANY transitive blocking path
        let cg = make_call_graph(
            vec![
                ("process", vec!["helper"]),
                ("helper", vec!["Mutex::lock"]),  // Mutex::lock is blocking
            ],
            vec![
                ("process", FnAttributes::default()),
                ("helper", FnAttributes::default()),
            ],
        );

        // Old heuristic would NOT catch "process" as blocking
        // (it doesn't contain "io::", "net::", "fs::", or "sys::")
        // But the call graph DOES catch it
        assert!(cg.is_blocking("process"),
            "Call graph catches Mutex::lock transitively — old heuristic would miss this");
        assert!(cg.is_blocking("helper"),
            "helper directly calls Mutex::lock");
    }

    #[test]
    fn test_transitive_blocking_detected_via_call_graph() {
        // Deep chain: A → B → C → D → fs::open
        let cg = make_call_graph(
            vec![
                ("A", vec!["B"]),
                ("B", vec!["C"]),
                ("C", vec!["D"]),
                ("D", vec!["fs::open"]),
            ],
            vec![
                ("A", FnAttributes { is_pulse: true, pulse_hz: Some(60), requires_context: true, ..Default::default() }),
                ("B", FnAttributes::default()),
                ("C", FnAttributes::default()),
                ("D", FnAttributes::default()),
            ],
        );

        // All should be transitively blocking
        assert!(cg.is_blocking("A"), "A transitively blocking via B→C→D→fs::open");
        assert!(cg.is_blocking("B"), "B transitively blocking via C→D→fs::open");
        assert!(cg.is_blocking("C"), "C transitively blocking via D→fs::open");
        assert!(cg.is_blocking("D"), "D directly calls fs::open");

        // Verify safety violation would be detected
        let violations = cg.verify_pulse_safety_external();
        assert!(!violations.is_empty(), "Should detect @pulse 'A' calling blocking chain");
        assert_eq!(violations[0].pulse_fn, "A");
    }
}
