//! Call Graph Analysis Pass (Fixed-Point Propagation)
//!
//! Replaces the heuristic string-matching I/O detection in pulse_injection.rs
//! and sync_verifier.rs with proper transitive analysis.
//!
//! ## Algorithm
//! 1. Walk all function ASTs to extract direct call edges.
//! 2. Seed attributes: @pulse → is_pulse, known I/O operations → is_blocking.
//! 3. Fixed-point iteration: if B is blocking and A calls B, A becomes blocking.
//! 4. Safety verification: if @pulse function transitively reaches @blocking
//!    without a `spawn` boundary, emit a compiler error.

use crate::grammar::{SaltFile, SaltFn, SaltBlock, Stmt, Item, SaltImpl};
use crate::grammar::attr::{extract_pulse_hz, has_attribute};
use std::collections::{HashMap, HashSet};

/// Known I/O / blocking operations (canonical names)
const BLOCKING_OPERATIONS: &[&str] = &[
    // Network
    "TcpListener::bind", "TcpListener::accept",
    "TcpStream::read", "TcpStream::write",
    "UdpSocket::recv", "UdpSocket::send",
    // File system
    "fs::read", "fs::write", "fs::open",
    "File::read", "File::write",
    // Thread / sync
    "thread::sleep", "Mutex::lock",
    // Explicit yield (these are context-requiring, not blocking per se)
    "executor::yield_now",
];

/// Attributes discovered for a function
#[derive(Debug, Clone, Default)]
pub struct FnAttributes {
    /// Explicitly marked @pulse
    pub is_pulse: bool,
    /// Performs or transitively reaches a blocking I/O operation
    pub is_blocking: bool,
    /// Explicitly marked @yielding
    pub is_yielding: bool,
    /// Requires Context (pulse, yielding, or transitively calls one that does)
    pub requires_context: bool,
    /// Pulse frequency (if @pulse)
    pub pulse_hz: Option<u32>,
}

/// Result of call graph analysis
#[derive(Debug)]
pub struct CallGraphAnalysis {
    /// Per-function attributes (after propagation)
    pub fn_attributes: HashMap<String, FnAttributes>,
    /// Direct call edges: caller → [callees]
    pub call_edges: HashMap<String, Vec<String>>,
    /// Safety violations found
    pub violations: Vec<SafetyViolation>,
}

/// A safety violation: @pulse function transitively calls @blocking
#[derive(Debug, Clone)]
pub struct SafetyViolation {
    /// The @pulse function at the root
    pub pulse_fn: String,
    /// The blocking function that was reached
    pub blocking_fn: String,
    /// The call chain from pulse_fn to blocking_fn
    pub call_chain: Vec<String>,
}

/// Analyzes the call graph for a Salt file
pub struct CallGraphAnalyzer {
    /// Per-function attributes (mutable during analysis)
    pub(crate) fn_attributes: HashMap<String, FnAttributes>,
    /// Direct call edges
    pub(crate) call_edges: HashMap<String, Vec<String>>,
}

impl CallGraphAnalyzer {
    pub fn new() -> Self {
        Self {
            fn_attributes: HashMap::new(),
            call_edges: HashMap::new(),
        }
    }

    /// Full analysis pipeline: build → propagate → verify
    pub fn analyze(&mut self, file: &SaltFile) -> CallGraphAnalysis {
        // Phase 1: Seed explicit attributes and build call graph
        self.seed_from_file(file);

        // Phase 2: Fixed-point propagation
        self.propagate();

        // Phase 3: Safety verification
        let violations = self.verify_pulse_safety();

        CallGraphAnalysis {
            fn_attributes: self.fn_attributes.clone(),
            call_edges: self.call_edges.clone(),
            violations,
        }
    }

    /// Phase 1: Walk the file and seed attributes + call edges
    fn seed_from_file(&mut self, file: &SaltFile) {
        for item in &file.items {
            match item {
                Item::Fn(func) => {
                    self.seed_function(func, None);
                }
                Item::Impl(SaltImpl::Methods { target_ty, methods, .. }) => {
                    let target_name = format!("{:?}", target_ty);
                    for m in methods {
                        self.seed_function(m, Some(&target_name));
                    }
                }
                Item::Impl(SaltImpl::Trait { methods, target_ty, .. }) => {
                    let target_name = format!("{:?}", target_ty);
                    for m in methods {
                        self.seed_function(m, Some(&target_name));
                    }
                }
                _ => {}
            }
        }
    }

    /// Seed a single function's attributes and extract its call edges
    fn seed_function(&mut self, func: &SaltFn, impl_target: Option<&str>) {
        let name = if let Some(target) = impl_target {
            format!("{}::{}", target, func.name)
        } else {
            func.name.to_string()
        };

        let mut attrs = FnAttributes::default();

        // Check for @pulse attribute
        if let Some(hz) = extract_pulse_hz(&func.attributes) {
            attrs.is_pulse = true;
            attrs.pulse_hz = Some(hz);
            attrs.requires_context = true;
        }

        // Check for @yielding attribute
        if has_attribute(&func.attributes, "yielding") {
            attrs.is_yielding = true;
            attrs.requires_context = true;
        }

        // Check for @blocking attribute (explicit user annotation)
        if has_attribute(&func.attributes, "blocking") {
            attrs.is_blocking = true;
        }

        self.fn_attributes.insert(name.clone(), attrs);

        // Extract call edges from the function body
        let mut callees = Vec::new();
        self.extract_calls_from_block(&func.body, &mut callees);
        self.call_edges.insert(name, callees);
    }

    /// Walk a block to extract function call names
    fn extract_calls_from_block(&self, block: &SaltBlock, callees: &mut Vec<String>) {
        for stmt in &block.stmts {
            self.extract_calls_from_stmt(stmt, callees);
        }
    }

    /// Walk a statement to extract function call names
    fn extract_calls_from_stmt(&self, stmt: &Stmt, callees: &mut Vec<String>) {
        match stmt {
            Stmt::Expr(expr, _) => self.extract_calls_from_expr(expr, callees),
            Stmt::For(for_stmt) => {
                self.extract_calls_from_expr(&for_stmt.iter, callees);
                self.extract_calls_from_block(&for_stmt.body, callees);
            }
            Stmt::While(while_stmt) => {
                self.extract_calls_from_expr(&while_stmt.cond, callees);
                self.extract_calls_from_block(&while_stmt.body, callees);
            }
            Stmt::If(if_stmt) => {
                self.extract_calls_from_expr(&if_stmt.cond, callees);
                self.extract_calls_from_block(&if_stmt.then_branch, callees);
                if let Some(else_branch) = &if_stmt.else_branch {
                    match else_branch.as_ref() {
                        crate::grammar::SaltElse::Block(b) => {
                            self.extract_calls_from_block(b, callees);
                        }
                        crate::grammar::SaltElse::If(nested) => {
                            self.extract_calls_from_stmt(
                                &Stmt::If(*nested.clone()), callees
                            );
                        }
                    }
                }
            }
            Stmt::Match(match_stmt) => {
                self.extract_calls_from_expr(&match_stmt.scrutinee, callees);
                for arm in &match_stmt.arms {
                    self.extract_calls_from_block(&arm.body, callees);
                }
            }
            Stmt::Unsafe(block) => {
                self.extract_calls_from_block(block, callees);
            }
            Stmt::Return(Some(expr)) => {
                self.extract_calls_from_expr(expr, callees);
            }
            Stmt::Syn(syn::Stmt::Expr(expr, _)) => {
                self.extract_calls_from_expr(expr, callees);
            }
            Stmt::Syn(syn::Stmt::Local(local)) => {
                if let Some(init) = &local.init {
                    self.extract_calls_from_expr(&init.expr, callees);
                }
            }
            Stmt::Move(expr) => {
                self.extract_calls_from_expr(expr, callees);
            }
            Stmt::LetElse(le) => {
                self.extract_calls_from_expr(&le.init, callees);
                self.extract_calls_from_block(&le.else_block, callees);
            }
            _ => {}
        }
    }

    /// Extract function call names from a syn expression
    fn extract_calls_from_expr(&self, expr: &syn::Expr, callees: &mut Vec<String>) {
        match expr {
            syn::Expr::Call(call) => {
                // Extract the callee name
                if let Some(name) = self.expr_to_call_name(&call.func) {
                    callees.push(name);
                }
                // Also recurse into arguments
                for arg in &call.args {
                    self.extract_calls_from_expr(arg, callees);
                }
            }
            syn::Expr::MethodCall(mc) => {
                // Method::name pattern
                callees.push(mc.method.to_string());
                // Recurse into receiver and arguments
                self.extract_calls_from_expr(&mc.receiver, callees);
                for arg in &mc.args {
                    self.extract_calls_from_expr(arg, callees);
                }
            }
            syn::Expr::Binary(bin) => {
                self.extract_calls_from_expr(&bin.left, callees);
                self.extract_calls_from_expr(&bin.right, callees);
            }
            syn::Expr::Unary(un) => {
                self.extract_calls_from_expr(&un.expr, callees);
            }
            syn::Expr::Block(block) => {
                for stmt in &block.block.stmts {
                    if let syn::Stmt::Expr(e, _) = stmt {
                        self.extract_calls_from_expr(e, callees);
                    }
                }
            }
            syn::Expr::If(if_expr) => {
                self.extract_calls_from_expr(&if_expr.cond, callees);
                for stmt in &if_expr.then_branch.stmts {
                    if let syn::Stmt::Expr(e, _) = stmt {
                        self.extract_calls_from_expr(e, callees);
                    }
                }
                if let Some((_, else_expr)) = &if_expr.else_branch {
                    self.extract_calls_from_expr(else_expr, callees);
                }
            }
            syn::Expr::Field(field) => {
                self.extract_calls_from_expr(&field.base, callees);
            }
            syn::Expr::Index(idx) => {
                self.extract_calls_from_expr(&idx.expr, callees);
                self.extract_calls_from_expr(&idx.index, callees);
            }
            syn::Expr::Paren(p) => {
                self.extract_calls_from_expr(&p.expr, callees);
            }
            syn::Expr::Reference(r) => {
                self.extract_calls_from_expr(&r.expr, callees);
            }
            syn::Expr::Assign(a) => {
                self.extract_calls_from_expr(&a.left, callees);
                self.extract_calls_from_expr(&a.right, callees);
            }
            syn::Expr::Cast(c) => {
                self.extract_calls_from_expr(&c.expr, callees);
            }
            syn::Expr::Return(r) => {
                if let Some(e) = &r.expr {
                    self.extract_calls_from_expr(e, callees);
                }
            }
            syn::Expr::Tuple(t) => {
                for e in &t.elems {
                    self.extract_calls_from_expr(e, callees);
                }
            }
            _ => {} // Literals, paths, etc. — no calls
        }
    }

    /// Convert a call expression's function position to a name string
    fn expr_to_call_name(&self, expr: &syn::Expr) -> Option<String> {
        match expr {
            syn::Expr::Path(path) => {
                let segments: Vec<String> = path.path.segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect();
                Some(segments.join("::"))
            }
            _ => None,
        }
    }

    /// Phase 2: Fixed-point propagation
    /// If function B is blocking and A calls B, then A becomes blocking.
    /// If function B requires_context and A calls B, then A requires_context.
    fn propagate(&mut self) {
        // First, mark functions that directly call known blocking operations
        let call_edges_snapshot = self.call_edges.clone();
        for (fn_name, callees) in &call_edges_snapshot {
            for callee in callees {
                if self.is_known_blocking(callee) {
                    if let Some(attrs) = self.fn_attributes.get_mut(fn_name) {
                        attrs.is_blocking = true;
                    }
                }
            }
        }

        // Fixed-point iteration
        let mut changed = true;
        while changed {
            changed = false;
            let snapshot = self.fn_attributes.clone();

            for (fn_name, callees) in &self.call_edges {
                for callee in callees {
                    // Look up callee attributes (may be in our map or unknown)
                    if let Some(callee_attrs) = snapshot.get(callee) {
                        if let Some(caller_attrs) = self.fn_attributes.get_mut(fn_name) {
                            // Propagate blocking
                            if callee_attrs.is_blocking && !caller_attrs.is_blocking {
                                caller_attrs.is_blocking = true;
                                changed = true;
                            }
                            // Propagate context requirement
                            if callee_attrs.requires_context && !caller_attrs.requires_context {
                                caller_attrs.requires_context = true;
                                changed = true;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Check if a callee name matches a known blocking operation
    pub(crate) fn is_known_blocking(&self, callee: &str) -> bool {
        BLOCKING_OPERATIONS.iter().any(|op| {
            callee == *op || callee.ends_with(op)
        })
    }

    /// Phase 3: Verify that @pulse functions don't transitively call blocking
    fn verify_pulse_safety(&self) -> Vec<SafetyViolation> {
        let mut violations = Vec::new();

        for (fn_name, attrs) in &self.fn_attributes {
            if attrs.is_pulse && attrs.is_blocking {
                // Find the call chain to the blocking function
                if let Some(chain) = self.find_blocking_chain(fn_name) {
                    violations.push(SafetyViolation {
                        pulse_fn: fn_name.clone(),
                        blocking_fn: chain.last().cloned().unwrap_or_default(),
                        call_chain: chain,
                    });
                }
            }
        }

        violations
    }

    /// BFS to find the shortest call chain from a function to a blocking leaf
    pub(crate) fn find_blocking_chain(&self, start: &str) -> Option<Vec<String>> {
        let mut visited = HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(vec![start.to_string()]);

        while let Some(path) = queue.pop_front() {
            let current = path.last().expect("path always has start element");
            if visited.contains(current) {
                continue;
            }
            visited.insert(current.clone());

            // Check if current is a known blocking operation
            if self.is_known_blocking(current) {
                return Some(path);
            }

            // Check if current has blocking attribute (explicit @blocking)
            if let Some(attrs) = self.fn_attributes.get(current) {
                if attrs.is_blocking && path.len() > 1 {
                    return Some(path);
                }
            }

            // Expand children
            if let Some(callees) = self.call_edges.get(current) {
                for callee in callees {
                    if !visited.contains(callee) {
                        let mut new_path = path.clone();
                        new_path.push(callee.clone());
                        queue.push_back(new_path);
                    }
                }
            }
        }

        None
    }

    // =========================================================================
    // Public query API (used by pulse_injection and sync_verifier)
    // =========================================================================

    /// Check if a function is blocking (direct or transitive)
    pub fn is_blocking(&self, name: &str) -> bool {
        self.fn_attributes.get(name)
            .map(|a| a.is_blocking)
            .unwrap_or(false)
    }

    /// Check if a function requires Context
    pub fn requires_context(&self, name: &str) -> bool {
        self.fn_attributes.get(name)
            .map(|a| a.requires_context)
            .unwrap_or(false)
    }

    /// Get all functions that are transitively blocking
    pub fn blocking_functions(&self) -> Vec<&str> {
        self.fn_attributes.iter()
            .filter(|(_, a)| a.is_blocking)
            .map(|(n, _)| n.as_str())
            .collect()
    }

    /// Get direct callees of a function
    pub fn callees_of(&self, name: &str) -> &[String] {
        self.call_edges.get(name)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    // =========================================================================
    // Public helpers for cross-module testing (used by pulse_injection tests)
    // =========================================================================

    /// Inject call edges directly (for test setup without parsing)
    pub fn inject_edges(&mut self, fn_name: &str, callees: Vec<String>) {
        self.call_edges.insert(fn_name.to_string(), callees);
        // Ensure the function has attributes entry
        self.fn_attributes.entry(fn_name.to_string())
            .or_default();
    }

    /// Inject function attributes directly (for test setup without parsing)
    pub fn inject_attributes(&mut self, fn_name: &str, attrs: FnAttributes) {
        self.fn_attributes.insert(fn_name.to_string(), attrs);
    }

    /// Run propagation phase (exposed for test setup)
    pub fn run_propagation(&mut self) {
        self.propagate();
    }

    /// Run pulse safety verification (exposed for cross-module tests)
    pub fn verify_pulse_safety_external(&self) -> Vec<SafetyViolation> {
        self.verify_pulse_safety()
    }
}

impl Default for CallGraphAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

