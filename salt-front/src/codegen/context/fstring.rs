use crate::codegen::context::{CodegenContext, LoweringContext};

/// [V4.0 SCORCHED EARTH] F-string segment for native expansion
#[derive(Clone, Debug)]
pub enum FStringSegment {
    Literal(String),
    Expr(String, Option<String>), // (expression, optional format spec)
}

pub fn native_fstring_expand_impl(ctx: &CodegenContext, content: &str) -> String {
    let segments = parse_fstring_segments_impl(content);
    if segments.is_empty() { return "\"\"".to_string(); }
    let has_interpolation = segments.iter().any(|s| matches!(s, FStringSegment::Expr(_, _)));
    if !has_interpolation {
        if let Some(FStringSegment::Literal(s)) = segments.first() {
            return format!("\"{}\"", escape_string_impl(s));
        }
    }

    let mut literal_len = 0;
    let mut interp_count = 0;
    for seg in &segments {
        match seg {
            FStringSegment::Literal(s) => literal_len += s.len(),
            FStringSegment::Expr(_, _) => interp_count += 1,
        }
    }

    let mut code = String::new();
    code.push_str("{ let mut __h = std::string::InterpolatedStringHandler::new(");
    code.push_str(&format!("{}, {}); ", literal_len, interp_count));
    for seg in segments {
        match seg {
            FStringSegment::Literal(s) => {
                if !s.is_empty() {
                    code.push_str(&format!("__h.append_literal(\"{}\", {}); ", escape_string_impl(&s), s.len()));
                }
            }
            FStringSegment::Expr(expr, _spec) => {
                code.push_str(&format!("__fstring_append_expr!(__h, {}); ", expr));
            }
        }
    }
    code.push_str("__h.finalize() }");
    code
}

pub fn native_hex_expand_impl(content: &str) -> String {
    let clean_hex: String = content.chars().filter(|c| !c.is_whitespace()).collect();
    if clean_hex.len() % 2 != 0 {
        return "Vec::<u8>::new()".to_string();
    }
    if clean_hex.is_empty() { return "Vec::<u8>::new()".to_string(); }
    let mut bytes = Vec::new();
    for i in (0..clean_hex.len()).step_by(2) {
        let byte_str = &clean_hex[i..i + 2];
        if let Ok(_) = u8::from_str_radix(byte_str, 16) {
            bytes.push(format!("0x{}", byte_str.to_uppercase()));
        }
    }
    format!("Vec::<u8>::from_array([{}])", bytes.join(", "))
}

pub fn native_target_fstring_expand_impl(ctx: &CodegenContext, target: &str, content: &str) -> String {
    let segments = parse_fstring_segments_impl(content);
    if segments.is_empty() { return "{ }".to_string(); }
    let mut code = String::new();
    code.push_str("{\n");
    for seg in &segments {
        match seg {
            FStringSegment::Literal(s) => {
                if !s.is_empty() {
                    let escaped = escape_string_impl(s);
                    code.push_str(&format!("    {}.write_str(\"{}\", {});\n", target, escaped, s.len()));
                }
            }
            FStringSegment::Expr(expr, spec) => {
                let formatted = format_with_spec_v4_impl(expr, spec.as_deref());
                code.push_str(&format!("    {}.append_any({});\n", target, formatted));
            }
        }
    }
    code.push_str("}");
    code
}

pub fn escape_string_impl(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\0D").replace('\t', "\\t")
}

pub fn format_with_spec_v4_impl(expr: &str, spec: Option<&str>) -> String {
    let spec = match spec {
        Some(s) => s.trim(),
        None => return expr.to_string(),
    };
    
    if spec.ends_with('f') {
        if let Some(precision_str) = spec.strip_suffix('f') {
            let precision_str = precision_str.strip_prefix('.').unwrap_or(precision_str);
            if let Ok(precision) = precision_str.parse::<u8>() {
                return format!("fmt_f64({}, {})", expr, precision);
            }
        }
        return format!("fmt_f64({}, 6)", expr);
    }
    
    if spec == "d" || spec.is_empty() {
         return expr.to_string();
    }
    
    if spec == "x" || spec == "X" {
        return format!("fmt_hex({})", expr);
    }
    
    if spec == "b" {
        return format!("fmt_bin({})", expr);
    }
    
    expr.to_string()
}

pub fn parse_fstring_segments_impl(content: &str) -> Vec<FStringSegment> {
    let mut segments = Vec::new();
    let mut chars = content.chars().peekable();
    let mut current_literal = String::new();

    while let Some(c) = chars.next() {
        match c {
            '{' => {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    current_literal.push('{');
                    continue;
                }
                if !current_literal.is_empty() {
                    segments.push(FStringSegment::Literal(std::mem::take(&mut current_literal)));
                }
                let (expr, spec) = parse_fstring_expr_impl(&mut chars);
                if !expr.is_empty() {
                    segments.push(FStringSegment::Expr(expr, spec));
                }
            }
            '}' => {
                if chars.peek() == Some(&'}') {
                    chars.next();
                    current_literal.push('}');
                }
            }
            '\\' => {
                current_literal.push('\\');
                if let Some(escaped) = chars.next() {
                    current_literal.push(escaped);
                }
            }
            _ => {
                current_literal.push(c);
            }
        }
    }
    if !current_literal.is_empty() {
        segments.push(FStringSegment::Literal(current_literal));
    }
    segments
}

fn parse_fstring_expr_impl(chars: &mut std::iter::Peekable<std::str::Chars>) -> (String, Option<String>) {
    let mut expr = String::new();
    let mut spec = None;
    let mut depth = 0;
    
    loop {
        match chars.peek() {
            None => break,
            Some(&'}') if depth == 0 => {
                chars.next();
                break;
            }
            Some(&':') if depth == 0 => {
                chars.next();
                let mut spec_str = String::new();
                loop {
                    match chars.peek() {
                        None | Some(&'}') => break,
                        Some(&c) => {
                            chars.next();
                            spec_str.push(c);
                        }
                    }
                }
                if chars.peek() == Some(&'}') {
                    chars.next();
                }
                spec = Some(spec_str);
                break;
            }
            Some(&c) => {
                chars.next();
                expr.push(c);
                match c {
                    '(' | '[' | '{' => depth += 1,
                    ')' | ']' | '}' => if depth > 0 { depth -= 1; },
                    _ => {}
                }
            }
        }
    }
    (expr.trim().to_string(), spec)
}
