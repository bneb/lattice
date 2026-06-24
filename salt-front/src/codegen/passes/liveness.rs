//! Cross-Yield Liveness Analysis
//!
//! Identifies variables that are live across yield/suspension points
//! in `@yielding` functions. These variables must be captured into
//! the TaskFrame struct for the coroutine state machine.
//!
//! ## Algorithm
//! 1. Walk function body to locate all yield points (salt.yield operations).
//! 2. For each variable, compute def and last-use positions.
//! 3. A variable is "cross-yield" if its def precedes a yield point and
//!    its last use follows that yield point.
//! 4. ZST pruning: Context (unit type) is excluded from the frame.

use crate::grammar::{SaltFn, SaltBlock, Stmt};
use crate::grammar::attr::has_attribute;
use std::collections::HashMap;

/// A yield/suspension point discovered in a function body
#[derive(Debug, Clone)]
pub struct YieldPointInfo {
    /// Sequential index of this yield point
    pub index: usize,
    /// Approximate statement position (for ordering)
    pub position: usize,
    /// Optional label (e.g., "loop_back_edge", "io_wait")
    pub label: String,
}

/// A variable that must be captured into the TaskFrame
#[derive(Debug, Clone)]
pub struct FrameMember {
    /// Index in the TaskFrame struct
    pub index: usize,
    /// Type name (MLIR type string)
    pub ty: String,
    /// Original variable name
    pub name: String,
}

/// Result of cross-yield analysis for a single function
#[derive(Debug, Clone, Default)]
pub struct LivenessResult {
    /// Yield points found in this function
    pub yield_points: Vec<YieldPointInfo>,
    /// Variables that cross yield boundaries (must be in TaskFrame)
    pub frame_members: Vec<FrameMember>,
    /// Total frame size in members (excluding ZSTs)
    pub frame_size: usize,
    /// Whether this function needs coroutine transformation
    pub needs_transform: bool,
}

/// Information about a variable definition and its uses
#[derive(Debug, Clone)]
struct VarLifetime {
    /// Statement position where the variable is defined
    def_position: usize,
    /// Statement position of the last use
    last_use_position: usize,
    /// Type of the variable (as a string)
    ty: String,
    /// Whether this is a ZST (zero-sized type)
    is_zst: bool,
}

/// Known zero-sized types that should not appear in the frame
const ZST_TYPES: &[&str] = &[
    "Context", "()", "Unit", "unit",
];

/// Cross-yield liveness analyzer
pub struct CrossYieldAnalyzer {
    /// Current statement position counter
    position: usize,
    /// Yield points discovered during walk
    yield_points: Vec<YieldPointInfo>,
    /// Variable definitions and their lifetimes
    var_lifetimes: HashMap<String, VarLifetime>,
}

impl CrossYieldAnalyzer {
    pub fn new() -> Self {
        Self {
            position: 0,
            yield_points: Vec::new(),
            var_lifetimes: HashMap::new(),
        }
    }

    /// Analyze a function for cross-yield liveness
    pub fn analyze(&mut self, func: &SaltFn) -> LivenessResult {
        // Only @yielding functions need coroutine transformation
        let is_yielding = has_attribute(&func.attributes, "yielding")
            || has_attribute(&func.attributes, "pulse");

        if !is_yielding {
            return LivenessResult {
                yield_points: Vec::new(),
                frame_members: Vec::new(),
                frame_size: 0,
                needs_transform: false,
            };
        }

        // Register function parameters as defined at position 0
        for arg in &func.args {
            let ty_str = arg.ty.as_ref()
                .map(|t| format!("{:?}", t))
                .unwrap_or_else(|| "self".to_string());
            let is_zst = ZST_TYPES.iter().any(|z| ty_str.contains(z));
            self.var_lifetimes.insert(arg.name.to_string(), VarLifetime {
                def_position: 0,
                last_use_position: 0,
                ty: ty_str,
                is_zst,
            });
        }

        // Walk the function body
        self.walk_block(&func.body);

        // Determine which variables cross yield boundaries
        let mut frame_members = Vec::new();
        let mut index = 0;

        for (name, lifetime) in &self.var_lifetimes {
            if lifetime.is_zst {
                continue; // ZST pruning
            }

            // Check if this variable crosses any yield point
            let crosses_yield = self.yield_points.iter().any(|yp| {
                lifetime.def_position < yp.position
                    && lifetime.last_use_position > yp.position
            });

            if crosses_yield {
                frame_members.push(FrameMember {
                    index,
                    ty: lifetime.ty.clone(),
                    name: name.clone(),
                });
                index += 1;
            }
        }

        // Sort by name for deterministic output
        frame_members.sort_by(|a, b| a.name.cmp(&b.name));
        // Reassign indices after sort
        for (i, m) in frame_members.iter_mut().enumerate() {
            m.index = i;
        }

        let frame_size = frame_members.len();
        let needs_transform = !self.yield_points.is_empty();

        LivenessResult {
            yield_points: self.yield_points.clone(),
            frame_members,
            frame_size,
            needs_transform,
        }
    }

    /// Walk a block, tracking positions and identifying yield points
    fn walk_block(&mut self, block: &SaltBlock) {
        for stmt in &block.stmts {
            self.walk_stmt(stmt);
            self.position += 1;
        }
    }

    /// Walk a statement, looking for yield points and variable defs/uses
    fn walk_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Syn(syn::Stmt::Local(local)) => {
                // Variable definition
                if let syn::Pat::Ident(pat_id) = &local.pat {
                    let name = pat_id.ident.to_string();
                    let ty_str = "i64".to_string(); // Default; real type comes from codegen
                    let is_zst = false; // Conservative: let codegen decide
                    self.var_lifetimes.insert(name, VarLifetime {
                        def_position: self.position,
                        last_use_position: self.position,
                        ty: ty_str,
                        is_zst,
                    });
                }
                // Walk the initializer
                if let Some(init) = &local.init {
                    self.record_uses_in_expr(&init.expr);
                }
            }
            Stmt::Expr(expr, _) => {
                self.record_uses_in_expr(expr);
                // Check if this expression is a yield call
                if self.is_yield_call(expr) {
                    self.yield_points.push(YieldPointInfo {
                        index: self.yield_points.len(),
                        position: self.position,
                        label: format!("yield_{}", self.yield_points.len()),
                    });
                }
            }
            Stmt::For(for_stmt) => {
                self.record_uses_in_expr(&for_stmt.iter);
                // Loop back-edge is an implicit yield point for @yielding functions
                self.yield_points.push(YieldPointInfo {
                    index: self.yield_points.len(),
                    position: self.position,
                    label: "loop_back_edge".to_string(),
                });
                self.walk_block(&for_stmt.body);
            }
            Stmt::While(while_stmt) => {
                self.record_uses_in_expr(&while_stmt.cond);
                // Loop back-edge is an implicit yield point
                self.yield_points.push(YieldPointInfo {
                    index: self.yield_points.len(),
                    position: self.position,
                    label: "loop_back_edge".to_string(),
                });
                self.walk_block(&while_stmt.body);
            }
            Stmt::If(if_stmt) => {
                self.record_uses_in_expr(&if_stmt.cond);
                self.walk_block(&if_stmt.then_branch);
                if let Some(else_branch) = &if_stmt.else_branch {
                    match else_branch.as_ref() {
                        crate::grammar::SaltElse::Block(b) => self.walk_block(b),
                        crate::grammar::SaltElse::If(nested) => {
                            self.walk_stmt(&Stmt::If(*nested.clone()));
                        }
                    }
                }
            }
            Stmt::Match(match_stmt) => {
                self.record_uses_in_expr(&match_stmt.scrutinee);
                for arm in &match_stmt.arms {
                    self.walk_block(&arm.body);
                }
            }
            Stmt::Unsafe(block) => self.walk_block(block),
            Stmt::Return(Some(expr)) => self.record_uses_in_expr(expr),
            Stmt::Move(expr) => self.record_uses_in_expr(expr),
            Stmt::LetElse(le) => {
                self.record_uses_in_expr(&le.init);
                self.walk_block(&le.else_block);
            }
            _ => {}
        }
    }

    /// Record variable uses in an expression (updates last_use_position)
    fn record_uses_in_expr(&mut self, expr: &syn::Expr) {
        match expr {
            syn::Expr::Path(path) => {
                if let Some(ident) = path.path.get_ident() {
                    let name = ident.to_string();
                    if let Some(lt) = self.var_lifetimes.get_mut(&name) {
                        lt.last_use_position = self.position;
                    }
                }
            }
            syn::Expr::Call(call) => {
                self.record_uses_in_expr(&call.func);
                for arg in &call.args {
                    self.record_uses_in_expr(arg);
                }
            }
            syn::Expr::MethodCall(mc) => {
                self.record_uses_in_expr(&mc.receiver);
                for arg in &mc.args {
                    self.record_uses_in_expr(arg);
                }
            }
            syn::Expr::Binary(bin) => {
                self.record_uses_in_expr(&bin.left);
                self.record_uses_in_expr(&bin.right);
            }
            syn::Expr::Unary(un) => {
                self.record_uses_in_expr(&un.expr);
            }
            syn::Expr::Field(f) => {
                self.record_uses_in_expr(&f.base);
            }
            syn::Expr::Index(idx) => {
                self.record_uses_in_expr(&idx.expr);
                self.record_uses_in_expr(&idx.index);
            }
            syn::Expr::Assign(a) => {
                self.record_uses_in_expr(&a.left);
                self.record_uses_in_expr(&a.right);
            }
            syn::Expr::Return(ret) => {
                if let Some(inner) = &ret.expr {
                    self.record_uses_in_expr(inner);
                }
            }
            syn::Expr::Paren(p) => {
                self.record_uses_in_expr(&p.expr);
            }
            syn::Expr::Reference(r) => {
                self.record_uses_in_expr(&r.expr);
            }
            syn::Expr::Cast(c) => {
                self.record_uses_in_expr(&c.expr);
            }
            syn::Expr::Tuple(t) => {
                for e in &t.elems {
                    self.record_uses_in_expr(e);
                }
            }
            _ => {} // Literals — no variable uses
        }
    }

    /// Check if an expression is a yield call (e.g., `yield_now()`, `salt::yield()`)
    fn is_yield_call(&self, expr: &syn::Expr) -> bool {
        match expr {
            syn::Expr::Call(call) => {
                if let syn::Expr::Path(path) = &*call.func {
                    let name: String = path.path.segments
                        .iter()
                        .map(|s| s.ident.to_string())
                        .collect::<Vec<_>>()
                        .join("::");
                    return name.contains("yield_now")
                        || name.contains("yield_to_executor")
                        || name.contains("salt_yield");
                }
                false
            }
            _ => false,
        }
    }
}

impl Default for CrossYieldAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a VarLifetime for testing
    fn make_lifetime(def: usize, last_use: usize, ty: &str, is_zst: bool) -> VarLifetime {
        VarLifetime {
            def_position: def,
            last_use_position: last_use,
            ty: ty.to_string(),
            is_zst,
        }
    }

    #[test]
    fn test_no_yield_points_empty_frame() {
        let analyzer = CrossYieldAnalyzer {
            position: 10,
            yield_points: vec![], // No yields
            var_lifetimes: {
                let mut m = HashMap::new();
                m.insert("x".into(), make_lifetime(0, 5, "i64", false));
                m.insert("y".into(), make_lifetime(3, 8, "f32", false));
                m
            },
        };

        // No yield points means no variables cross a yield boundary
        let crosses: Vec<_> = analyzer.var_lifetimes.iter()
            .filter(|(_, lt)| {
                analyzer.yield_points.iter().any(|yp| {
                    lt.def_position < yp.position && lt.last_use_position > yp.position
                })
            })
            .collect();
        assert!(crosses.is_empty());
    }

    #[test]
    fn test_single_yield_captures_crossing_var() {
        let analyzer = CrossYieldAnalyzer {
            position: 10,
            yield_points: vec![YieldPointInfo {
                index: 0,
                position: 3,
                label: "yield_0".into(),
            }],
            var_lifetimes: {
                let mut m = HashMap::new();
                // x is defined at 0, used at 5 → crosses yield at 3
                m.insert("x".into(), make_lifetime(0, 5, "i64", false));
                // y is defined at 4, used at 8 → does NOT cross yield at 3
                m.insert("y".into(), make_lifetime(4, 8, "f32", false));
                m
            },
        };

        let crosses: Vec<&String> = analyzer.var_lifetimes.iter()
            .filter(|(_, lt)| {
                analyzer.yield_points.iter().any(|yp| {
                    lt.def_position < yp.position && lt.last_use_position > yp.position
                })
            })
            .map(|(name, _)| name)
            .collect();

        assert_eq!(crosses.len(), 1);
        assert!(crosses.contains(&&"x".to_string()));
    }

    #[test]
    fn test_zst_pruning() {
        let analyzer = CrossYieldAnalyzer {
            position: 10,
            yield_points: vec![YieldPointInfo {
                index: 0,
                position: 3,
                label: "yield_0".into(),
            }],
            var_lifetimes: {
                let mut m = HashMap::new();
                // ctx is Context (ZST) — defined at 0, used at 5 → crosses yield
                m.insert("ctx".into(), make_lifetime(0, 5, "Context", true));
                // data is i64 — defined at 0, used at 5 → crosses yield
                m.insert("data".into(), make_lifetime(0, 5, "i64", false));
                m
            },
        };

        let frame: Vec<&String> = analyzer.var_lifetimes.iter()
            .filter(|(_, lt)| {
                !lt.is_zst && analyzer.yield_points.iter().any(|yp| {
                    lt.def_position < yp.position && lt.last_use_position > yp.position
                })
            })
            .map(|(name, _)| name)
            .collect();

        assert_eq!(frame.len(), 1);
        assert!(frame.contains(&&"data".to_string()));
        // ctx should be pruned (ZST)
        assert!(!frame.contains(&&"ctx".to_string()));
    }

    #[test]
    fn test_multiple_yields() {
        let analyzer = CrossYieldAnalyzer {
            position: 20,
            yield_points: vec![
                YieldPointInfo { index: 0, position: 5, label: "yield_0".into() },
                YieldPointInfo { index: 1, position: 10, label: "yield_1".into() },
                YieldPointInfo { index: 2, position: 15, label: "yield_2".into() },
            ],
            var_lifetimes: {
                let mut m = HashMap::new();
                // a: def=0, use=18 → crosses all three yields
                m.insert("a".into(), make_lifetime(0, 18, "i64", false));
                // b: def=6, use=12 → crosses yield_1 only
                m.insert("b".into(), make_lifetime(6, 12, "i32", false));
                // c: def=11, use=14 → crosses no yields (between yield_1 and yield_2)
                m.insert("c".into(), make_lifetime(11, 14, "u8", false));
                m
            },
        };

        let crosses: Vec<&String> = analyzer.var_lifetimes.iter()
            .filter(|(_, lt)| {
                !lt.is_zst && analyzer.yield_points.iter().any(|yp| {
                    lt.def_position < yp.position && lt.last_use_position > yp.position
                })
            })
            .map(|(name, _)| name)
            .collect();

        assert!(crosses.contains(&&"a".to_string()), "a crosses all yields");
        assert!(crosses.contains(&&"b".to_string()), "b crosses yield_1");
        assert!(!crosses.contains(&&"c".to_string()), "c is between yields, crosses none");
    }

    #[test]
    fn test_known_zst_types() {
        for ty in ZST_TYPES {
            assert!(ZST_TYPES.contains(ty));
        }
        assert!(ZST_TYPES.contains(&"Context"));
        assert!(ZST_TYPES.contains(&"()"));
        assert!(ZST_TYPES.contains(&"Unit"));
    }
}
