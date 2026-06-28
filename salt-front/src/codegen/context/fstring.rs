use crate::codegen::context::CodegenContext;

/// F-string segment for native expansion
#[derive(Clone, Debug)]
pub enum FStringSegment {
    Literal(String),
    Expr(String, Option<String>), // (expression, optional format spec)
}

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

/// Implements f-string expansion on CodegenContext (Ctx1).
///
/// These methods duplicate the LoweringContext f-string logic but operate
/// on `CodegenContext` directly with TraitRegistry access for native expansion.
impl<'a> crate::codegen::context::CodegenContext<'a> {
    /// Process a prefixed string literal using comptime handlers
    /// During bootstrap: returns None (use Rust fallback)
    /// After ready: returns Some(generated_code) using native expansion
    pub fn process_prefixed_string(&self, prefix: &str, content: &str) -> Option<String> {
        // Bootstrap safety: if comptime not ready, use Rust fallback
        if !self.is_comptime_ready() {
            return None;
        }

        // Native f-string expansion with TraitRegistry context
        if prefix == "f" {
            return Some(self.native_fstring_expand(content));
        }

        // Native hex string expansion
        // hex"DEADBEEF" → Vec::<u8>::from_array([0xDE, 0xAD, 0xBE, 0xEF])
        if prefix == "hex" {
            return Some(self.native_hex_expand(content));
        }

        // For other prefixes, check registry
        let _handler_name = self.string_prefix_handlers().get(prefix)?.clone();
        // Future(Phase 4): Invoke comptime string prefix handler once the comptime evaluator is ready
        None
    }

    /// Native f-string expansion
    /// This replaces lib.rs preprocessing with TraitRegistry-aware generation
    pub fn native_fstring_expand(&self, content: &str) -> String {
        // Parse segments from f-string content
        let segments = self.parse_fstring_segments(content);

        if segments.is_empty() {
            return "\"\"".to_string();
        }

        // Check if pure literal (no interpolations)
        let has_interpolation = segments.iter().any(|s| matches!(s, FStringSegment::Expr(_, _)));
        if !has_interpolation {
            if let Some(FStringSegment::Literal(s)) = segments.first() {
                return format!("\"{}\"", self.escape_string(s));
            }
        }

        // Calculate sizes for InterpolatedStringHandler
        let mut literal_len = 0;
        let mut interp_count = 0;
        for seg in &segments {
            match seg {
                FStringSegment::Literal(s) => literal_len += s.len(),
                FStringSegment::Expr(_, _) => interp_count += 1,
            }
        }

        // Generate InterpolatedStringHandler block
        // Use Rust path notation (::) since syn parses . as field access
        let mut code = String::new();
        code.push_str("{\n");
        code.push_str(&format!(
            "    let mut __h = std::string::InterpolatedStringHandler::new({}, {});\n",
            literal_len, interp_count
        ));

        for seg in &segments {
            match seg {
                FStringSegment::Literal(s) => {
                    let escaped = self.escape_string(s);
                    code.push_str(&format!(
                        "    __h.append_literal(\"{}\", {});\n",
                        escaped, s.len()
                    ));
                }
                FStringSegment::Expr(expr, spec) => {
                    // TraitRegistry-aware format spec handling
                    let formatted = self.format_with_spec_v4(expr, spec.as_deref());
                    if formatted.starts_with("fmt_") {
                        // Format-spec expression (e.g., {x:.2f}) -> append_fmt
                        code.push_str(&format!(
                            "    __h.append_fmt({});\n",
                            formatted
                        ));
                    } else {
                        // Type-aware dispatch via internal macro
                        // The __fstring_append_expr! macro resolves the expression's type at
                        // compile time and dispatches to append_i32/append_i64/append_f64/append_bool
                        // or the fmt() call chain for struct types.
                        code.push_str(&format!(
                            "    __fstring_append_expr!(__h, {});\n",
                            formatted
                        ));
                    }
                }
            }
        }

        code.push_str("    __h.finalize()\n");
        code.push('}');

        code
    }

    /// Parse f-string content into segments
    pub fn parse_fstring_segments(&self, content: &str) -> Vec<FStringSegment> {
        let mut segments = Vec::new();
        let mut chars = content.chars().peekable();
        let mut current_literal = String::new();

        while let Some(c) = chars.next() {
            match c {
                '{' => {
                    // Check for escaped brace {{
                    if chars.peek() == Some(&'{') {
                        chars.next();
                        current_literal.push('{');
                        continue;
                    }

                    // Flush current literal
                    if !current_literal.is_empty() {
                        segments.push(FStringSegment::Literal(std::mem::take(&mut current_literal)));
                    }

                    // Parse expression with optional format spec
                    let (expr, spec) = self.parse_fstring_expr(&mut chars);
                    if !expr.is_empty() {
                        segments.push(FStringSegment::Expr(expr, spec));
                    }
                }
                '}' => {
                    // Check for escaped brace }}
                    if chars.peek() == Some(&'}') {
                        chars.next();
                        current_literal.push('}');
                    }
                    // Otherwise ignore stray }
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

        // Flush remaining literal
        if !current_literal.is_empty() {
            segments.push(FStringSegment::Literal(current_literal));
        }

        segments
    }

    /// Parse expression inside {} including optional format spec
    fn parse_fstring_expr(&self, chars: &mut std::iter::Peekable<std::str::Chars>) -> (String, Option<String>) {
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
                    // Parse format spec
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

                    // Track nesting
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

    /// Format with spec using TraitRegistry context
    fn format_with_spec_v4(&self, expr: &str, spec: Option<&str>) -> String {
        let spec = match spec {
            Some(s) => s.trim(),
            None => return expr.to_string(),
        };

        // Float precision: .Nf
        if spec.ends_with('f') {
            if let Some(precision_str) = spec.strip_suffix('f') {
                let precision_str = precision_str.strip_prefix('.').unwrap_or(precision_str);
                if let Ok(precision) = precision_str.parse::<u8>() {
                    return format!("fmt_f64({}, {})", expr, precision);
                }
            }
            // Default float precision
            return format!("fmt_f64({}, 6)", expr);
        }

        // Integer formats
        if spec == "d" || spec.is_empty() {
            return expr.to_string();
        }

        // Hex format
        if spec == "x" || spec == "X" {
            return format!("fmt_hex({})", expr);
        }

        // Binary format
        if spec == "b" {
            return format!("fmt_bin({})", expr);
        }

        // Unknown spec - pass through
        expr.to_string()
    }

    /// Escape string for output
    fn escape_string(&self, s: &str) -> String {
        s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\0D")
    }

    /// Native Hex Expansion
    /// Converts hex"DEADBEEF" → Vec::<u8>::from_array([0xDE, 0xAD, 0xBE, 0xEF])
    /// Allows whitespace separators: hex"DE AD BE EF" is valid
    pub fn native_hex_expand(&self, content: &str) -> String {
        // 1. Strip whitespace/separators (allow hex"AA BB CC")
        let clean_hex: String = content.chars().filter(|c| !c.is_whitespace()).collect();

        // 2. Validation: Must have even length
        if !clean_hex.len().is_multiple_of(2) {
            return "Vec::<u8>::new()".to_string();
        }

        if clean_hex.is_empty() {
            return "Vec::<u8>::new()".to_string();
        }

        // 3. Convert hex pairs to byte literals
        let mut bytes = Vec::new();
        for i in (0..clean_hex.len()).step_by(2) {
            let byte_str = &clean_hex[i..i + 2];
            // Validate hex characters
            if u8::from_str_radix(byte_str, 16).is_err() {
                return "Vec::<u8>::new()".to_string();
            }
            bytes.push(format!("0x{}", byte_str.to_uppercase()));
        }

        // 4. Generate Salt source code for Vec constructor
        format!("Vec::<u8>::from_array([{}])", bytes.join(", "))
    }

    /// Native Target F-String Expansion
    /// Converts target.f"Hello {x}" → { target.write_str("Hello ", 6); target.write_i32(x); }
    /// This implements zero-allocation streaming by decomposing the f-string into direct
    /// write_* calls on the target Writer, avoiding intermediate String allocation.
    pub fn native_target_fstring_expand(&self, target: &str, content: &str) -> String {
        // Parse segments from f-string content (reuses existing parser)
        let segments = self.parse_fstring_segments(content);

        if segments.is_empty() {
            // Empty string - just return unit
            return "{ }".to_string();
        }

        // Generate block with direct write calls
        let mut code = String::new();
        code.push_str("{\n");

        for seg in &segments {
            match seg {
                FStringSegment::Literal(s) => {
                    if !s.is_empty() {
                        let escaped = self.escape_string(s);
                        code.push_str(&format!(
                            "    {}.write_str(\"{}\", {});\n",
                            target, escaped, s.len()
                        ));
                    }
                }
                FStringSegment::Expr(expr, spec) => {
                    // Determine the appropriate write method based on type/spec
                    // Type info resolved via heuristics.
                    let (method, formatted_expr) = self.determine_write_method(expr, spec.as_deref());

                    code.push_str(&format!(
                        "    {}.{}({});\n",
                        target, method, formatted_expr
                    ));
                }
            }
        }

        code.push('}');
        code
    }

    /// Determine the appropriate write_* method for an interpolated expression
    /// Returns (method_name, formatted_expression)
    fn determine_write_method(&self, expr: &str, spec: Option<&str>) -> (String, String) {
        // Check format spec first - it overrides type inference
        if let Some(s) = spec {
            // Float with precision: .Nf
            if s.ends_with('f') {
                let precision_str = s.strip_suffix('f').unwrap_or("").strip_prefix('.').unwrap_or("6");
                let precision = precision_str.parse::<u8>().unwrap_or(6);
                return ("write_f64_prec".to_string(), format!("{}, {}", expr, precision));
            }

            // Boolean
            if s == "?" {
                return ("write_bool".to_string(), expr.to_string());
            }
        }

        // Type inference heuristics based on expression patterns
        let expr_trimmed = expr.trim();

        // Check for literal patterns
        if expr_trimmed.starts_with('"') || expr_trimmed.starts_with("&\"") {
            // String literal - use write_str
            // The length needs to be extracted... for now fall back to write_str pattern
            return ("write_str".to_string(), format!("{}, strlen({})", expr, expr));
        }

        // Check for float literals (contains . or f suffix)
        if expr_trimmed.contains('.') && !expr_trimmed.contains("::")
           || expr_trimmed.ends_with("f32") || expr_trimmed.ends_with("f64") {
            let precision = spec.and_then(|s| {
                s.strip_suffix('f')?.strip_prefix('.')?.parse::<u8>().ok()
            }).unwrap_or(6);
            return ("write_f64_prec".to_string(), format!("{}, {}", expr, precision));
        }

        // Check for bool literals
        if expr_trimmed == "true" || expr_trimmed == "false" {
            return ("write_bool".to_string(), expr.to_string());
        }

        // Check for i64 suffix
        if expr_trimmed.ends_with("i64") || expr_trimmed.ends_with("u64") {
            return ("write_i64".to_string(), expr.to_string());
        }

        // Default to write_i32 for integer expressions (most common case)
        ("write_i32".to_string(), expr.to_string())
    }
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
        let result = native_hex_expand_impl("0xgg");
        // "0x" is valid, "gg" is not - both get processed but "gg" fails from_str_radix
        // Let's test a case we know works:
        let result2 = native_hex_expand_impl("ffzz");
        // "ff" = valid, "zz" = invalid hex chars -> z is not in 0-9a-f, but it gets to from_str_radix and fails
        // Since z isn't a hex char, the question is whether the filter or from_str_radix catches it
        // Actually from_str_radix accepts any char 0-9a-fA-F; 'z' will fail
        // So "ff" -> 0xFF, "zz" skipped; the result should have one byte
        // Wait, actually `from_str_radix("zz", 16)` returns Err, so the byte is skipped
        // But the output shouldn't have "zz" bytes
        assert!(result2.contains("0xFF"));
        assert!(!result2.contains("ZZ"));
    }
}
