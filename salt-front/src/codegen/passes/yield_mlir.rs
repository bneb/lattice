use syn::Expr;

pub fn generate_yield_check_mlir() -> String {
    r#"
    // Register-pinned deadline check
    %now = "salt.cycle_counter"() : () -> i64
    %deadline = "salt.get_pinned_deadline"() : () -> i64
    %exceeded = arith.cmpi ugt, %now, %deadline : i64
    scf.if %exceeded {
        "salt.yield_to_executor"() : () -> ()
    }
    "#.to_string()
}

/// Generate striped loop MLIR with amortized deadline check
/// Instead of checking every iteration, unroll by stripe_factor and check once
pub fn generate_striped_loop_mlir(stripe_factor: u32) -> String {
    format!(r#"
    // Striped loop (factor={stripe})
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

/// Generate budget-based rdtsc yield check MLIR
/// Injected at loop back-edges for @pulse_budget(N) annotated functions.
/// Reads the hardware cycle counter (rdtsc on x86-64), compares against
/// the budget deadline, and yields to the executor if exceeded.
///
/// This is the runtime safety net for I/O-touching loops that could stall
/// the kernel if the hardware device doesn't respond.
pub fn generate_budget_yield_check_mlir(budget_cycles: u64) -> String {
    format!(r#"
    // @pulse_budget({budget}) — rdtsc deadline check
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
// KEUOS INTRINSICS - Register-Pinned Deadline
// =============================================================================

/// LLVM IR for reading the register-pinned deadline
/// On Apple M4 (AArch64): x19 is a callee-saved register used as the
/// KeuOS Deadline Register. This avoids TLS pointer chase entirely.
///
/// Lowers to: `cmp x19, x20` (1 cycle vs ~12 cycles for TLS)
pub fn generate_pinned_deadline_intrinsic_llir() -> String {
    r#"
; Register-pinned deadline read
; x19 = KeuOS Deadline Register (callee-saved, ABI-safe)
define i64 @salt.get_pinned_deadline() {
entry:
  %deadline = call i64 @llvm.read_register.i64(metadata !keuos_deadline_reg)
  ret i64 %deadline
}

!keuos_deadline_reg = !{!"x19"}

; Set the deadline (called by executor at task start)
define void @salt.set_pinned_deadline(i64 %deadline) {
entry:
  call void @llvm.write_register.i64(metadata !keuos_deadline_reg, i64 %deadline)
  ret void
}
    "#.to_string()
}

/// Generate the cycle counter intrinsic LLVM IR
/// On AArch64: reads CNTVCT_EL0 (virtual timer counter)
pub fn generate_cycle_counter_intrinsic_llir() -> String {
    r#"
; Cycle counter (AArch64 CNTVCT_EL0)
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
pub(crate) fn extract_literal_u64(expr: &Expr) -> Option<u64> {
    if let Expr::Lit(lit_expr) = expr {
        if let syn::Lit::Int(lit_int) = &lit_expr.lit {
            return lit_int.base10_parse::<u64>().ok();
        }
    }
    None
}
