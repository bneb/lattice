//! Helper methods for the AST interpreter.
//!
//! Extracted from interpreter.rs to keep file sizes under the line limit.

use std::collections::HashMap;
use std::fmt::Write;
use crate::interpreter::{Interpreter, Value};

impl Interpreter {
    pub(crate) fn eval_expr_call(&mut self, call: &syn::ExprCall, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        let mut args = Vec::new();
        for arg in &call.args {
            let val = self.eval_expr(arg, scope)?;
            if val.is_return() { return Ok(val); }
            args.push(val);
        }

        let fn_name = match &*call.func {
            syn::Expr::Path(p) => {
                p.path.segments.iter()
                    .map(|s| s.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::")
            }
            _ => return Err("Unsupported call target".into()),
        };

        self.call_function(&fn_name, &args)
    }

    pub(crate) fn eval_expr_method_call(&mut self, mc: &syn::ExprMethodCall, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        let receiver = self.eval_expr(&mc.receiver, scope)?;
        if receiver.is_return() { return Ok(receiver); }
        let method = mc.method.to_string();
        match method.as_str() {
            "abs" => Ok(Value::I64(receiver.as_i64().abs())),
            _ => Ok(receiver),
        }
    }

    pub(crate) fn eval_expr_if(&mut self, if_expr: &syn::ExprIf, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        let cond = self.eval_expr(&if_expr.cond, scope)?;
        if cond.is_return() { return Ok(cond); }
        if cond.as_bool() {
            self.exec_syn_block(&if_expr.then_branch, scope)
        } else if let Some((_, else_branch)) = &if_expr.else_branch {
            self.eval_expr(else_branch, scope)
        } else {
            Ok(Value::Unit)
        }
    }

    pub(crate) fn eval_expr_while(&mut self, while_expr: &syn::ExprWhile, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        loop {
            let cond = self.eval_expr(&while_expr.cond, scope)?;
            if cond.is_return() { return Ok(cond); }
            if !cond.as_bool() { break; }
            let result = self.exec_syn_block(&while_expr.body, scope)?;
            if result.is_return() { return Ok(result); }
        }
        Ok(Value::Unit)
    }

    pub(crate) fn eval_expr_for_loop(&mut self, for_loop: &syn::ExprForLoop, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        let iter_name = self.extract_pat_name(&for_loop.pat);
        if let syn::Expr::Range(range) = &*for_loop.expr {
            let start = if let Some(s) = &range.start { self.eval_expr(s, scope)?.as_i64() } else { 0 };
            let end = if let Some(e) = &range.end { self.eval_expr(e, scope)?.as_i64() } else { return Err("Unbounded range".into()); };
            for i in start..end {
                scope.insert(iter_name.clone(), Value::I64(i));
                let result = self.exec_syn_block(&for_loop.body, scope)?;
                if result.is_return() { return Ok(result); }
            }
            Ok(Value::Unit)
        } else {
            Err("Only range-based for loops supported".into())
        }
    }

    pub(crate) fn eval_expr_cast(&mut self, cast: &syn::ExprCast, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        let val = self.eval_expr(&cast.expr, scope)?;
        if val.is_return() { return Ok(val); }
        if let syn::Type::Path(tp) = &*cast.ty {
            if let Some(seg) = tp.path.segments.last() {
                match seg.ident.to_string().as_str() {
                    "i32" => return Ok(Value::I32(val.as_i64() as i32)),
                    "i64" => return Ok(Value::I64(val.as_i64())),
                    "bool" => return Ok(Value::Bool(val.as_bool())),
                    _ => {}
                }
            }
        }
        Ok(val)
    }

    pub(crate) fn exec_syn_block(&mut self, block: &syn::Block, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        let mut last = Value::Unit;
        for stmt in &block.stmts {
            last = self.exec_syn_stmt(stmt, scope)?;
            if last.is_return() { return Ok(last); }
        }
        Ok(last)
    }

    pub(crate) fn eval_fstring(&mut self, template: &str, scope: &HashMap<String, Value>) -> Result<Value, String> {
        let mut result = String::new();
        let mut chars = template.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '{' {
                let mut var_expr = String::new();
                let mut depth = 1;
                while let Some(&nc) = chars.peek() {
                    if nc == '{' { depth += 1; }
                    if nc == '}' { depth -= 1; if depth == 0 { chars.next(); break; } }
                    var_expr.push(nc);
                    chars.next();
                }
                let var_name = var_expr.trim().to_string();
                if let Some(val) = scope.get(&var_name) {
                    write!(result, "{}", val).ok();
                } else {
                    // Try evaluating as simple expression
                    // For now just try to look up + for "x as i64" patterns, strip " as ..."
                    let base = var_name.split(" as ").next().unwrap_or(&var_name).trim();
                    if let Some(val) = scope.get(base) {
                        write!(result, "{}", val).ok();
                    } else {
                        write!(result, "{{{}}}", var_name).ok();
                    }
                }
            } else if c == '\\' {
                if let Some(nc) = chars.next() {
                    match nc { 'n' => result.push('\n'), 't' => result.push('\t'), _ => { result.push('\\'); result.push(nc); } }
                }
            } else {
                result.push(c);
            }
        }
        Ok(Value::Str(result))
    }

    pub(crate) fn check_steps(&mut self) -> Result<(), String> {
        self.steps += 1;
        if self.steps > self.max_steps {
            Err(format!("Execution limit exceeded ({} steps). Possible infinite loop.", self.max_steps))
        } else {
            Ok(())
        }
    }
}
