use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use crate::codegen::expr::emit_expr;
use std::collections::HashMap;

pub fn emit_io_intrinsic(
    ctx: &mut LoweringContext,
    out: &mut String,
    name: &str,
    args: &[syn::Expr],
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<Option<(String, Type)>, String> {
    match name {
        "println" | "print" => {
            let add_newline = name == "println";
            if args.is_empty() {
                if add_newline { ctx.emit_print_literal(out, "\n")?; }
                return Ok(Some(("%unit".to_string(), Type::Unit)));
            }

            let is_fstring = matches!(&args[0], syn::Expr::Macro(m) 
                if m.mac.path.segments.last()
                    .map(|s| s.ident.to_string())
                    .unwrap_or_default() == "__fstring__");

            if is_fstring {
                let macro_expr = match &args[0] { syn::Expr::Macro(m) => m, _ => unreachable!() };
                let tokens_str = macro_expr.mac.tokens.to_string();
                let content = tokens_str.trim_matches('"');
                let fstring_segments = ctx.parse_fstring_segments(content);
                for seg in &fstring_segments {
                    match seg {
                        crate::codegen::context::FStringSegment::Literal(s) => {
                            if !s.is_empty() { ctx.emit_print_literal(out, s)?; }
                        }
                        crate::codegen::context::FStringSegment::Expr(expr_str, _) => {
                            let parsed: syn::Expr = syn::parse_str(expr_str)
                                .map_err(|e| format!("println f-string expr parse error: {} (expr: {})", e, expr_str))?;
                            let (val, ty) = emit_expr(ctx, out, &parsed, local_vars, None)?;
                            ctx.emit_print_typed(out, &val, &ty)?;
                        }
                    }
                }
                if add_newline { ctx.emit_print_literal(out, "\n")?; }
                return Ok(Some(("%unit".to_string(), Type::Unit)));
            }

            let format_string = match &args[0] {
                syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) => s.value(),
                _ => return Err("println!() first argument must be a string literal".to_string()),
            };

            let mut segments = Vec::new();
            let mut current = String::new();
            let mut chars = format_string.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '{' {
                    if chars.peek() == Some(&'{') { chars.next(); current.push('{'); }
                    else if chars.peek() == Some(&'}') {
                        chars.next();
                        if !current.is_empty() { segments.push((current.clone(), false)); current.clear(); }
                        segments.push(("{}".to_string(), true));
                    } else { return Err("Named format specifiers not yet supported".to_string()); }
                } else if c == '}' {
                    if chars.peek() == Some(&'}') { chars.next(); current.push('}'); }
                    else { return Err("Unmatched } in format string".to_string()); }
                } else { current.push(c); }
            }
            if !current.is_empty() { segments.push((current, false)); }

            let placeholder_count = segments.iter().filter(|(_, is_ph)| *is_ph).count();
            if placeholder_count != args.len() - 1 {
                return Err(format!("println!() expects {} arguments but got {}", placeholder_count, args.len() - 1));
            }

            let mut arg_idx = 1;
            for (segment, is_placeholder) in &segments {
                if *is_placeholder {
                    let (val, ty) = emit_expr(ctx, out, &args[arg_idx], local_vars, None)?;
                    ctx.emit_print_typed(out, &val, &ty)?;
                    arg_idx += 1;
                } else if !segment.is_empty() {
                    ctx.emit_print_literal(out, segment)?;
                }
            }
            if add_newline { ctx.emit_print_literal(out, "\n")?; }
            Ok(Some(("%unit".to_string(), Type::Unit)))
        }
        _ => Ok(None),
    }
}
