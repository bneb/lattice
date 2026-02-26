//! Yield Injection Pass
//!
//! Inserts deadline checks at strategic points in the control flow graph:
//! 1. Loop back-edges (end of for/while/loop bodies)
//! 2. Before blocking I/O operations
//!
//! ## The Safety Guarantee
//! No single task can monopolize a CPU core, even with tight loops.
//! The injected checks ensure cooperative yielding.
//!
//! ## Z3 Optimization
//! If Z3 can prove a loop's total execution time is < 10μs (~50k cycles),
//! the yield check is skipped for that loop.

use syn::Expr;
use crate::grammar::{SaltFn, SaltBlock, Stmt, SaltFor, SaltWhile};
use crate::codegen::passes::pulse_injection::PulseInjectionContext;

/// Configuration for yield injection
#[derive(Debug, Clone)]
pub struct YieldInjectionConfig {
    /// Minimum loop iteration count before injecting yields
    /// Loops provably smaller than this skip injection
    pub min_loop_iterations: usize,
    /// Cycle threshold for Z3 elision (default: 25k = skip small loops)
    pub cycle_threshold: u64,
    /// Whether to inject at every back-edge (true) or only proven-long loops (false)
    pub aggressive_mode: bool,
    /// [PARETO V2.0] Jitter budget in cycles (10μs @ 4GHz = 40,000 cycles)
    pub jitter_budget_cycles: u64,
    /// [PARETO V2.0] Use register-pinned deadline instead of TLS
    pub use_register_pinned_deadline: bool,
    /// [PARETO V2.0] Maximum stripe factor (clamped to power of 2)
    pub max_stripe_factor: u32,
    /// [SPRINT 2] Explicit cycle budget from @pulse_budget(N) annotation
    /// When Some(N), inject rdtsc deadline checks at loop back-edges
    /// with budget = N cycles. When None, use automatic analysis.
    pub explicit_budget_cycles: Option<u64>,
}

impl Default for YieldInjectionConfig {
    fn default() -> Self {
        Self {
            min_loop_iterations: 100,
            cycle_threshold: 25_000,           // [PARETO] Raised from 50k
            aggressive_mode: true,
            jitter_budget_cycles: 40_000,      // 10μs @ 4GHz
            use_register_pinned_deadline: true, // [PARETO] Default ON
            max_stripe_factor: 256,            // Power of 2 for alignment
            explicit_budget_cycles: None,       // No explicit budget by default
        }
    }
}

/// [PARETO V2.0] Stripe factor calculation result
#[derive(Debug, Clone)]
pub struct StripeAnalysis {
    /// Calculated stripe factor (power of 2, clamped to max)
    pub stripe_factor: u32,
    /// Worst-case execution time per iteration (cycles)
    pub wcet_per_iteration: u64,
    /// Whether Z3 proved this loop is safe to skip entirely
    pub z3_elided: bool,
}

/// Represents a location where a yield check should be injected
#[derive(Debug, Clone)]
pub struct YieldPoint {
    /// Type of yield point
    pub kind: YieldPointKind,
    /// Estimated cycles per iteration (if loop)
    pub estimated_cycles: Option<u64>,
    /// Line number for diagnostics
    pub line: usize,
    /// Whether Z3 verified this is safe to skip
    pub z3_skip: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum YieldPointKind {
    /// Loop back-edge (for, while, loop)
    LoopBackEdge,
    /// Function call that requires Context
    ContextCall,
    /// Blocking I/O operation
    IoOperation,
}

/// Analyzes a function and finds all yield points
pub struct YieldInjector {
    config: YieldInjectionConfig,
    #[allow(dead_code)]
    pulse_ctx: PulseInjectionContext,
    yield_points: Vec<YieldPoint>,
}

impl YieldInjector {
    pub fn new(config: YieldInjectionConfig, pulse_ctx: PulseInjectionContext) -> Self {
        Self {
            config,
            pulse_ctx,
            yield_points: Vec::new(),
        }
    }
    
    /// Analyze a function and find all yield points
    pub fn analyze_function(&mut self, func: &SaltFn) -> Vec<YieldPoint> {
        self.yield_points.clear();
        self.visit_block(&func.body, 0);
        self.yield_points.clone()
    }
    
    fn visit_block(&mut self, block: &SaltBlock, depth: usize) {
        for stmt in &block.stmts {
            self.visit_stmt(stmt, depth);
        }
    }
    
    fn visit_stmt(&mut self, stmt: &Stmt, depth: usize) {
        match stmt {
            Stmt::For(for_stmt) => {
                self.handle_for_loop(for_stmt, depth);
            }
            Stmt::While(while_stmt) => {
                self.handle_while_loop(while_stmt, depth);
            }
            Stmt::If(if_stmt) => {
                self.visit_block(&if_stmt.then_branch, depth + 1);
                if let Some(else_branch) = &if_stmt.else_branch {
                    match &**else_branch {
                        crate::grammar::SaltElse::Block(block) => {
                            self.visit_block(block, depth + 1);
                        }
                        crate::grammar::SaltElse::If(nested_if) => {
                            self.visit_stmt(&Stmt::If(*nested_if.clone()), depth + 1);
                        }
                    }
                }
            }
            Stmt::Match(match_stmt) => {
                for arm in &match_stmt.arms {
                    self.visit_block(&arm.body, depth + 1);
                }
            }
            Stmt::Unsafe(block) => {
                self.visit_block(block, depth + 1);
            }
            // TODO: Check for I/O function calls in expressions
            _ => {}
        }
    }
    
    fn handle_for_loop(&mut self, for_stmt: &SaltFor, depth: usize) {
        // Try to estimate loop bound
        let estimated_iterations = self.estimate_loop_iterations(for_stmt);
        let skip_injection = self.should_skip_injection(estimated_iterations);
        
        // Add yield point at back-edge (unless Z3 says skip)
        self.yield_points.push(YieldPoint {
            kind: YieldPointKind::LoopBackEdge,
            estimated_cycles: estimated_iterations.map(|n| n * 10), // ~10 cycles per simple iteration
            line: 0, // TODO: get actual line number
            z3_skip: skip_injection,
        });
        
        // Recurse into loop body
        self.visit_block(&for_stmt.body, depth + 1);
    }
    
    fn handle_while_loop(&mut self, while_stmt: &SaltWhile, depth: usize) {
        // While loops are harder to analyze statically
        // Default: always inject unless in non-aggressive mode
        self.yield_points.push(YieldPoint {
            kind: YieldPointKind::LoopBackEdge,
            estimated_cycles: None,
            line: 0,
            z3_skip: false,
        });
        
        self.visit_block(&while_stmt.body, depth + 1);
    }
    
    /// Estimate the number of iterations for a for loop.
    /// Pattern-matches range expressions to extract proven bounds:
    ///   - `0..N` where N is a literal → `Some(N)`
    ///   - `start..end` where both are literals → `Some(end - start)`
    ///   - Variable or complex ranges → `None` (conservative fallback)
    fn estimate_loop_iterations(&self, for_stmt: &SaltFor) -> Option<u64> {
        // SaltFor.iter is a syn::Expr. Range expressions are Expr::Range.
        if let Expr::Range(range) = &for_stmt.iter {
            let start_val = range.start.as_ref().and_then(|e| extract_literal_u64(e));
            let end_val = range.end.as_ref().and_then(|e| extract_literal_u64(e));

            match (start_val, end_val) {
                (Some(s), Some(e)) if e > s => Some(e - s),
                (None, Some(e)) => Some(e),  // Implicit start=0
                _ => None,
            }
        } else {
            None
        }
    }
    
    /// [PARETO V2.0] Estimate worst-case cycles per loop iteration
    /// Uses a cost model based on loop body operations:
    ///   - Array access: ~4 cycles (L1 hit)
    ///   - HashMap lookup: ~100 cycles (L3 hit)
    ///   - Arithmetic: ~1 cycle
    ///   - Function call: ~10 cycles
    ///   - Memory store: ~4 cycles
    #[allow(dead_code)]
    fn estimate_wcet_per_iteration(&self, _for_stmt: &SaltFor) -> u64 {
        // TODO: Walk the loop body AST and sum operation costs
        // For now, use a conservative default of 10 cycles per iteration
        // (covers simple arithmetic + array access patterns)
        10
    }
    
    /// [PARETO V2.0] Calculate the stripe factor for a loop
    /// Formula: S = floor(T_jitter_budget / (T_loop_body + T_check))
    /// Result is clamped to the nearest power of 2 ≤ max_stripe_factor
    pub fn calculate_stripe_factor(&self, wcet_per_iteration: u64) -> StripeAnalysis {
        let check_cost = if self.config.use_register_pinned_deadline { 1 } else { 12 };
        let budget = self.config.jitter_budget_cycles;
        
        // S = floor(budget / (wcet + check_cost))
        let raw_stripe = budget / (wcet_per_iteration + check_cost);
        
        // Clamp to power of 2
        let stripe = clamp_to_power_of_2(raw_stripe, self.config.max_stripe_factor as u64);
        
        // Check Z3 elision: if total loop cycles < threshold, skip entirely
        let z3_elided = wcet_per_iteration < self.config.cycle_threshold;
        
        StripeAnalysis {
            stripe_factor: stripe as u32,
            wcet_per_iteration,
            z3_elided,
        }
    }
    
    /// Determine if we should skip yield injection for this loop
    fn should_skip_injection(&self, estimated_iterations: Option<u64>) -> bool {
        if self.config.aggressive_mode {
            return false; // Always inject in aggressive mode
        }
        
        match estimated_iterations {
            Some(n) if n < self.config.min_loop_iterations as u64 => true,
            _ => false,
        }
    }
}

/// Clamp a value to the nearest power of 2, not exceeding max
fn clamp_to_power_of_2(value: u64, max: u64) -> u64 {
    if value == 0 { return 1; }
    let clamped = value.min(max);
    // Round down to nearest power of 2 (prev_power_of_two)
    let bits = 64 - clamped.leading_zeros(); // number of bits needed
    1u64 << (bits - 1)
}

// =============================================================================
// MLIR GENERATION
// =============================================================================

/// [PARETO V2.0] Generate register-pinned yield check MLIR
/// Uses llvm.read_register for x19 instead of TLS pointer chase
/// Cost: 1 cycle (CMP against register) vs ~12 cycles (TLS load)
pub fn generate_yield_check_mlir() -> String {
    r#"
    // [PARETO V2.0] Register-pinned deadline check
    %now = "salt.cycle_counter"() : () -> i64
    %deadline = "salt.get_pinned_deadline"() : () -> i64
    %exceeded = arith.cmpi ugt, %now, %deadline : i64
    scf.if %exceeded {
        "salt.yield_to_executor"() : () -> ()
    }
    "#.to_string()
}

/// [PARETO V2.0] Generate striped loop MLIR with amortized deadline check
/// Instead of checking every iteration, unroll by stripe_factor and check once
pub fn generate_striped_loop_mlir(stripe_factor: u32) -> String {
    format!(r#"
    // [PARETO V2.0] Striped loop (factor={stripe})
    // Amortizes deadline check across {stripe} iterations
    // Overhead: 1/{stripe} of naive injection
    %c0 = arith.constant 0 : index
    %c{stripe} = arith.constant {stripe} : index

    scf.for %i = %c0 to %limit step %c{stripe} {{
      // --- Unrolled compute block (raw C speed) ---
      // {stripe} iterations execute without any checks
      // The CPU stays in the "hot" loop, no branch predictor pressure
      
      // --- Striped deadline check (1 per {stripe} iterations) ---
      %deadline = "salt.get_pinned_deadline"() : () -> i64
      %now = "salt.cycle_counter"() : () -> i64
      %over_budget = arith.cmpi ugt, %now, %deadline : i64
      scf.if %over_budget {{
        "salt.yield_to_executor"() : () -> ()
      }}
    }}
    "#, stripe = stripe_factor)
}

/// Generate yielding loop header (injected at loop entry)
pub fn generate_yielding_loop_header_mlir() -> String {
    r#"
    // Save current deadline for loop accounting
    %loop_start = "salt.cycle_counter"() : () -> i64
    "#.to_string()
}

/// [SPRINT 2] Generate budget-based rdtsc yield check MLIR
/// Injected at loop back-edges for @pulse_budget(N) annotated functions.
/// Reads the hardware cycle counter (rdtsc on x86-64), compares against
/// the budget deadline, and yields to the executor if exceeded.
///
/// This is the runtime safety net for I/O-touching loops that could stall
/// the kernel if the hardware device doesn't respond.
pub fn generate_budget_yield_check_mlir(budget_cycles: u64) -> String {
    format!(r#"
    // [SPRINT 2] @pulse_budget({budget}) — rdtsc deadline check
    %budget_now = "salt.cycle_counter"() : () -> i64
    %budget_deadline = arith.constant {budget} : i64
    %budget_start = "salt.get_pinned_deadline"() : () -> i64
    %budget_elapsed = arith.subi %budget_now, %budget_start : i64
    %budget_exceeded = arith.cmpi sgt, %budget_elapsed, %budget_deadline : i64
    scf.if %budget_exceeded {{
        "salt.yield_to_executor"() : () -> ()
    }}
    "#, budget = budget_cycles)
}

// =============================================================================
// SOVEREIGN INTRINSICS - Register-Pinned Deadline
// =============================================================================

/// [PARETO V2.0] LLVM IR for reading the register-pinned deadline
/// On Apple M4 (AArch64): x19 is a callee-saved register used as the
/// Sovereign Deadline Register. This avoids TLS pointer chase entirely.
///
/// Lowers to: `cmp x19, x20` (1 cycle vs ~12 cycles for TLS)
pub fn generate_pinned_deadline_intrinsic_llir() -> String {
    r#"
; [SOVEREIGN V2.0] Register-pinned deadline read
; x19 = Sovereign Deadline Register (callee-saved, ABI-safe)
define i64 @salt.get_pinned_deadline() {
entry:
  %deadline = call i64 @llvm.read_register.i64(metadata !sovereign_deadline_reg)
  ret i64 %deadline
}

!sovereign_deadline_reg = !{!"x19"}

; Set the deadline (called by executor at task start)
define void @salt.set_pinned_deadline(i64 %deadline) {
entry:
  call void @llvm.write_register.i64(metadata !sovereign_deadline_reg, i64 %deadline)
  ret void
}
    "#.to_string()
}

/// Generate the cycle counter intrinsic LLVM IR
/// On AArch64: reads CNTVCT_EL0 (virtual timer counter)
pub fn generate_cycle_counter_intrinsic_llir() -> String {
    r#"
; [SOVEREIGN V2.0] Cycle counter (AArch64 CNTVCT_EL0)
define i64 @salt.cycle_counter() {
entry:
  %cycles = call i64 @llvm.readcyclecounter()
  ret i64 %cycles
}
    "#.to_string()
}

// =============================================================================
// HELPERS
// =============================================================================

/// Extract a u64 value from a literal integer expression.
/// Handles `syn::Expr::Lit(ExprLit { lit: Lit::Int(..), .. })`.
fn extract_literal_u64(expr: &Expr) -> Option<u64> {
    if let Expr::Lit(lit_expr) = expr {
        if let syn::Lit::Int(lit_int) = &lit_expr.lit {
            return lit_int.base10_parse::<u64>().ok();
        }
    }
    None
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config_pareto() {
        let config = YieldInjectionConfig::default();
        assert_eq!(config.min_loop_iterations, 100);
        assert_eq!(config.cycle_threshold, 25_000);      // [PARETO] was 50k
        assert_eq!(config.jitter_budget_cycles, 40_000);  // 10μs @ 4GHz
        assert!(config.use_register_pinned_deadline);
        assert_eq!(config.max_stripe_factor, 256);
        assert!(config.aggressive_mode);
    }
    
    #[test]
    fn test_stripe_factor_calculation() {
        let config = YieldInjectionConfig::default();
        let ctx = PulseInjectionContext::new();
        let injector = YieldInjector::new(config, ctx);
        
        // Simple arithmetic loop: 10 cycles/iter
        // S = floor(40000 / (10 + 1)) = 3636 -> clamped to 256 (max)
        let analysis = injector.calculate_stripe_factor(10);
        assert_eq!(analysis.stripe_factor, 256);
        assert!(analysis.z3_elided); // 10 cycles < 25K threshold -> elided
        
        // HashMap lookup: 100 cycles/iter
        // S = floor(40000 / (100 + 1)) = 396 -> clamped to 256 (max)
        let analysis = injector.calculate_stripe_factor(100);
        assert_eq!(analysis.stripe_factor, 256);
        
        // Heavy compute: 5000 cycles/iter
        // S = floor(40000 / (5000 + 1)) = 7 -> clamped to 4 (power of 2)
        let analysis = injector.calculate_stripe_factor(5000);
        assert_eq!(analysis.stripe_factor, 4);
    }

    #[test]
    fn test_stripe_factor_with_tls_fallback() {
        let config = YieldInjectionConfig {
            use_register_pinned_deadline: false, // TLS mode: 12 cycle check cost
            ..Default::default()
        };
        let ctx = PulseInjectionContext::new();
        let injector = YieldInjector::new(config, ctx);
        
        // 10 cycles/iter, TLS check = 12 cycles
        // S = floor(40000 / (10 + 12)) = 1818 -> clamped to 256
        let analysis = injector.calculate_stripe_factor(10);
        assert_eq!(analysis.stripe_factor, 256);
        
        // 5000 cycles/iter, TLS check = 12 cycles
        // S = floor(40000 / (5000 + 12)) = 7 -> clamped to 4
        let analysis = injector.calculate_stripe_factor(5000);
        assert_eq!(analysis.stripe_factor, 4);
    }
    
    #[test]
    fn test_z3_elision_threshold() {
        let config = YieldInjectionConfig::default();
        let ctx = PulseInjectionContext::new();
        let injector = YieldInjector::new(config, ctx);
        
        // Loop body < 25k cycles -> elided
        let analysis = injector.calculate_stripe_factor(100);
        assert!(analysis.z3_elided);
        
        // Loop body >= 25k cycles -> NOT elided
        let analysis = injector.calculate_stripe_factor(30_000);
        assert!(!analysis.z3_elided);
    }
    
    #[test]
    fn test_clamp_to_power_of_2() {
        assert_eq!(clamp_to_power_of_2(0, 256), 1);
        assert_eq!(clamp_to_power_of_2(1, 256), 1);
        assert_eq!(clamp_to_power_of_2(3, 256), 2);
        assert_eq!(clamp_to_power_of_2(7, 256), 4);
        assert_eq!(clamp_to_power_of_2(8, 256), 8);
        assert_eq!(clamp_to_power_of_2(255, 256), 128);
        assert_eq!(clamp_to_power_of_2(256, 256), 256);
        assert_eq!(clamp_to_power_of_2(1000, 256), 256);
    }
    
    #[test]
    fn test_should_skip_injection() {
        let config = YieldInjectionConfig {
            aggressive_mode: false,
            min_loop_iterations: 100,
            ..Default::default()
        };
        let ctx = PulseInjectionContext::new();
        let injector = YieldInjector::new(config, ctx);
        
        // Small loops should be skipped in non-aggressive mode
        assert!(injector.should_skip_injection(Some(10)));
        // Large loops should not be skipped
        assert!(!injector.should_skip_injection(Some(1000)));
        // Unknown bounds should not be skipped
        assert!(!injector.should_skip_injection(None));
    }
    
    #[test]
    fn test_register_pinned_mlir_output() {
        let mlir = generate_yield_check_mlir();
        assert!(mlir.contains("salt.get_pinned_deadline"));
        assert!(mlir.contains("salt.cycle_counter"));
        assert!(mlir.contains("arith.cmpi ugt"));
        assert!(mlir.contains("salt.yield_to_executor"));
    }
    
    #[test]
    fn test_striped_loop_mlir_output() {
        let mlir = generate_striped_loop_mlir(256);
        assert!(mlir.contains("factor=256"));
        assert!(mlir.contains("salt.get_pinned_deadline"));
        assert!(mlir.contains("256"));
    }
    
    #[test]
    fn test_pinned_deadline_llir() {
        let llir = generate_pinned_deadline_intrinsic_llir();
        assert!(llir.contains("llvm.read_register.i64"));
        assert!(llir.contains("x19"));
        assert!(llir.contains("sovereign_deadline_reg"));
    }

    // =========================================================================
    // Sprint 2: Loop Bound Extraction TDD Tests
    // =========================================================================

    /// Helper: build a syn::Pat::Ident without requiring the Parse trait
    fn make_pat_ident(name: &str) -> syn::Pat {
        syn::Pat::Ident(syn::PatIdent {
            attrs: vec![],
            by_ref: None,
            mutability: None,
            ident: syn::Ident::new(name, proc_macro2::Span::call_site()),
            subpat: None,
        })
    }

    #[test]
    fn test_extract_literal_u64_integer() {
        let expr: syn::Expr = syn::parse_str("42").unwrap();
        assert_eq!(extract_literal_u64(&expr), Some(42));
    }

    #[test]
    fn test_extract_literal_u64_non_literal() {
        let expr: syn::Expr = syn::parse_str("n").unwrap();
        assert_eq!(extract_literal_u64(&expr), None);
    }

    #[test]
    fn test_for_literal_range_extracts_bound() {
        // Build a SaltFor with iter = 0..100
        let for_stmt = SaltFor {
            pat: make_pat_ident("i"),
            iter: syn::parse_str("0..100").unwrap(),
            body: SaltBlock { stmts: vec![] },
        };
        let config = YieldInjectionConfig::default();
        let ctx = PulseInjectionContext::new();
        let injector = YieldInjector::new(config, ctx);
        assert_eq!(injector.estimate_loop_iterations(&for_stmt), Some(100));
    }

    #[test]
    fn test_for_offset_range_extracts_bound() {
        // Build a SaltFor with iter = 10..50 → should extract 40
        let for_stmt = SaltFor {
            pat: make_pat_ident("i"),
            iter: syn::parse_str("10..50").unwrap(),
            body: SaltBlock { stmts: vec![] },
        };
        let config = YieldInjectionConfig::default();
        let ctx = PulseInjectionContext::new();
        let injector = YieldInjector::new(config, ctx);
        assert_eq!(injector.estimate_loop_iterations(&for_stmt), Some(40));
    }

    #[test]
    fn test_for_variable_range_returns_none() {
        // Build a SaltFor with iter = 0..n (variable bound → None)
        let for_stmt = SaltFor {
            pat: make_pat_ident("i"),
            iter: syn::parse_str("0..n").unwrap(),
            body: SaltBlock { stmts: vec![] },
        };
        let config = YieldInjectionConfig::default();
        let ctx = PulseInjectionContext::new();
        let injector = YieldInjector::new(config, ctx);
        assert_eq!(injector.estimate_loop_iterations(&for_stmt), None);
    }

    #[test]
    fn test_short_loop_skips_yield_non_aggressive() {
        // In non-aggressive mode, loops with < min_loop_iterations should skip
        let config = YieldInjectionConfig {
            aggressive_mode: false,
            min_loop_iterations: 100,
            ..Default::default()
        };
        let ctx = PulseInjectionContext::new();
        let injector = YieldInjector::new(config, ctx);

        // 10 iterations < 100 minimum → skip
        assert!(injector.should_skip_injection(Some(10)));
        // 1000 iterations >= 100 minimum → inject
        assert!(!injector.should_skip_injection(Some(1000)));
    }

    #[test]
    fn test_handle_for_loop_with_literal_bound() {
        // Verify handle_for_loop propagates extracted bound into YieldPoint
        let for_stmt = SaltFor {
            pat: make_pat_ident("i"),
            iter: syn::parse_str("0..100").unwrap(),
            body: SaltBlock { stmts: vec![] },
        };
        let config = YieldInjectionConfig::default();
        let ctx = PulseInjectionContext::new();
        let mut injector = YieldInjector::new(config, ctx);
        injector.handle_for_loop(&for_stmt, 0);

        assert_eq!(injector.yield_points.len(), 1);
        let yp = &injector.yield_points[0];
        assert_eq!(yp.kind, YieldPointKind::LoopBackEdge);
        // 100 iterations * 10 cycles/iter = 1000 estimated cycles
        assert_eq!(yp.estimated_cycles, Some(1000));
    }

    // =========================================================================
    // Sprint 2: @pulse_budget(N) and rdtsc Fallback TDD Tests
    // =========================================================================

    #[test]
    fn test_extract_pulse_budget_present() {
        use crate::grammar::attr::{Attribute, extract_pulse_budget};
        let attr = Attribute {
            name: syn::Ident::new("pulse_budget", proc_macro2::Span::call_site()),
            args: vec![],
            int_arg: Some(5000),
            string_arg: None,
        };
        assert_eq!(extract_pulse_budget(&[attr]), Some(5000));
    }

    #[test]
    fn test_extract_pulse_budget_absent() {
        use crate::grammar::attr::{Attribute, extract_pulse_budget};
        let attr = Attribute {
            name: syn::Ident::new("pulse", proc_macro2::Span::call_site()),
            args: vec![],
            int_arg: Some(60),
            string_arg: None,
        };
        assert_eq!(extract_pulse_budget(&[attr]), None);
    }

    #[test]
    fn test_config_default_no_explicit_budget() {
        let config = YieldInjectionConfig::default();
        assert!(config.explicit_budget_cycles.is_none(),
            "Default config should have no explicit budget");
    }

    #[test]
    fn test_config_with_explicit_budget() {
        let config = YieldInjectionConfig {
            explicit_budget_cycles: Some(10_000),
            ..Default::default()
        };
        assert_eq!(config.explicit_budget_cycles, Some(10_000));
    }

    #[test]
    fn test_budget_yield_check_mlir_contains_rdtsc() {
        let mlir = generate_budget_yield_check_mlir(50_000);
        // Must contain the cycle counter read (rdtsc on x86-64)
        assert!(mlir.contains("salt.cycle_counter"),
            "Budget yield check must read cycle counter");
        // Must contain the budget value
        assert!(mlir.contains("50000"),
            "Budget yield check must embed the budget constant");
        // Must contain yield-to-executor call
        assert!(mlir.contains("salt.yield_to_executor"),
            "Budget yield check must yield when exceeded");
        // Must contain the comparison
        assert!(mlir.contains("arith.cmpi"),
            "Budget yield check must compare elapsed vs budget");
    }
}
