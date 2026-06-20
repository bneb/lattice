use crate::grammar::*;
use std::collections::{HashMap, HashSet};
use syn::visit::{self, Visit};

struct CallVisitor {
    callees: Vec<String>,
}

impl<'ast> Visit<'ast> for CallVisitor {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(p) = &*node.func {
            if let Some(segment) = p.path.segments.last() {
                self.callees.push(segment.ident.to_string());
            }
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        self.callees.push(node.method.to_string());
        visit::visit_expr_method_call(self, node);
    }
}

pub struct PurityAnalyzer;

impl PurityAnalyzer {
    pub fn analyze(files: &[&SaltFile]) -> HashSet<String> {
        let mut calls_graph: HashMap<String, Vec<String>> = HashMap::new();

        for file in files {
            for item in &file.items {
                if let Item::Fn(f) = item {
                    let mut visitor = CallVisitor { callees: Vec::new() };
                    Self::visit_block(&f.body, &mut visitor);
                    calls_graph.insert(f.name.to_string(), visitor.callees);
                }
            }
        }

        // Transitive closure
        let mut can_free = HashSet::new();
        can_free.insert("free".to_string());
        can_free.insert("drop".to_string());

        let mut changed = true;
        while changed {
            changed = false;
            for (caller, callees) in &calls_graph {
                if !can_free.contains(caller)
                    && callees.iter().any(|c| can_free.contains(c)) {
                        can_free.insert(caller.clone());
                        changed = true;
                    }
            }
        }

        can_free
    }

    fn visit_block(block: &SaltBlock, visitor: &mut CallVisitor) {
        for stmt in &block.stmts {
            Self::visit_stmt(stmt, visitor);
        }
    }

    fn visit_stmt(stmt: &Stmt, visitor: &mut CallVisitor) {
        match stmt {
            Stmt::Syn(s) => visitor.visit_stmt(s),
            Stmt::Expr(e, _) | Stmt::Invariant(e) | Stmt::Move(e) => {
                visitor.visit_expr(e);
            }
            Stmt::While(w) => {
                visitor.visit_expr(&w.cond);
                Self::visit_block(&w.body, visitor);
            }
            Stmt::For(f) => {
                visitor.visit_expr(&f.iter);
                Self::visit_block(&f.body, visitor);
            }
            Stmt::If(i) => {
                visitor.visit_expr(&i.cond);
                Self::visit_block(&i.then_branch, visitor);
                let mut current_else = &i.else_branch;
                while let Some(e) = current_else {
                    match &**e {
                        SaltElse::Block(b) => {
                            Self::visit_block(b, visitor);
                            break;
                        }
                        SaltElse::If(elif) => {
                            visitor.visit_expr(&elif.cond);
                            Self::visit_block(&elif.then_branch, visitor);
                            current_else = &elif.else_branch;
                        }
                    }
                }
            }
            Stmt::Match(m) => {
                visitor.visit_expr(&m.scrutinee);
                for arm in &m.arms {
                    Self::visit_block(&arm.body, visitor);
                }
            }
            Stmt::LetElse(l) => {
                visitor.visit_expr(&l.init);
                Self::visit_block(&l.else_block, visitor);
            }
            Stmt::MapWindow { addr, size, body, .. } => {
                visitor.visit_expr(addr);
                visitor.visit_expr(size);
                Self::visit_block(body, visitor);
            }
            Stmt::WithRegion { body, .. } | Stmt::Unsafe(body) | Stmt::DynamicCheck(body) => {
                Self::visit_block(body, visitor);
            }
            Stmt::Return(e) => {
                if let Some(expr) = e {
                    visitor.visit_expr(expr);
                }
            }
            Stmt::Loop(b) => {
                Self::visit_block(b, visitor);
            }
            Stmt::Break | Stmt::Continue => {}
        }
    }
}
