use crate::codegen::context::CodegenContext;
use crate::codegen::context::FStringSegment;

pub fn native_fstring_expand_impl(_ctx: &CodegenContext, content: &str) -> String {
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
    code.push_str("{ let mut __h = std.string.InterpolatedStringHandler::new(");
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
    if !clean_hex.len().is_multiple_of(2) {
        return "Vec::<u8>::new()".to_string();
    }
    if clean_hex.is_empty() { return "Vec::<u8>::new()".to_string(); }
    let mut bytes = Vec::new();
    for i in (0..clean_hex.len()).step_by(2) {
        let byte_str = &clean_hex[i..i + 2];
        if u8::from_str_radix(byte_str, 16).is_ok() {
            bytes.push(format!("0x{}", byte_str.to_uppercase()));
        }
    }
    format!("Vec::<u8>::from_array([{}])", bytes.join(", "))
}

pub fn native_target_fstring_expand_impl(_ctx: &CodegenContext, target: &str, content: &str) -> String {
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
    code.push('}');
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_backslash() {
        assert_eq!(escape_string_impl("a\\b"), "a\\\\b");
    }

    #[test]
    fn test_escape_double_quote() {
        assert_eq!(escape_string_impl("hello\"world"), "hello\\\"world");
    }

    #[test]
    fn test_escape_newline() {
        assert_eq!(escape_string_impl("line1\nline2"), "line1\\nline2");
    }

    #[test]
    fn test_escape_tab() {
        assert_eq!(escape_string_impl("col1\tcol2"), "col1\\tcol2");
    }

    #[test]
    fn test_escape_carriage_return() {
        assert_eq!(escape_string_impl("a\rb"), "a\\0Db");
    }

    #[test]
    fn test_escape_multiple() {
        assert_eq!(escape_string_impl("\"hello\nworld\""), "\\\"hello\\nworld\\\"");
    }

    #[test]
    fn test_format_spec_none() {
        assert_eq!(format_with_spec_v4_impl("x", None), "x");
    }

    #[test]
    fn test_format_spec_float_default() {
        let result = format_with_spec_v4_impl("val", Some("f"));
        assert_eq!(result, "fmt_f64(val, 6)");
    }

    #[test]
    fn test_format_spec_float_precision() {
        let result = format_with_spec_v4_impl("val", Some(".2f"));
        assert_eq!(result, "fmt_f64(val, 2)");
    }

    #[test]
    fn test_format_spec_decimal() {
        assert_eq!(format_with_spec_v4_impl("val", Some("d")), "val");
    }

    #[test]
    fn test_format_spec_hex() {
        assert_eq!(format_with_spec_v4_impl("val", Some("x")), "fmt_hex(val)");
    }

    #[test]
    fn test_format_spec_hex_upper() {
        assert_eq!(format_with_spec_v4_impl("val", Some("X")), "fmt_hex(val)");
    }

    #[test]
    fn test_format_spec_binary() {
        assert_eq!(format_with_spec_v4_impl("val", Some("b")), "fmt_bin(val)");
    }

    #[test]
    fn test_format_spec_empty() {
        assert_eq!(format_with_spec_v4_impl("val", Some("")), "val");
    }

    #[test]
    fn test_parse_literal_only() {
        let segs = parse_fstring_segments_impl("hello");
        assert_eq!(segs.len(), 1);
        assert!(matches!(&segs[0], FStringSegment::Literal(s) if s == "hello"));
    }

    #[test]
    fn test_parse_single_expr() {
        let segs = parse_fstring_segments_impl("{x}");
        assert_eq!(segs.len(), 1);
        assert!(matches!(&segs[0], FStringSegment::Expr(e, None) if e == "x"));
    }

    #[test]
    fn test_parse_expr_with_spec() {
        let segs = parse_fstring_segments_impl("{val:.2f}");
        assert_eq!(segs.len(), 1);
        assert!(matches!(&segs[0], FStringSegment::Expr(e, Some(s)) if e == "val" && s == ".2f"));
    }

    #[test]
    fn test_parse_mixed_literal_and_expr() {
        let segs = parse_fstring_segments_impl("hello {name}");
        assert_eq!(segs.len(), 2);
        assert!(matches!(&segs[0], FStringSegment::Literal(s) if s == "hello "));
        assert!(matches!(&segs[1], FStringSegment::Expr(e, None) if e == "name"));
    }

    #[test]
    fn test_parse_escaped_brace() {
        let segs = parse_fstring_segments_impl("{{hello}}");
        assert_eq!(segs.len(), 1);
        assert!(matches!(&segs[0], FStringSegment::Literal(s) if s == "{hello}"));
    }

    #[test]
    fn test_parse_empty() {
        let segs = parse_fstring_segments_impl("");
        assert!(segs.is_empty());
    }

    #[test]
    fn test_parse_nested_braces() {
        let segs = parse_fstring_segments_impl("{fn({a})}");
        assert_eq!(segs.len(), 1);
        assert!(matches!(&segs[0], FStringSegment::Expr(e, None) if e == "fn({a})"));
    }

    #[test]
    fn test_hex_expand_empty() {
        let result = native_hex_expand_impl("");
        assert_eq!(result, "Vec::<u8>::new()");
    }

    #[test]
    fn test_hex_expand_odd_length() {
        let result = native_hex_expand_impl("a");
        assert_eq!(result, "Vec::<u8>::new()");
    }

    #[test]
    fn test_hex_expand_valid() {
        let result = native_hex_expand_impl("deadbeef");
        assert_eq!(result, "Vec::<u8>::from_array([0xDE, 0xAD, 0xBE, 0xEF])");
    }

    #[test]
    fn test_hex_expand_with_whitespace() {
        let result = native_hex_expand_impl("de ad be ef");
        assert_eq!(result, "Vec::<u8>::from_array([0xDE, 0xAD, 0xBE, 0xEF])");
    }

    #[test]
    fn test_hex_expand_invalid_skipped() {
        let result2 = native_hex_expand_impl("ffzz");
        assert!(result2.contains("0xFF"));
        assert!(!result2.contains("ZZ"));
    }
}
