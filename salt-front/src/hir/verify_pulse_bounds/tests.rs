//! @pulse Verifier Tests — TDD for {verify_pulse_bounds}
//!
//! Each layer tests a specific invariant of the pulse cost analysis.
//! Tests construct BasicBlock CFGs directly without going through
//! the full compiler pipeline.

use crate::hir::async_lower::{BasicBlock, Terminator};
use crate::hir::stmt::{Stmt, StmtKind};
use crate::hir::expr::{Expr, ExprKind, BinOp, Literal, Block};
use crate::hir::types::Type;
use super::{verify_pulse_bounds, PulseResult};

// =============================================================================
// Helpers — construct HIR nodes concisely
// =============================================================================

fn span() -> proc_macro2::Span {
    proc_macro2::Span::call_site()
}

/// Make a simple integer literal expression.
fn int_lit(v: i64) -> Expr {
    Expr { kind: ExprKind::Literal(Literal::Int(v)), ty: Type::I64, span: span() }
}

/// Make a binary add: lhs + rhs (cost = 1).
fn add_expr(lhs: Expr, rhs: Expr) -> Expr {
    Expr {
        kind: ExprKind::Binary {
            op: BinOp::Add,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
        ty: Type::I64,
        span: span(),
    }
}

/// Make a binary division: lhs / rhs (cost = 20).
fn div_expr(lhs: Expr, rhs: Expr) -> Expr {
    Expr {
        kind: ExprKind::Binary {
            op: BinOp::Div,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
        ty: Type::I64,
        span: span(),
    }
}

/// Make a binary multiply: lhs * rhs (cost = 1).
fn mul_expr(lhs: Expr, rhs: Expr) -> Expr {
    Expr {
        kind: ExprKind::Binary {
            op: BinOp::Mul,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
        ty: Type::I64,
        span: span(),
    }
}

/// Make a comparison: lhs < rhs (cost = 1).
fn lt_expr(lhs: Expr, rhs: Expr) -> Expr {
    Expr {
        kind: ExprKind::Binary {
            op: BinOp::Lt,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
        ty: Type::Bool,
        span: span(),
    }
}

/// Make a semi expression statement.
fn semi(expr: Expr) -> Stmt {
    Stmt { kind: StmtKind::Semi(expr), span: span() }
}

/// Make N addition statements (cost = N cycles).
fn make_add_stmts(n: usize) -> Vec<Stmt> {
    (0..n).map(|_| semi(add_expr(int_lit(1), int_lit(2)))).collect()
}

/// Make N division statements (cost = 20*N cycles).
fn make_div_stmts(n: usize) -> Vec<Stmt> {
    (0..n).map(|_| semi(div_expr(int_lit(100), int_lit(3)))).collect()
}

/// Make N multiply statements (cost = N cycles).
fn make_mul_stmts(n: usize) -> Vec<Stmt> {
    (0..n).map(|_| semi(mul_expr(int_lit(5), int_lit(7)))).collect()
}

// =============================================================================
// Layer 0: Trivial async — 2 adds + yield (well under budget)
// =============================================================================

#[test]
fn layer0_trivial_under_budget() {
    // Block 0: 2 adds → Yield → Block 1
    // Block 1: Return
    let blocks = vec![
        BasicBlock {
            id: 0,
            stmts: make_add_stmts(2),
            terminator: Terminator::Yield { resume_state: 1 },
        },
        BasicBlock {
            id: 1,
            stmts: vec![],
            terminator: Terminator::Return,
        },
    ];

    let result = verify_pulse_bounds(&blocks, 100);
    match result {
        PulseResult::Verified { max_path_cost } => {
            assert!(max_path_cost <= 100, "expected cost ≤ 100, got {}", max_path_cost);
            // 2 adds = 2 cycles (cost of block 0)
            assert_eq!(max_path_cost, 2, "2 additions should cost 2 cycles");
        }
        other => panic!("Expected Verified, got {:?}", other),
    }
}

// =============================================================================
// Layer 1: Heavy computation — exceeds budget
// =============================================================================

#[test]
fn layer1_heavy_computation_exceeds_budget() {
    // Block 0: 1000 multiplies (1000 cycles) → Return
    // Budget = 500 → should FAIL
    let blocks = vec![
        BasicBlock {
            id: 0,
            stmts: make_mul_stmts(1000),
            terminator: Terminator::Return,
        },
    ];

    let result = verify_pulse_bounds(&blocks, 500);
    match result {
        PulseResult::Violation { path_cost, budget, .. } => {
            assert_eq!(budget, 500);
            assert!(path_cost > 500, "path_cost should exceed budget");
        }
        other => panic!("Expected Violation, got {:?}", other),
    }
}

// =============================================================================
// Layer 2: Division-heavy path with yield splitting — passes
// =============================================================================

#[test]
fn layer2_division_with_yield_passes() {
    // Block 0: 10 divisions (200 cycles) → Yield → Block 1
    // Block 1: 10 divisions (200 cycles) → Return
    // Budget = 300 → each segment is 200 ≤ 300, so PASS
    let blocks = vec![
        BasicBlock {
            id: 0,
            stmts: make_div_stmts(10),
            terminator: Terminator::Yield { resume_state: 1 },
        },
        BasicBlock {
            id: 1,
            stmts: make_div_stmts(10),
            terminator: Terminator::Return,
        },
    ];

    let result = verify_pulse_bounds(&blocks, 300);
    match result {
        PulseResult::Verified { max_path_cost } => {
            // Max is 200 (either segment alone)
            assert_eq!(max_path_cost, 200, "each segment costs 200 cycles");
        }
        other => panic!("Expected Verified, got {:?}", other),
    }
}

// =============================================================================
// Layer 3: Unbounded loop without yield — detected
// =============================================================================

#[test]
fn layer3_unbounded_loop_detected() {
    // Block 0: contains `loop { x = x + 1; }` (no yield) → Return
    let loop_body = Block {
        stmts: make_add_stmts(1),
        value: None,
        ty: Type::Unit,
    };
    let blocks = vec![
        BasicBlock {
            id: 0,
            stmts: vec![Stmt {
                kind: StmtKind::Loop(loop_body),
                span: span(),
            }],
            terminator: Terminator::Return,
        },
    ];

    let result = verify_pulse_bounds(&blocks, 50_000);
    match result {
        PulseResult::UnboundedLoop { block_id } => {
            assert_eq!(block_id, 0);
        }
        other => panic!("Expected UnboundedLoop, got {:?}", other),
    }
}

// =============================================================================
// Layer 4: Bounded loop (while with 10 arithmetic ops) — passes
// =============================================================================

#[test]
fn layer4_bounded_while_passes() {
    // Block 0: while (i < 10) { a = a + 1; } → Yield
    // The while stmt costs: cond_cost + 10 * (cond_cost + body_cost)
    // cond = i < 10 → cost 1 (comparison)
    // body = 1 add → cost 1
    // Total while: 1 + 10*(1+1) = 21
    let while_cond = lt_expr(int_lit(0), int_lit(10));
    let while_body = Block {
        stmts: make_add_stmts(1),
        value: None,
        ty: Type::Unit,
    };
    let blocks = vec![
        BasicBlock {
            id: 0,
            stmts: vec![Stmt {
                kind: StmtKind::While { cond: while_cond, body: while_body },
                span: span(),
            }],
            terminator: Terminator::Yield { resume_state: 1 },
        },
        BasicBlock {
            id: 1,
            stmts: vec![],
            terminator: Terminator::Return,
        },
    ];

    let result = verify_pulse_bounds(&blocks, 100);
    match result {
        PulseResult::Verified { max_path_cost } => {
            // while: cond(1) + 10*(cond(1)+body(1)) = 1 + 20 = 21
            assert_eq!(max_path_cost, 21, "bounded while should cost 21 cycles");
        }
        other => panic!("Expected Verified, got {:?}", other),
    }
}

// =============================================================================
// Layer 5: Conditional paths — worst case arm checked
// =============================================================================

#[test]
fn layer5_conditional_worst_case() {
    // Block 0: 5 adds (5 cycles) → Branch(cond, block1, block2)
    // Block 1 (hot): 10 divisions (200 cycles) → Yield → Block 3
    // Block 2 (cold): 2 adds (2 cycles) → Yield → Block 3
    // Block 3: Return
    // Budget = 250 → worst case = block0(5) + branch(1) + block1(200) = 206 ≤ 250 → PASS
    let blocks = vec![
        BasicBlock {
            id: 0,
            stmts: make_add_stmts(5),
            terminator: Terminator::Branch {
                cond: lt_expr(int_lit(0), int_lit(10)),
                target_true: 1,
                target_false: 2,
            },
        },
        BasicBlock {
            id: 1,
            stmts: make_div_stmts(10), // 200 cycles
            terminator: Terminator::Yield { resume_state: 3 },
        },
        BasicBlock {
            id: 2,
            stmts: make_add_stmts(2), // 2 cycles
            terminator: Terminator::Yield { resume_state: 3 },
        },
        BasicBlock {
            id: 3,
            stmts: vec![],
            terminator: Terminator::Return,
        },
    ];

    let result = verify_pulse_bounds(&blocks, 250);
    match result {
        PulseResult::Verified { max_path_cost } => {
            // Worst path: block0(5) + branch(1) + block1(200) = 206
            assert_eq!(max_path_cost, 206, "worst case should be 206 cycles");
        }
        other => panic!("Expected Verified, got {:?}", other),
    }
}

// =============================================================================
// Layer 5b: Conditional paths — worst case EXCEEDS budget
// =============================================================================

#[test]
fn layer5b_conditional_worst_case_exceeds() {
    // Same as layer5 but budget = 100 → worst case 206 > 100 → FAIL
    let blocks = vec![
        BasicBlock {
            id: 0,
            stmts: make_add_stmts(5),
            terminator: Terminator::Branch {
                cond: lt_expr(int_lit(0), int_lit(10)),
                target_true: 1,
                target_false: 2,
            },
        },
        BasicBlock {
            id: 1,
            stmts: make_div_stmts(10),
            terminator: Terminator::Yield { resume_state: 3 },
        },
        BasicBlock {
            id: 2,
            stmts: make_add_stmts(2),
            terminator: Terminator::Yield { resume_state: 3 },
        },
        BasicBlock {
            id: 3,
            stmts: vec![],
            terminator: Terminator::Return,
        },
    ];

    let result = verify_pulse_bounds(&blocks, 100);
    match result {
        PulseResult::Violation { path_cost, budget, .. } => {
            assert_eq!(budget, 100);
            assert!(path_cost > 100, "worst path should exceed budget of 100");
        }
        other => panic!("Expected Violation, got {:?}", other),
    }
}

// =============================================================================
// Layer 6: Empty CFG — trivially verified
// =============================================================================

#[test]
fn layer6_empty_cfg() {
    let result = verify_pulse_bounds(&[], 100);
    match result {
        PulseResult::Verified { max_path_cost } => {
            assert_eq!(max_path_cost, 0);
        }
        other => panic!("Expected Verified, got {:?}", other),
    }
}
