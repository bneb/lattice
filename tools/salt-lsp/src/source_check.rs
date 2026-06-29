//! Source-level diagnostic checks and definition lookup for Salt LSP.
//! Operates on source text directly without full compilation.

use tower_lsp::lsp_types::*;
use std::collections::HashSet;

// =============================================================================
// Public API
// =============================================================================

/// Run all source-level checks and return diagnostics.
pub fn diagnose_source(text: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    diags.extend(check_missing_return(text));
    diags.extend(check_unused_vars(text));
    diags.extend(check_type_mismatch(text));
    diags.extend(check_unknown_var(text));
    diags
}

/// Find the definition position of a variable/param/field in source text.
/// Returns `(line, col)` of the definition that precedes `ref_line`.
pub fn find_var_definition(text: &str, word: &str, ref_line: usize) -> Option<(usize, usize)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut result = None;
    for (idx, line) in lines.iter().enumerate() {
        if idx >= ref_line { break; }
        if let Some(col) = find_definition_in_line(line, word) {
            result = Some((idx, col));
        }
    }
    result
}

// =============================================================================
// Shared Helpers
// =============================================================================

fn mk_diag(line: usize, start: usize, end: usize, msg: &str, sev: DiagnosticSeverity) -> Diagnostic {
    Diagnostic {
        range: Range {
            start: Position { line: line as u32, character: start as u32 },
            end: Position { line: line as u32, character: end as u32 },
        },
        severity: Some(sev),
        source: Some("salt-lsp".to_string()),
        message: msg.to_string(),
        ..Default::default()
    }
}

fn is_word_in_line_from(line: &str, word: &str, min_col: usize) -> bool {
    let bytes = line.as_bytes();
    let w = word.as_bytes();
    if w.is_empty() || min_col >= bytes.len() { return false; }
    let mut i = min_col;
    while i + w.len() <= bytes.len() {
        if &bytes[i..i + w.len()] == w {
            let before = i == 0 || !is_ident_char(bytes[i - 1]);
            let after = i + w.len() >= bytes.len() || !is_ident_char(bytes[i + w.len()]);
            if before && after { return true; }
        }
        i += 1;
    }
    false
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

// =============================================================================
// Missing Return Check
// =============================================================================

pub fn check_missing_return(text: &str) -> Vec<Diagnostic> {
    let lines: Vec<&str> = text.lines().collect();
    let mut diags = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if !fn_has_return_type(line) { i += 1; continue; }
        let body_start = find_fn_body_start(&lines, i);
        let body_end = body_start.and_then(|s| find_matching_brace(&lines, s));
        match (body_start, body_end) {
            (Some(start), Some(end)) => {
                if !has_return_in_range(&lines, start, end) {
                    diags.push(mk_diag(i, 0, line.len(),
                        "Function has a return type but no return statement.",
                        DiagnosticSeverity::ERROR));
                }
                i = end + 1;
            }
            _ => { i += 1; }
        }
    }
    diags
}

fn fn_has_return_type(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("fn ") && t.contains("->")
}

fn find_fn_body_start(lines: &[&str], fn_line: usize) -> Option<usize> {
    for (idx, line) in lines.iter().enumerate().skip(fn_line) {
        if line.contains('{') { return Some(idx); }
    }
    None
}

fn find_matching_brace(lines: &[&str], start: usize) -> Option<usize> {
    let mut depth = 0u32;
    for (idx, line) in lines.iter().enumerate().skip(start) {
        for b in line.bytes() {
            if b == b'{' { depth += 1; }
            else if b == b'}' {
                if depth <= 1 { return Some(idx); }
                depth -= 1;
            }
        }
    }
    None
}

fn has_return_in_range(lines: &[&str], start: usize, end: usize) -> bool {
    for line in lines.iter().take(end + 1).skip(start) {
        let t = line.trim();
        if t.starts_with("//") { continue; }
        if t == "return;" || t.starts_with("return ") || t.contains(" return ") {
            return true;
        }
    }
    false
}

// =============================================================================
// Variable Tracking
// =============================================================================

#[derive(Debug, Clone)]
struct VarDef { name: String, line: usize, col: usize }

fn collect_var_defs(text: &str) -> Vec<VarDef> {
    let mut vars = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let t = line.trim();
        if let Some(v) = extract_let_var(t, line, idx) { vars.push(v); }
        vars.extend(extract_fn_params(t, line, idx));
    }
    vars
}

fn extract_let_var(trimmed: &str, original: &str, idx: usize) -> Option<VarDef> {
    let after = trimmed.strip_prefix("let ")?;
    let after = after.strip_prefix("mut ").unwrap_or(after);
    let end = after.find(|c: char| !c.is_alphanumeric() && c != '_')?;
    let name = &after[..end];
    if name.is_empty() { return None; }
    let col = original.find(name)?;
    Some(VarDef { name: name.to_string(), line: idx, col })
}

fn extract_fn_params(trimmed: &str, original: &str, idx: usize) -> Vec<VarDef> {
    let mut vars = Vec::new();
    if !trimmed.starts_with("fn ") { return vars; }
    let paren = match trimmed.find('(') { Some(p) => p, _ => return vars };
    let close = match trimmed[paren..].find(')') { Some(c) => paren + c, _ => return vars };
    for param in trimmed[paren + 1..close].split(',') {
        let p = param.trim();
        if p.is_empty() { continue; }
        let colon = match p.find(':') { Some(c) => c, _ => continue };
        let pname = p[..colon].trim();
        if pname.is_empty() || !pname.chars().all(|c| c.is_alphanumeric() || c == '_') { continue; }
        if let Some(col) = original.find(pname) {
            vars.push(VarDef { name: pname.to_string(), line: idx, col });
        }
    }
    vars
}

fn is_word_in_line(line: &str, word: &str) -> bool {
    let bytes = line.as_bytes();
    let w = word.as_bytes();
    if w.is_empty() { return false; }
    let mut i = 0;
    while i + w.len() <= bytes.len() {
        if &bytes[i..i + w.len()] == w {
            let before = i == 0 || !is_ident_char(bytes[i - 1]);
            let after = i + w.len() >= bytes.len() || !is_ident_char(bytes[i + w.len()]);
            if before && after { return true; }
        }
        i += 1;
    }
    false
}

// =============================================================================
// Unused Variable Check
// =============================================================================

pub fn check_unused_vars(text: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let vars = collect_var_defs(text);
    let lines: Vec<&str> = text.lines().collect();
    for v in &vars {
        if v.name.starts_with('_') { continue; }
        let used = lines.iter().enumerate().any(|(idx, line)| {
            if idx < v.line || line.trim().starts_with("//") { return false; }
            if idx == v.line {
                let after = v.col + v.name.len();
                return after < line.len() && is_word_in_line_from(line, &v.name, after);
            }
            is_word_in_line(line, &v.name)
        });
        if !used {
            diags.push(mk_diag(v.line, v.col, v.col + v.name.len(),
                &format!("Unused variable `{}`.", v.name),
                DiagnosticSeverity::WARNING));
        }
    }
    diags
}

// =============================================================================
// Type Mismatch Check (string literal to numeric type)
// =============================================================================

pub fn check_type_mismatch(text: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        if !line.contains("let ") { continue; }
        let t = line.trim();
        let decl_ty = match extract_declared_type(t) { Some(d) => d, None => continue };
        if is_numeric_type(&decl_ty) && t.chars().filter(|c| *c == '"').count() >= 2 {
            if let Some(quote) = line.find('"') {
                diags.push(mk_diag(idx, quote, quote + 1,
                    &format!("Type mismatch: expected `{}`, found string literal.", decl_ty),
                    DiagnosticSeverity::ERROR));
            }
        }
    }
    diags
}

fn extract_declared_type(line: &str) -> Option<String> {
    let s = line;
    let let_pos = s.find("let ")?;
    let after_let = &s[let_pos + 4..];
    let name_str = if let Some(s) = after_let.strip_prefix("mut ") { s } else { after_let };
    let colon = name_str.find(':')?;
    let mut depth = 0i32;
    for (i, ch) in name_str[colon + 1..].char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => depth -= 1,
            '=' if depth == 0 => {
                return Some(name_str[colon + 1..colon + 1 + i].trim().to_string());
            }
            _ => {}
        }
    }
    None
}

fn is_numeric_type(ty: &str) -> bool {
    matches!(ty, "i32" | "i64" | "u32" | "u64" | "f32" | "f64")
}

// =============================================================================
// Unknown Variable Check
// =============================================================================

pub fn check_unknown_var(text: &str) -> Vec<Diagnostic> {
    let known = collect_known_names(text);
    let mut diags = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let t = line.trim();
        if t.is_empty() || t.starts_with("//") || t.starts_with("package ")
            || t.starts_with("use ") || t.starts_with("import ") { continue; }
        for word in extract_identifiers(line) {
            if known.contains(&word) { continue; }
            if word.is_empty() { continue; }
            let first = word.chars().next().unwrap();
            if !first.is_lowercase() && first != '_' { continue; }
            diags.push(mk_diag(idx, 0, line.len(),
                &format!("Unknown variable `{}`.", word),
                DiagnosticSeverity::ERROR));
            break;
        }
    }
    diags
}

fn collect_known_names(text: &str) -> HashSet<String> {
    let mut known: HashSet<String> = [
        "fn","let","mut","return","if","else","for","while","loop",
        "match","break","continue","move","pub","use","unsafe","impl",
        "struct","enum","trait","package","as","where","true","false",
        "void","requires","ensures","invariant","concept","const","extern",
    ].iter().map(|s| s.to_string()).collect();
    for ty in &["i8","i16","i32","i64","u8","u16","u32","u64",
        "f32","f64","bool","char","void","String","StringView",
        "Ptr","Option","Result","Status","Vec",
    ] { known.insert(ty.to_string()); }
    for v in collect_var_defs(text) { known.insert(v.name); }
    for name in extract_fn_names(text) { known.insert(name); }
    for name in extract_struct_names(text) { known.insert(name); }
    known
}

fn extract_fn_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        let search_in = t.strip_prefix("pub ").unwrap_or(t);
        if !search_in.starts_with("fn ") { continue; }
        if let Some(paren) = search_in.find('(') {
            let name = search_in[3..paren].trim();
            if !name.is_empty() { names.push(name.to_string()); }
        }
    }
    names
}

fn extract_struct_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        let search_in = t.strip_prefix("pub ").unwrap_or(t);
        if !search_in.starts_with("struct ") { continue; }
        let after = &search_in[7..];
        let end = after.find(['{', '<', ' ']).unwrap_or(after.len());
        let name = after[..end].trim();
        if !name.is_empty() { names.push(name.to_string()); }
    }
    names
}

fn extract_identifiers(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            words.push(line[start..i].to_string());
        } else { i += 1; }
    }
    words
}

// =============================================================================
// Definition Lookup (for Go-to-Definition of local variables & fields)
// =============================================================================

fn find_definition_in_line(line: &str, word: &str) -> Option<usize> {
    let t = line.trim();
    find_let_binding(t, line, word)
        .or_else(|| find_param_definition(t, line, word))
        .or_else(|| find_field_definition(t, line, word))
}

fn find_let_binding(trimmed: &str, original: &str, word: &str) -> Option<usize> {
    let after = trimmed.strip_prefix("let ")?;
    let after = after.strip_prefix("mut ").unwrap_or(after);
    let end = after.find(|c: char| !c.is_alphanumeric() && c != '_')?;
    if &after[..end] == word { original.find(word) } else { None }
}

fn find_param_definition(trimmed: &str, original: &str, word: &str) -> Option<usize> {
    if !trimmed.starts_with("fn ") { return None; }
    let paren = trimmed.find('(')?;
    let close = trimmed[paren..].find(')').map(|p| paren + p)?;
    for param in trimmed[paren + 1..close].split(',') {
        let p = param.trim();
        let colon = p.find(':')?;
        let pname = p[..colon].trim();
        if pname == word { return original.find(pname); }
    }
    None
}

fn find_field_definition(trimmed: &str, original: &str, word: &str) -> Option<usize> {
    if !trimmed.starts_with("struct ") { return None; }
    let brace = trimmed.find('{')?;
    let close_pos = trimmed[brace + 1..].rfind('}')?;
    for field in trimmed[brace + 1..brace + 1 + close_pos].split(',') {
        let f = field.trim();
        let colon = f.find(':')?;
        let fname = f[..colon].trim();
        if fname == word { return original.find(fname); }
    }
    None
}
