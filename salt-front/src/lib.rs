// Internal Compiler Error Macro
#[macro_export]
macro_rules! ice {
    ($($arg:tt)*) => ({
        eprintln!("INTERNAL COMPILER ERROR: {}", format_args!($($arg)*));
        eprintln!("This is a bug in the Salt compiler. Please report it.");
        panic!("ICE: {}", format_args!($($arg)*));
    })
}

pub mod grammar;
pub mod codegen;
pub mod evaluator;
pub mod cli;
pub mod driver;
pub mod passes;
pub mod keywords;
pub mod fuzz_ast;
pub mod types;
pub mod registry;
pub mod common;
pub mod grammar_tokens;
pub mod hir;



use syn::parse_str;
use crate::grammar::SaltFile;
use crate::codegen::emit_mlir;

/// Preprocess Salt source to make it parseable by syn
pub fn preprocess(source: &str) -> String {
    let result = source
        .lines()
        .map(|line| {
            // Remove Salt-style comments (// only — # comments are no longer supported)
            let line = if let Some(idx) = line.find("//") {
                &line[..idx]
            } else {
                line
            };
            // Replace Salt keywords with syn-friendly ones
            let line = line.replace("!llvm.ptr", "LlvmPtr");
            
            // [FIX] Convert Rust-style use syntax to Salt import syntax:
            // `use std::string::*;` -> `import std.string;` (wildcard = bare module)
            // `use std::string::{A, B};` -> `import std.string.{A, B};`
            // `use std::string::Foo;` -> `import std.string.Foo;`
            let line = if line.trim_start().starts_with("use ") {
                // Convert :: to . and 'use' to 'import'
                let converted = line.replace("use ", "import ")
                    .replace("::", ".");
                // Handle ::* wildcard - remove trailing ".*"
                // "import std.string.*;" -> "import std.string;"
                converted.replace(".*", "")
            } else {
                line.to_string()
            };

            
            // [C++ STYLE GENERICS] Convert HashMap<i64, i64>::new() to HashMap::<i64, i64>::new()
            // so that syn can parse it (syn requires turbofish in expression position)
            let line = convert_generic_call_syntax(&line);
            
            // [SOVEREIGN PHASE 3] Convert tensor shape syntax to parseable form
            // `Tensor<f32, {2, 128, 784}>` -> `Tensor<f32, __Shape_2_128_784__>`
            let line = convert_tensor_shape_syntax(&line);
            
            // [SOVEREIGN FLUENT-MATH] Convert @ operator to .matmul() method
            // Pattern: A @ B becomes A.matmul(B)
            // This is done via simple regex-like replacement for now
            let line = convert_matmul_operator(&line);
            
            // [PHASE 2] Convert |> pipe operator to function application
            // x |> f() becomes f(x)
            let line = convert_pipe_operator(&line);
            
            // [PHASE 2] Convert |?> railway operator to __railway__! macro
            // x |?> f() becomes __railway__!(x, f)
            let line = convert_railway_operator(&line);
            
            // [V4.0 SCORCHED EARTH] Convert prefixed strings to macro calls for syn parsing
            // f"Hello {x}" -> __fstring__!("Hello {x}")
            // hex"DEADBEEF" -> __hex__!("DEADBEEF")
            let line = convert_prefixed_string_syntax(&line);
            
            // [SOVEREIGN] Convert postfix ~ (force-unwrap) to __force_unwrap__! macro
            // val~ -> __force_unwrap__!(val)
            let line = convert_force_unwrap(&line);
            
            // [CROSS-MODULE STRUCT] Convert module.StructName { ... } to module::StructName { ... }
            // so syn parses it as a struct literal, not field access + block.
            let line = convert_module_struct_literal(&line);
            
            line
        })
        .collect::<Vec<_>>()
        .join("\n");
    
    // [P1 HARDENING] Expand @derive annotations into trait impl blocks
    expand_derive_annotations(&result)
}

/// [P1 HARDENING] Expand @derive(Clone, Hash, Eq, Ord) annotations on structs.
/// Scans for `@derive(Trait1, Trait2, ...)` followed by `pub struct Name { fields }`.
/// Generates trait impl blocks and appends them after the struct definition.
fn expand_derive_annotations(source: &str) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let mut result = String::new();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Detect @derive(Trait1, Trait2, ...)
        if trimmed.starts_with("@derive(") && trimmed.ends_with(")") {
            let inner = &trimmed[8..trimmed.len() - 1]; // e.g. "Clone, Hash, Eq, Ord"
            let traits: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();

            // Consume the @derive line
            result.push_str(lines[i]);
            result.push('\n');
            i += 1;

            // Now expect "pub struct Name {" (possibly on one line or multi-line)
            if i >= lines.len() {
                continue;
            }

            // Collect the struct definition lines
            let struct_start = i;
            let mut struct_name = String::new();
            let mut fields: Vec<(String, String)> = Vec::new(); // (name, type)
            let mut brace_depth = 0;
            let mut found_struct = false;
            let mut struct_end = i;

            // Parse struct header
            let header = lines[i].trim();
            if let Some(name_start) = header.find("struct ") {
                let after_struct = &header[name_start + 7..];
                let name_end = after_struct.find(|c: char| !c.is_alphanumeric() && c != '_')
                    .unwrap_or(after_struct.len());
                struct_name = after_struct[..name_end].to_string();
                found_struct = true;
            }

            if !found_struct {
                // Not a struct definition following @derive — skip
                continue;
            }

            // Scan through struct body to find fields and closing brace
            while i < lines.len() {
                let line = lines[i];
                result.push_str(line);
                result.push('\n');

                for ch in line.chars() {
                    if ch == '{' { brace_depth += 1; }
                    if ch == '}' { brace_depth -= 1; }
                }

                // Parse field declarations: "pub field_name: FieldType" or "field_name: FieldType"
                let field_line = line.trim();
                if field_line.contains(':') && !field_line.starts_with("//") && !field_line.contains("struct") {
                    let field_line = if field_line.starts_with("pub ") {
                        &field_line[4..]
                    } else {
                        field_line
                    };
                    if let Some(colon_pos) = field_line.find(':') {
                        let field_name = field_line[..colon_pos].trim().to_string();
                        let mut field_type = field_line[colon_pos + 1..].trim().to_string();
                        // Remove trailing comma
                        if field_type.ends_with(',') {
                            field_type.pop();
                        }
                        // Remove trailing comment
                        if let Some(comment_pos) = field_type.find("//") {
                            field_type = field_type[..comment_pos].trim().to_string();
                        }
                        if !field_name.is_empty() && !field_type.is_empty() && field_name != "_phantom" {
                            fields.push((field_name, field_type));
                        }
                    }
                }

                if brace_depth == 0 && found_struct {
                    struct_end = i;
                    break;
                }
                i += 1;
            }
            i += 1;

            // Generate trait impls
            if !struct_name.is_empty() && !fields.is_empty() {
                for trait_name in &traits {
                    match *trait_name {
                        "Clone" => {
                            result.push_str(&format!("\nimpl Clone for {} {{\n", struct_name));
                            result.push_str(&format!("    fn clone(&self) -> {} {{\n", struct_name));
                            result.push_str(&format!("        return {} {{ ", struct_name));
                            let field_inits: Vec<String> = fields.iter()
                                .map(|(name, _)| format!("{}: self.{}", name, name))
                                .collect();
                            result.push_str(&field_inits.join(", "));
                            result.push_str(" };\n    }\n}\n");
                        }
                        "Eq" => {
                            result.push_str(&format!("\nimpl Eq for {} {{\n", struct_name));
                            result.push_str(&format!("    fn eq(&self, other: &{}) -> bool {{\n", struct_name));
                            let conditions: Vec<String> = fields.iter()
                                .map(|(name, _)| format!("self.{} == other.{}", name, name))
                                .collect();
                            result.push_str(&format!("        return {};\n", conditions.join(" && ")));
                            result.push_str("    }\n}\n");
                        }
                        "Hash" => {
                            result.push_str(&format!("\nimpl Hash for {} {{\n", struct_name));
                            result.push_str("    fn hash(&self) -> u64 {\n");
                            if fields.len() == 1 {
                                result.push_str(&format!("        return (self.{} as u64);\n", fields[0].0));
                            } else {
                                // Combine field hashes with XOR + rotation
                                result.push_str(&format!("        let mut h: u64 = self.{} as u64;\n", fields[0].0));
                                for field in &fields[1..] {
                                    result.push_str(&format!("        h = h ^ ((self.{} as u64) << 16) ^ ((self.{} as u64) >> 48);\n", field.0, field.0));
                                }
                                result.push_str("        return h;\n");
                            }
                            result.push_str("    }\n}\n");
                        }
                        "Ord" => {
                            result.push_str(&format!("\nimpl Ord for {} {{\n", struct_name));
                            result.push_str(&format!("    fn cmp(&self, other: &{}) -> i32 {{\n", struct_name));
                            for (idx, (name, _)) in fields.iter().enumerate() {
                                if idx < fields.len() - 1 {
                                    result.push_str(&format!("        let c{} = self.{}.cmp(&other.{});\n", idx, name, name));
                                    result.push_str(&format!("        if c{} != 0 {{ return c{}; }}\n", idx, idx));
                                } else {
                                    result.push_str(&format!("        return self.{}.cmp(&other.{});\n", name, name));
                                }
                            }
                            result.push_str("    }\n}\n");
                        }
                        _ => {
                            // Unknown trait — skip silently
                        }
                    }
                }
            }
        } else {
            result.push_str(lines[i]);
            result.push('\n');
            i += 1;
        }
    }

    result
}

/// [V4.0 SCORCHED EARTH] Convert prefixed string literals to macro calls
/// `f"Hello {name}"` -> `__fstring__!("Hello {name}")`
/// `hex"DEADBEEF"` -> `__hex__!("DEADBEEF")`
/// [SOVEREIGN WRITER PROTOCOL] Also converts target expressions:
/// `console.f"Hello {x}"` -> `__target_fstring__!(console, "Hello {x}")`
/// This converts the syntax to something syn can parse as a macro invocation,
/// which codegen then intercepts and expands via native_fstring_expand/native_hex_expand.
/// 
/// IMPORTANT: We must track string context to avoid converting prefixes inside
/// existing string literals (e.g., `@string_prefix("f")` should NOT become `@string_prefix("__fstring__!(")")`).
fn convert_prefixed_string_syntax(line: &str) -> String {
    // Quick check: must contain a prefix followed by a quote
    if !line.contains("f\"") && !line.contains("hex\"") {
        return line.to_string();
    }
    
    let mut result = String::new();
    let mut chars = line.chars().peekable();
    let mut in_string = false; // Track if we're inside a string literal
    
    while let Some(c) = chars.next() {
        // Track string context (but not prefixed string we're converting)
        if c == '"' && !in_string {
            // Check if previous char indicates this might be a prefixed string
            let _prev_char = result.chars().last().unwrap_or(' ');
            // If we just wrote "__fstring__!(", "__hex__!(", or "__target_fstring__!(", this is our generated string
            if result.ends_with("__fstring__!(") || result.ends_with("__hex__!(") || result.ends_with(", ") {
                // We're entering our generated string - don't set in_string
                result.push(c);
                continue;
            }
            // Otherwise, we're entering a regular string literal
            in_string = true;
            result.push(c);
            continue;
        }
        
        if c == '"' && in_string {
            // Check for escaped quote
            let escaped = result.ends_with('\\');
            if !escaped {
                in_string = false;
            }
            result.push(c);
            continue;
        }
        
        // Skip all conversions if we're inside a string
        if in_string {
            result.push(c);
            continue;
        }
        
        // [SOVEREIGN WRITER PROTOCOL] Check for '.f"' pattern (target.f"...")
        // This is the target expression syntax for streaming to Writers
        if c == '.' && chars.peek() == Some(&'f') {
            // Peek ahead to verify pattern: .f"
            let mut peek_chars = chars.clone();
            if peek_chars.next() == Some('f') && peek_chars.next() == Some('"') {
                // We have a .f" pattern - extract the target expression from result
                // The target is everything back to the last separator (=, ;, (, {, ,, space after let/mut)
                let target = extract_target_expression(&result);
                
                if !target.is_empty() {
                    // Remove the target from result (we'll re-emit it in the macro)
                    let target_len = target.len();
                    result.truncate(result.len() - target_len);
                    
                    // Consume .f"
                    chars.next(); // f
                    chars.next(); // "
                    
                    // Collect the f-string content
                    let content = collect_string_content(&mut chars);
                    
                    // Emit as __target_fstring__!(target, "content")
                    result.push_str(&format!("__target_fstring__!({}, \"{}\")", target.trim(), content));
                    continue;
                }
            }
        }
        
        // Check for 'f"' prefix (f-string) - only outside strings
        if c == 'f' && chars.peek() == Some(&'"') {
            // Check this isn't part of a longer identifier (e.g., "if\"...")
            let is_standalone = result.is_empty() || !result.chars().last().unwrap().is_alphanumeric();
            if is_standalone {
                // Also check this isn't preceded by a '.' (which would be target.f")
                let is_target_pattern = result.ends_with('.');
                if !is_target_pattern {
                    // Consume the opening quote
                    chars.next();
                    // Collect the string content (this is the f-string content)
                    let content = collect_string_content(&mut chars);
                    result.push_str(&format!("__fstring__!(\"{}\")", content));
                    continue;
                }
            }
        }
        
        // Check for 'hex"' prefix - only outside strings
        if c == 'h' && chars.peek() == Some(&'e') {
            // Peek ahead to check for "hex\""
            let mut peek_chars = chars.clone();
            if peek_chars.next() == Some('e') 
                && peek_chars.next() == Some('x') 
                && peek_chars.next() == Some('"') 
            {
                let is_standalone = result.is_empty() || !result.chars().last().unwrap().is_alphanumeric();
                if is_standalone {
                    // Consume "ex\""
                    chars.next(); // e
                    chars.next(); // x
                    chars.next(); // "
                    let content = collect_string_content(&mut chars);
                    result.push_str(&format!("__hex__!(\"{}\")", content));
                    continue;
                }
            }
        }
        
        result.push(c);
    }
    
    result
}

/// [SOVEREIGN WRITER PROTOCOL] Extract the target expression for target.f"..." syntax
/// Scans backwards from the current position to find the complete expression
fn extract_target_expression(buffer: &str) -> String {
    let mut depth = 0;
    let mut start_pos = 0;  // Start at beginning of buffer by default
    
    // Scan backwards to find the start of the target expression
    for (i, c) in buffer.chars().rev().enumerate() {
        let pos = buffer.len() - i - 1;
        match c {
            ')' | ']' | '>' => depth += 1,
            '(' | '[' | '<' => {
                if depth > 0 {
                    depth -= 1;
                } else {
                    // Hit an unbalanced opener - target starts after this
                    start_pos = pos + 1;
                    break;
                }
            }
            '=' | ';' | ',' | '{' if depth == 0 => {
                // These delimiters mark the boundary before the target
                start_pos = pos + 1;
                break;
            }
            ' ' | '\t' if depth == 0 => {
                // Check if this is meaningful whitespace (after keyword)
                let before = &buffer[..pos];
                if before.ends_with("let") || before.ends_with("mut") || before.ends_with("return") {
                    start_pos = pos + 1;
                    break;
                }
                // Regular whitespace - this is a separator
                // (handles case like "let _ = console" where there's space after =)
                start_pos = pos + 1;
                break;
            }
            _ => {}
        }
    }
    
    // Extract from 'start_pos' to the end of the buffer 
    buffer[start_pos..].trim_start().to_string()
}

/// Collect string content between quotes, handling escapes
fn collect_string_content(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut content = String::new();
    while let Some(c) = chars.next() {
        if c == '"' {
            break; // End of string
        } else if c == '\\' {
            // Escape sequence - keep both backslash and following char
            content.push(c);
            if let Some(escaped) = chars.next() {
                content.push(escaped);
            }
        } else {
            content.push(c);
        }
    }
    content
}

/// [C++ STYLE GENERICS] Convert `Ident<T, U>::method()` to `Ident::<T, U>::method()`
/// This allows C++-style generic instantiation syntax while keeping syn happy.
/// Disambiguation: `>` immediately followed by `::` is always a generic instantiation.
/// Regular comparisons like `if x < y` never have `>::` after them.
fn convert_generic_call_syntax(line: &str) -> String {
    // Quick check: must contain both < and >:: to be relevant
    if !line.contains(">::") {
        return line.to_string();
    }
    
    let mut result = String::new();
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_string = false;
    
    while i < len {
        let c = chars[i];
        
        // Track string literals
        if c == '"' && !in_string {
            in_string = true;
            result.push(c);
            i += 1;
            continue;
        }
        if c == '"' && in_string {
            let escaped = i > 0 && chars[i - 1] == '\\';
            if !escaped {
                in_string = false;
            }
            result.push(c);
            i += 1;
            continue;
        }
        if in_string {
            result.push(c);
            i += 1;
            continue;
        }
        
        // Look for pattern: Ident<...>::  (where Ident does NOT end with ::)
        // We detect `<` that starts generic args by checking:
        //   1. The char before `<` is alphanumeric or _ (end of identifier)
        //   2. The char before that is NOT ':' (not already turbofish ::< )
        //   3. There's a matching `>` followed by `::` somewhere ahead
        if c == '<' {
            let prev_is_ident = i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_');
            let already_turbofish = i >= 2 && chars[i - 1] == ':' && chars[i - 2] == ':';
            
            if prev_is_ident && !already_turbofish {
                // Try to find matching > followed by ::
                let mut depth = 1;
                let mut j = i + 1;
                let mut found_close = false;
                
                while j < len && depth > 0 {
                    match chars[j] {
                        '<' => depth += 1,
                        '>' => {
                            depth -= 1;
                            if depth == 0 {
                                // Check if > is followed by ::
                                if j + 2 < len && chars[j + 1] == ':' && chars[j + 2] == ':' {
                                    found_close = true;
                                }
                            }
                        }
                        '"' => break, // Hit string, bail
                        ';' | '{' | '}' if depth <= 1 => break, // Statement boundary, bail
                        _ => {}
                    }
                    j += 1;
                }
                
                if found_close {
                    // Insert :: before < to convert to turbofish
                    result.push_str("::");
                }
            }
        }
        
        result.push(c);
        i += 1;
    }
    
    result
}

/// [SOVEREIGN PHASE 3] Convert tensor shape syntax to syn-parseable form with AUTO-RANK
/// `Tensor<f32, {128, 784}>` -> `Tensor<f32, __Shape_2_128_784__>` (2 dims = rank 2)
/// `Tensor<f32, {784}>` -> `Tensor<f32, __Shape_1_784__>` (1 dim = rank 1)
/// Only converts `{...}` that appear to be in type position (after comma in generics)
fn convert_tensor_shape_syntax(line: &str) -> String {
    // Quick check: must have both '{' and 'Tensor' for shaped tensor syntax
    if !line.contains('{') || !line.contains("Tensor") {
        return line.to_string();
    }
    
    let mut result = String::new();
    let mut chars = line.chars().peekable();
    let mut in_generic = 0; // Track angle bracket depth
    let mut last_was_comma_or_angle = false;
    
    while let Some(c) = chars.next() {
        if c == '<' {
            in_generic += 1;
            last_was_comma_or_angle = true;
            result.push(c);
        } else if c == '>' && in_generic > 0 {
            in_generic -= 1;
            last_was_comma_or_angle = false;
            result.push(c);
        } else if c == ',' && in_generic > 0 {
            last_was_comma_or_angle = true;
            result.push(c);
        } else if c == ' ' && last_was_comma_or_angle {
            // Keep whitespace, maintain state
            result.push(c);
        } else if c == '{' && in_generic > 0 && last_was_comma_or_angle {
            // This looks like a shape in generic position: `<T, {2, 3, 4}>`
            let mut contents = String::new();
            let mut depth = 1;
            while let Some(inner) = chars.next() {
                if inner == '{' {
                    depth += 1;
                    contents.push(inner);
                } else if inner == '}' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    contents.push(inner);
                } else {
                    contents.push(inner);
                }
            }
            
            // Check if contents are just numbers/commas/spaces/identifiers
            // For auto-rank, we accept even single values (no comma required)
            let is_shape = contents.chars().all(|ch| 
                ch.is_ascii_digit() || ch == ',' || ch == ' ' || ch.is_ascii_uppercase() || ch == '_' || ch == '?'
            ) && !contents.trim().is_empty();
            
            if is_shape {
                // [AUTO-RANK] Convert to __Shape_Rank_D1_D2...__ format
                // The rank is automatically computed from the number of dimensions
                let parts: Vec<&str> = contents.split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect();
                let rank = parts.len();
                let dims_str = parts.join("_");
                // Format: __Shape_{rank}_{dims}__
                result.push_str(&format!("__Shape_{}_{}__", rank, dims_str));
            } else {
                // Not a shape, output as block
                result.push('{');
                result.push_str(&contents);
                result.push('}');
            }
            last_was_comma_or_angle = false;
        } else {
            result.push(c);
            last_was_comma_or_angle = false;
        }
    }
    
    result
}

/// Convert `A @ B` matmul syntax to `A.matmul(B)` method call
fn convert_matmul_operator(line: &str) -> String {
    // Find @ that is a binary operator (space-separated from operands)
    // Simple heuristic: look for pattern " @ " and convert
    if !line.contains(" @ ") {
        return line.to_string();
    }
    
    let mut result = String::new();
    let mut chars = line.chars().peekable();
    let mut in_string = false;
    let mut buffer = String::new();
    
    while let Some(c) = chars.next() {
        // Track string literals
        if c == '"' && !in_string {
            in_string = true;
            buffer.push(c);
            continue;
        } else if c == '"' && in_string {
            in_string = false;
            buffer.push(c);
            continue;
        }
        
        if in_string {
            buffer.push(c);
            continue;
        }
        
        // Check for " @ " pattern
        if c == ' ' && chars.peek() == Some(&'@') {
            let at = chars.next().unwrap(); // consume @
            if chars.peek() == Some(&' ') {
                chars.next(); // consume trailing space
                
                // Found " @ " - need to extract LHS and wrap
                // LHS is everything after the last assignment or statement start
                let lhs = extract_matmul_lhs(&buffer);
                let prefix = &buffer[..buffer.len() - lhs.len()];
                
                // Collect RHS until we hit a non-expression character
                let rhs = extract_matmul_rhs(&mut chars);
                
                result.push_str(prefix);
                result.push_str(&format!("{}.matmul({})", lhs.trim(), rhs.trim()));
                buffer.clear();
                continue;
            } else {
                buffer.push(c);
                buffer.push(at);
                continue;
            }
        }
        
        buffer.push(c);
    }
    
    result.push_str(&buffer);
    result
}

/// Extract LHS operand for matmul (simple: goes back to last = or ( or start)
fn extract_matmul_lhs(buffer: &str) -> String {
    let mut depth = 0;
    let mut end = buffer.len();
    
    for (i, c) in buffer.chars().rev().enumerate() {
        match c {
            ')' | ']' => depth += 1,
            '(' | '[' => {
                if depth > 0 {
                    depth -= 1;
                } else {
                    end = buffer.len() - i;
                    break;
                }
            }
            '=' | ';' | ',' if depth == 0 => {
                end = buffer.len() - i;
                break;
            }
            _ => {}
        }
    }
    
    buffer[end..].to_string()
}

/// Extract RHS operand for matmul (until ; or ) or , or end)
fn extract_matmul_rhs(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut result = String::new();
    let mut depth = 0;
    
    while let Some(&c) = chars.peek() {
        match c {
            '(' | '[' => {
                depth += 1;
                result.push(chars.next().unwrap());
            }
            ')' | ']' => {
                if depth > 0 {
                    depth -= 1;
                    result.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            ';' | ',' if depth == 0 => break,
            _ => {
                result.push(chars.next().unwrap());
            }
        }
    }
    
    result
}

/// [PHASE 2] Convert `x |> f()` pipe syntax to `f(x)` function application
/// Chains: `x |> f() |> g()` → `g(f(x))`
/// With args: `x |> f(y)` → `f(x, y)` (prepends LHS as first argument)
/// Bare fn: `x |> f` → `f(x)`
fn convert_pipe_operator(line: &str) -> String {
    if !line.contains("|>") {
        return line.to_string();
    }
    
    let mut result = String::new();
    let mut chars = line.chars().peekable();
    let mut in_string = false;
    let mut buffer = String::new();
    
    while let Some(c) = chars.next() {
        // Track string literals
        if c == '"' && !in_string {
            in_string = true;
            buffer.push(c);
            continue;
        } else if c == '"' && in_string {
            let escaped = buffer.ends_with('\\');
            if !escaped {
                in_string = false;
            }
            buffer.push(c);
            continue;
        }
        
        if in_string {
            buffer.push(c);
            continue;
        }
        
        // Check for |> pattern (with optional spaces)
        if c == '|' && chars.peek() == Some(&'>') {
            chars.next(); // consume >
            
            // Skip whitespace after |>
            while chars.peek() == Some(&' ') {
                chars.next();
            }
            
            // LHS is the trimmed buffer content after the last = or statement boundary
            let lhs = extract_pipe_lhs(&buffer);
            let prefix = &buffer[..buffer.len() - lhs.len()];
            
            // Collect RHS: function name and optional (args)
            let (fn_name, fn_args) = extract_pipe_rhs(&mut chars);
            
            // Build the transformed call
            let lhs_trimmed = lhs.trim();
            let transformed = if fn_args.is_empty() {
                format!("{}({})", fn_name, lhs_trimmed)
            } else {
                format!("{}({}, {})", fn_name, lhs_trimmed, fn_args)
            };
            
            // Push prefix to result, put transformed into buffer
            // so it can be the LHS for the next |> in a chain
            result.push_str(prefix);
            buffer.clear();
            buffer.push_str(&transformed);
            continue;
        }
        
        buffer.push(c);
    }
    
    result.push_str(&buffer);
    result
}

/// Extract LHS operand for pipe (goes back to last = or statement boundary)
fn extract_pipe_lhs(buffer: &str) -> String {
    let mut depth = 0;
    
    for (i, c) in buffer.chars().rev().enumerate() {
        let pos = buffer.len() - i - 1;
        match c {
            ')' | ']' => depth += 1,
            '(' | '[' => {
                if depth > 0 {
                    depth -= 1;
                } else {
                    return buffer[pos + 1..].to_string();
                }
            }
            '=' | ';' | ',' | '{' if depth == 0 => {
                return buffer[pos + 1..].to_string();
            }
            _ => {}
        }
    }
    
    buffer.to_string()
}

/// Extract RHS for pipe: function name and optional args inside parens
/// Returns (fn_name, args_inside_parens) where args is empty string if no parens
fn extract_pipe_rhs(chars: &mut std::iter::Peekable<std::str::Chars>) -> (String, String) {
    let mut fn_name = String::new();
    let mut args = String::new();
    
    // Collect function name (until ( or ; or space or end)
    while let Some(&c) = chars.peek() {
        if c == '(' {
            chars.next(); // consume (
            // Collect args until matching )
            let mut depth = 1;
            while let Some(ac) = chars.next() {
                if ac == '(' {
                    depth += 1;
                    args.push(ac);
                } else if ac == ')' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    args.push(ac);
                } else {
                    args.push(ac);
                }
            }
            break;
        } else if c == ';' || c == ',' || c == ' ' || c == '|' {
            break;
        } else {
            fn_name.push(chars.next().unwrap());
        }
    }
    
    (fn_name.trim().to_string(), args.trim().to_string())
}

/// [PHASE 2] Convert `x |?> f()` railway syntax to `__railway__!(x, f)` macro
/// Chains: `x |?> f() |?> g()` → `__railway__!(__railway__!(x, f), g)`
/// With args: `x |?> f(y)` → `__railway__!(x, f, y)`
fn convert_railway_operator(line: &str) -> String {
    if !line.contains("|?>") {
        return line.to_string();
    }
    
    let mut result = String::new();
    let mut chars = line.chars().peekable();
    let mut in_string = false;
    let mut buffer = String::new();
    
    while let Some(c) = chars.next() {
        // Track string literals
        if c == '"' && !in_string {
            in_string = true;
            buffer.push(c);
            continue;
        } else if c == '"' && in_string {
            let escaped = buffer.ends_with('\\');
            if !escaped {
                in_string = false;
            }
            buffer.push(c);
            continue;
        }
        
        if in_string {
            buffer.push(c);
            continue;
        }
        
        // Check for |?> pattern
        if c == '|' && chars.peek() == Some(&'?') {
            let mut peek = chars.clone();
            peek.next(); // consume ?
            if peek.peek() == Some(&'>') {
                chars.next(); // consume ?
                chars.next(); // consume >
                
                // Skip whitespace after |?>
                while chars.peek() == Some(&' ') {
                    chars.next();
                }
                
                // LHS is the trimmed buffer after last boundary
                let lhs = extract_pipe_lhs(&buffer);
                let prefix = &buffer[..buffer.len() - lhs.len()];
                let lhs_trimmed = lhs.trim();
                
                // Collect RHS: function name and optional (args)
                let (fn_name, fn_args) = extract_pipe_rhs(&mut chars);
                
                // Build __railway__!(lhs, fn_name[, fn_args])
                let transformed = if fn_args.is_empty() {
                    format!("__railway__!({}, {})", lhs_trimmed, fn_name)
                } else {
                    format!("__railway__!({}, {}, {})", lhs_trimmed, fn_name, fn_args)
                };
                
                // Push prefix to result, put transformed into buffer
                // so it can be the LHS for the next |?> in a chain
                result.push_str(prefix);
                buffer.clear();
                buffer.push_str(&transformed);
                continue;
            }
        }
        
        buffer.push(c);
    }
    
    result.push_str(&buffer);
    result
}

/// [SOVEREIGN] Convert postfix force-unwrap operator.
/// `expr~` -> `__force_unwrap__!(expr)` when ~ follows an expression-ending char.
/// Prefix `~x` (bitwise NOT) is preserved unchanged.
fn convert_force_unwrap(line: &str) -> String {
    if !line.contains('~') {
        return line.to_string();
    }
    
    let mut result = String::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let mut in_string = false;
    
    while i < chars.len() {
        let c = chars[i];
        
        // Track string literals
        if c == '"' && !in_string {
            in_string = true;
            result.push(c);
            i += 1;
            continue;
        }
        if c == '"' && in_string {
            let escaped = i > 0 && chars[i - 1] == '\\';
            if !escaped {
                in_string = false;
            }
            result.push(c);
            i += 1;
            continue;
        }
        if in_string {
            result.push(c);
            i += 1;
            continue;
        }
        
        // Check for postfix ~
        if c == '~' {
            // Postfix if the previous character is expression-ending:
            // alphanumeric, _, ), ], }
            let is_postfix = if let Some(prev) = result.chars().last() {
                prev.is_alphanumeric() || prev == '_' || prev == ')' || prev == ']' || prev == '}'
            } else {
                false
            };
            
            if is_postfix {
                // Backtrack to find the expression start
                // Walk backwards through result to find the expression boundary
                let expr = extract_force_unwrap_expr(&result);
                let prefix_len = result.len() - expr.len();
                let prefix = result[..prefix_len].to_string();
                result = prefix;
                result.push_str(&format!("__force_unwrap__!({})", expr.trim()));
                i += 1;
                continue;
            }
        }
        
        result.push(c);
        i += 1;
    }
    
    result
}

/// Extract the expression preceding a postfix ~ operator.
/// Walks backwards from the end of `s` to find the expression boundary.
/// Handles balanced parentheses, method chains (a.b().c~), and simple identifiers.
fn extract_force_unwrap_expr(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut end = chars.len();
    let mut depth_paren = 0i32;
    let mut depth_angle = 0i32;
    
    // Walk backwards
    let mut pos = end;
    while pos > 0 {
        pos -= 1;
        let c = chars[pos];
        
        match c {
            ')' => depth_paren += 1,
            '(' => {
                if depth_paren > 0 {
                    depth_paren -= 1;
                } else {
                    // Unbalanced — this is our boundary
                    return s[pos + 1..end].to_string();
                }
            }
            '>' => depth_angle += 1,
            '<' => {
                if depth_angle > 0 {
                    depth_angle -= 1;
                } else {
                    return s[pos + 1..end].to_string();
                }
            }
            ']' | '}' => depth_paren += 1,
            '[' | '{' => {
                if depth_paren > 0 {
                    depth_paren -= 1;
                } else {
                    return s[pos + 1..end].to_string();
                }
            }
            // Inside balanced parens/angles, keep going
            _ if depth_paren > 0 || depth_angle > 0 => continue,
            // Identifier or method chain characters
            c if c.is_alphanumeric() || c == '_' || c == '.' || c == ':' => continue,
            // Boundary characters (space, =, ;, ,, etc.)
            _ => {
                return s[pos + 1..end].to_string();
            }
        }
    }
    
    // Reached the start of the string
    s[..end].to_string()
}

// [V4.0 SCORCHED EARTH] Legacy f-string preprocessing code deleted
// F-strings are now handled by codegen/context.rs::native_fstring_expand
// with full TraitRegistry context for signature-aware format spec dispatch.


pub fn compile_ast(file: &mut SaltFile, release_mode: bool, registry: Option<&crate::registry::Registry>, skip_scan: bool, vverify: bool, disable_alias_scopes: bool, no_verify: bool, lib_mode: bool, sip_mode: bool, debug_info: bool, source_file: &str) -> anyhow::Result<String> {
    // Run Comptime Evaluation Pass
    passes::comptime::run(file)
        .map_err(|e| anyhow::anyhow!("Comptime Error: {:?}", e))?;

    let mut mlir = emit_mlir(file, release_mode, registry, skip_scan, no_verify, disable_alias_scopes, lib_mode, sip_mode, debug_info, source_file).map_err(|e| anyhow::anyhow!(e))?;
    
    // Prepend Alias Scope Definitions (MLIR Attribute Aliases)
    // V7.3: Added per-argument scopes (scope_arg_0 through scope_arg_9) for fine-grained noalias
    // Guarded by disable_alias_scopes flag — when disabled, MLIR is compatible with standard mlir-opt
    if !disable_alias_scopes {
        let alias_defs = "
#salt_domain = #llvm.alias_scope_domain<id = distinct[0]<>, description = \"salt_mem\">
#scope_local = #llvm.alias_scope<id = distinct[1]<>, domain = #salt_domain, description = \"local\">
#scope_global = #llvm.alias_scope<id = distinct[2]<>, domain = #salt_domain, description = \"global\">
#scope_arg_0 = #llvm.alias_scope<id = distinct[10]<>, domain = #salt_domain, description = \"arg0\">
#scope_arg_1 = #llvm.alias_scope<id = distinct[11]<>, domain = #salt_domain, description = \"arg1\">
#scope_arg_2 = #llvm.alias_scope<id = distinct[12]<>, domain = #salt_domain, description = \"arg2\">
#scope_arg_3 = #llvm.alias_scope<id = distinct[13]<>, domain = #salt_domain, description = \"arg3\">
#scope_arg_4 = #llvm.alias_scope<id = distinct[14]<>, domain = #salt_domain, description = \"arg4\">
#scope_arg_5 = #llvm.alias_scope<id = distinct[15]<>, domain = #salt_domain, description = \"arg5\">
#scope_arg_6 = #llvm.alias_scope<id = distinct[16]<>, domain = #salt_domain, description = \"arg6\">
#scope_arg_7 = #llvm.alias_scope<id = distinct[17]<>, domain = #salt_domain, description = \"arg7\">
#scope_arg_8 = #llvm.alias_scope<id = distinct[18]<>, domain = #salt_domain, description = \"arg8\">
#scope_arg_9 = #llvm.alias_scope<id = distinct[19]<>, domain = #salt_domain, description = \"arg9\">
";
        mlir.insert_str(0, alias_defs);
    }
    
    Ok(mlir)
}

pub fn compile(source: &str, release_mode: bool, registry: Option<&crate::registry::Registry>, skip_scan: bool, vverify: bool) -> anyhow::Result<String> {
    // Reject `import` keyword — Salt uses `use` exclusively
    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("import ") {
            anyhow::bail!(
                "Line {}: `import` is not valid Salt syntax. Use `use` instead:\n  {}\n  → {}",
                i + 1, trimmed, trimmed.replacen("import", "use", 1)
            );
        }
    }
    let processed = preprocess(source);
    let mut file: SaltFile = parse_str(&processed)?;
    compile_ast(&mut file, release_mode, registry, skip_scan, vverify, false, false, false, false, false, "<stdin>")
}

/// [CROSS-MODULE STRUCT] Convert `module.StructName { ... }` to `module::StructName { ... }`
/// so that syn parses it as a struct literal construction, not a field access + block.
///
/// Detection heuristic: `ident.UpperCaseIdent` followed by ` {` or `{`.
/// The uppercase check ensures we don't convert field accesses like `p.val`
/// or method calls like `addr.make_phys()`.
fn convert_module_struct_literal(line: &str) -> String {
    // Quick check: must contain a `.` to be relevant
    if !line.contains('.') {
        return line.to_string();
    }

    let mut result = String::new();
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_string = false;

    while i < len {
        let c = chars[i];

        // Track string context
        if c == '"' && !in_string {
            in_string = true;
            result.push(c);
            i += 1;
            continue;
        }
        if c == '"' && in_string {
            let escaped = i > 0 && chars[i - 1] == '\\';
            if !escaped {
                in_string = false;
            }
            result.push(c);
            i += 1;
            continue;
        }
        if in_string {
            result.push(c);
            i += 1;
            continue;
        }

        // Look for pattern: ident.UpperIdent followed by `{` or ` {`
        if c == '.' && i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
            // Check if the char after `.` is uppercase (struct name convention)
            if i + 1 < len && chars[i + 1].is_ascii_uppercase() {
                // Scan ahead to find the end of the identifier after `.`
                let mut j = i + 1;
                while j < len && (chars[j].is_alphanumeric() || chars[j] == '_') {
                    j += 1;
                }
                // Check if followed by `{` or ` {` (struct literal)
                let mut k = j;
                while k < len && chars[k] == ' ' {
                    k += 1;
                }
                if k < len && chars[k] == '{' {
                    // Pattern confirmed: module.StructName { → module::StructName {
                    result.push_str("::");
                    i += 1; // skip the `.`
                    continue;
                }
            }
        }

        result.push(c);
        i += 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matmul_operator_simple() {
        let input = "let c = a @ b;";
        let output = convert_matmul_operator(input);
        // Note: slight spacing variation is acceptable as syn normalizes whitespace
        assert!(output.contains("a.matmul(b)"));
    }

    #[test]
    fn test_matmul_operator_with_parens() {
        let input = "let result = (weights @ input);";
        let output = convert_matmul_operator(input);
        assert!(output.contains("weights.matmul(input)"));
    }

    #[test]
    fn test_matmul_no_operator() {
        let input = "let x = a + b;";
        let output = convert_matmul_operator(input);
        assert_eq!(output, "let x = a + b;");
    }

    #[test]
    fn test_matmul_in_string_literal() {
        let input = r#"let s = "a @ b";"#;
        let output = convert_matmul_operator(input);
        // Should NOT convert @ inside strings
        assert_eq!(output, r#"let s = "a @ b";"#);
    }

    #[test]
    fn test_matmul_chained() {
        let input = "let c = a @ b;";
        let output = convert_matmul_operator(input);
        assert!(output.contains(".matmul("));
    }

    // ============================================================
    // TENSOR SHAPE SYNTAX TESTS (AUTO-RANK)
    // ============================================================

    #[test]
    fn test_tensor_shape_rank1_single_dim() {
        // {784} → __Shape_1_784__ (auto-rank = 1)
        let input = "let x: Tensor<f32, {784}> = alloc_tensor();";
        let output = convert_tensor_shape_syntax(input);
        assert!(output.contains("__Shape_1_784__"), "Expected __Shape_1_784__, got: {}", output);
    }

    #[test]
    fn test_tensor_shape_rank2_matrix() {
        // {128, 784} → __Shape_2_128_784__ (auto-rank = 2)
        let input = "let w: Tensor<f32, {128, 784}> = alloc_tensor();";
        let output = convert_tensor_shape_syntax(input);
        assert!(output.contains("__Shape_2_128_784__"), "Expected __Shape_2_128_784__, got: {}", output);
    }

    #[test]
    fn test_tensor_shape_rank3_volume() {
        // {3, 4, 5} → __Shape_3_3_4_5__ (auto-rank = 3)
        let input = "let t: Tensor<f32, {3, 4, 5}> = alloc_tensor();";
        let output = convert_tensor_shape_syntax(input);
        assert!(output.contains("__Shape_3_3_4_5__"), "Expected __Shape_3_3_4_5__, got: {}", output);
    }

    #[test]
    fn test_tensor_shape_with_spaces() {
        // {128,  784} with extra spaces → still works
        let input = "let w: Tensor<f32, {128,  784}> = alloc_tensor();";
        let output = convert_tensor_shape_syntax(input);
        assert!(output.contains("__Shape_2_128_784__"), "Expected __Shape_2_128_784__, got: {}", output);
    }

    #[test]
    fn test_tensor_shape_symbolic_dims() {
        // {HIDDEN, INPUT} with symbolic constants
        let input = "let w: Tensor<f32, {HIDDEN, INPUT}> = alloc_tensor();";
        let output = convert_tensor_shape_syntax(input);
        assert!(output.contains("__Shape_2_HIDDEN_INPUT__"), "Expected __Shape_2_HIDDEN_INPUT__, got: {}", output);
    }

    #[test]
    fn test_tensor_shape_ptr_wrapped() {
        // Ptr<Tensor<f32, {4, 3}>> → properly nested
        let input = "let w: Ptr<Tensor<f32, {4, 3}>> = alloc_tensor();";
        let output = convert_tensor_shape_syntax(input);
        assert!(output.contains("__Shape_2_4_3__"), "Expected __Shape_2_4_3__, got: {}", output);
    }

    #[test]
    fn test_tensor_no_false_positive_block() {
        // Regular block expression should NOT be converted
        let input = "let x = { let y = 5; y + 1 };";
        let output = convert_tensor_shape_syntax(input);
        assert_eq!(output, input, "Block expression should not be converted");
    }

    #[test]
    fn test_tensor_no_conversion_without_tensor() {
        // No Tensor keyword = no conversion
        let input = "let x: MyType<i32, {1, 2}> = foo();";
        let output = convert_tensor_shape_syntax(input);
        assert_eq!(output, input, "Should not convert without Tensor keyword");
    }

    #[test]
    fn test_tensor_preserves_non_type_braces() {
        // Block after statement should be preserved
        let input = "fn main() { let x = 5; }";
        let output = convert_tensor_shape_syntax(input);
        assert_eq!(output, input, "Function body should be preserved");
    }

    #[test]
    fn test_tensor_multiple_on_line() {
        // Multiple tensor types on same line
        let input = "let (a, b): (Tensor<f32, {4}>, Tensor<f32, {3, 2}>) = (x, y);";
        let output = convert_tensor_shape_syntax(input);
        assert!(output.contains("__Shape_1_4__"), "First tensor should convert");
        assert!(output.contains("__Shape_2_3_2__"), "Second tensor should convert");
    }

    #[test]
    fn test_tensor_integration_with_matmul() {
        // Full pipeline: tensor syntax + @ operator
        let input = "let result = w: Tensor<f32, {4, 3}> @ x: Tensor<f32, {3}>;";
        let step1 = convert_tensor_shape_syntax(input);
        let step2 = convert_matmul_operator(&step1);
        assert!(step1.contains("__Shape_2_4_3__"), "Tensor shape should convert");
        assert!(step1.contains("__Shape_1_3__"), "Vector shape should convert");
        assert!(step2.contains(".matmul("), "@ should become .matmul()");
    }

    // [V4.0 SCORCHED EARTH] F-string preprocessing tests removed
    // F-strings are now handled by codegen/context.rs::native_fstring_expand
    // Tests for f-string expansion are now in codegen context tests
    
    // ============================================================
    // SOVEREIGN WRITER PROTOCOL TESTS (target.f"..." syntax)
    // ============================================================

    #[test]
    fn test_target_fstring_simple() {
        // console.f"Hello" → __target_fstring__!(console, "Hello")
        let input = r#"console.f"Hello";"#;
        let output = convert_prefixed_string_syntax(input);
        assert!(output.contains("__target_fstring__!(console, \"Hello\")"), 
            "Expected target_fstring macro, got: {}", output);
    }

    #[test]
    fn test_target_fstring_with_interpolation() {
        // buf.f"Value: {x}" → __target_fstring__!(buf, "Value: {x}")
        let input = r#"buf.f"Value: {x}";"#;
        let output = convert_prefixed_string_syntax(input);
        assert!(output.contains("__target_fstring__!(buf, \"Value: {x}\")"), 
            "Expected target_fstring with interpolation, got: {}", output);
    }

    #[test]
    fn test_target_fstring_after_assignment() {
        // let _ = console.f"test" → let _ = __target_fstring__!(console, "test")
        let input = r#"let _ = console.f"test";"#;
        let output = convert_prefixed_string_syntax(input);
        assert!(output.contains("__target_fstring__!(console, \"test\")"), 
            "Expected target_fstring after assignment, got: {}", output);
    }

    #[test]
    fn test_target_fstring_method_chain() {
        // get_writer().f"output" → __target_fstring__!(get_writer(), "output")
        let input = r#"get_writer().f"output";"#;
        let output = convert_prefixed_string_syntax(input);
        assert!(output.contains("__target_fstring__!(get_writer(), \"output\")"), 
            "Expected target_fstring with method chain, got: {}", output);
    }

    #[test]
    fn test_regular_fstring_still_works() {
        // Regular f-strings should still work
        let input = r#"let s = f"Hello {name}";"#;
        let output = convert_prefixed_string_syntax(input);
        assert!(output.contains("__fstring__!(\"Hello {name}\")"), 
            "Regular f-string should still work, got: {}", output);
        assert!(!output.contains("__target_fstring__"), 
            "Should not produce target_fstring for regular f-string");
    }

    #[test]
    fn test_target_fstring_format_spec() {
        // writer.f"Pi: {pi:.2f}" → __target_fstring__!(writer, "Pi: {pi:.2f}")
        let input = r#"writer.f"Pi: {pi:.2f}";"#;
        let output = convert_prefixed_string_syntax(input);
        assert!(output.contains("__target_fstring__!(writer, \"Pi: {pi:.2f}\")"), 
            "Expected target_fstring with format spec, got: {}", output);
    }

    // ============================================================
    // PIPELINE OPERATOR TESTS (|> syntax)
    // Parameterized over input/expected output pairs
    // ============================================================

    macro_rules! pipe_test {
        ($name:ident, $input:expr, $expected:expr) => {
            #[test]
            fn $name() {
                let output = convert_pipe_operator($input);
                assert!(output.contains($expected),
                    "Input: {:?}\nExpected to contain: {:?}\nGot: {:?}",
                    $input, $expected, output);
            }
        };
    }

    // Simple: x |> f() → f(x)
    pipe_test!(test_pipe_simple, "let y = x |> f();", "f(x)");
    // Chain: x |> f() |> g() → g(f(x))
    pipe_test!(test_pipe_chain, "let y = x |> f() |> g();", "g(f(x))");
    // With extra args: x |> f(y) → f(x, y)
    pipe_test!(test_pipe_with_args, "let y = x |> f(y);", "f(x, y)");
    // Bare function (no parens): x |> f → f(x)
    pipe_test!(test_pipe_bare_fn, "let y = x |> f;", "f(x)");
    // Method call: x |> self.process() → self.process(x)
    pipe_test!(test_pipe_method, "let y = x |> self.process();", "self.process(x)");
    // Triple chain: x |> f() |> g() |> h() → h(g(f(x)))
    pipe_test!(test_pipe_triple, "let y = x |> f() |> g() |> h();", "h(g(f(x)))");
    // Function call as LHS
    pipe_test!(test_pipe_expr_lhs, "let y = get_value() |> process();", "process(get_value())");

    #[test]
    fn test_pipe_in_string_literal() {
        let input = r#"let s = "x |> f";"#;
        let output = convert_pipe_operator(input);
        assert!(output.contains("|>"), "Should not convert |> inside string literals, got: {}", output);
    }

    #[test]
    fn test_pipe_no_pipe() {
        let input = "let x = a + b;";
        let output = convert_pipe_operator(input);
        assert_eq!(output, input);
    }

    // ============================================================
    // RAILWAY OPERATOR TESTS (|?> syntax)
    // ============================================================

    macro_rules! railway_test {
        ($name:ident, $input:expr, $expected:expr) => {
            #[test]
            fn $name() {
                let output = convert_railway_operator($input);
                assert!(output.contains($expected),
                    "Input: {:?}\nExpected to contain: {:?}\nGot: {:?}",
                    $input, $expected, output);
            }
        };
    }

    // Simple: x |?> f() → __railway__!(x, f)
    railway_test!(test_railway_simple, "let y = x |?> f();", "__railway__!(x, f)");
    // Chain: x |?> f() |?> g() → __railway__!(__railway__!(x, f), g)
    railway_test!(test_railway_chain, "let y = x |?> f() |?> g();",
        "__railway__!(__railway__!(x, f), g)");
    // With args: x |?> f(y) → __railway__!(x, f, y)
    railway_test!(test_railway_with_args, "let y = x |?> f(y);", "__railway__!(x, f, y)");

    #[test]
    fn test_railway_in_string_literal() {
        let input = r#"let s = "x |?> f";"#;
        let output = convert_railway_operator(input);
        assert!(output.contains("|?>"), "Should not convert |?> inside string literals, got: {}", output);
    }

    #[test]
    fn test_railway_no_railway() {
        let input = "let x = a + b;";
        let output = convert_railway_operator(input);
        assert_eq!(output, input);
    }

    // ============================================================
    // FULL PREPROCESS() EMISSION TESTS
    // Verify all operators (|>, |?>, ^, ~) survive/transform
    // correctly through the complete preprocessing pipeline
    // ============================================================

    macro_rules! preprocess_test {
        ($name:ident, $input:expr, $expected:expr) => {
            #[test]
            fn $name() {
                let output = preprocess($input);
                assert!(output.contains($expected),
                    "Input: {:?}\nExpected output to contain: {:?}\nGot: {:?}",
                    $input, $expected, output);
            }
        };
    }

    // |> pipe: transforms x |> f() into f(x) through full pipeline
    preprocess_test!(test_preprocess_pipe_simple,
        "let y = x |> f();", "f(x)");
    preprocess_test!(test_preprocess_pipe_chain,
        "let y = x |> f() |> g();", "g(f(x))");

    // |?> railway: transforms x |?> f() into __railway__!(x, f)
    preprocess_test!(test_preprocess_railway_simple,
        "let y = x |?> f();", "__railway__!(x, f)");
    preprocess_test!(test_preprocess_railway_chain,
        "let y = x |?> f() |?> g();", "__railway__!(__railway__!(x, f), g)");

    // ^ XOR: must survive preprocessing unchanged (syn parses it natively)
    preprocess_test!(test_preprocess_xor_preserved,
        "let y = a ^ b;", "a ^ b");

    // ~ NOT: must survive preprocessing unchanged (syn parses it natively)
    preprocess_test!(test_preprocess_bitwise_not_preserved,
        "let y = ~x;", "~x");

    // Combined: ^ and |> in same line
    preprocess_test!(test_preprocess_xor_with_pipe,
        "let y = a ^ b |> f();", "f(a ^ b)");

    // Ensure | (bitwise OR) is NOT mangled by |> removal
    #[test]
    fn test_preprocess_bitwise_or_preserved() {
        let output = preprocess("let y = a | b;");
        assert!(output.contains("a | b"), "Bitwise OR should be preserved, got: {}", output);
    }

    // Ensure |> doesn't partially match | or || 
    #[test]
    fn test_preprocess_logical_or_preserved() {
        let output = preprocess("let y = a || b;");
        assert!(output.contains("a || b"), "Logical OR should be preserved, got: {}", output);
    }

    // ============================================================
    // FORCE UNWRAP (~) PREPROCESSOR TESTS
    // ============================================================

    #[test]
    fn test_force_unwrap_simple() {
        let output = convert_force_unwrap("let x = val~;");
        assert!(output.contains("__force_unwrap__!(val)"), 
            "Postfix ~ should become __force_unwrap__!, got: {}", output);
    }

    #[test]
    fn test_force_unwrap_method_chain() {
        let output = convert_force_unwrap("let x = foo.bar()~;");
        assert!(output.contains("__force_unwrap__!(foo.bar())"), 
            "Method chain ~ should become __force_unwrap__!, got: {}", output);
    }

    #[test]
    fn test_force_unwrap_fn_call() {
        let output = convert_force_unwrap("let x = get_value(42)~;");
        assert!(output.contains("__force_unwrap__!(get_value(42))"), 
            "Function call ~ should become __force_unwrap__!, got: {}", output);
    }

    #[test]
    fn test_force_unwrap_prefix_tilde_preserved() {
        // Prefix ~x (bitwise NOT) must NOT be converted
        let output = convert_force_unwrap("let y = ~x;");
        assert!(output.contains("~x"), 
            "Prefix ~ should be preserved as bitwise NOT, got: {}", output);
        assert!(!output.contains("__force_unwrap__"),
            "Prefix ~ should NOT become force_unwrap, got: {}", output);
    }

    #[test]
    fn test_force_unwrap_in_assignment() {
        let output = convert_force_unwrap("let x = result~;");
        assert!(output.contains("__force_unwrap__!(result)"),
            "Expected force_unwrap in assignment, got: {}", output);
    }

    #[test]
    fn test_force_unwrap_string_context() {
        // ~ inside string should not be converted
        let output = convert_force_unwrap("let s = \"hello~world\";");
        assert!(!output.contains("__force_unwrap__"),
            "~ inside string should NOT be converted, got: {}", output);
    }

    // ============================================================
    // CROSS-MODULE STRUCT LITERAL PREPROCESSOR TESTS
    // ============================================================
    // The preprocessor must convert `module.StructName { ... }` to
    // `module::StructName { ... }` so syn parses it as a struct literal,
    // not field access + block.

    #[test]
    fn test_module_struct_literal_basic() {
        let output = convert_module_struct_literal("let p = addr.PhysAddr { val: 0x1000 };");
        assert!(output.contains("addr::PhysAddr { val: 0x1000 }"),
            "module.Struct {{ }} should become module::Struct {{ }}, got: {}", output);
    }

    #[test]
    fn test_module_struct_literal_multifield() {
        let output = convert_module_struct_literal("let v = memory.VirtAddr { val: x, tag: 0 };");
        assert!(output.contains("memory::VirtAddr { val: x, tag: 0 }"),
            "Multi-field struct literal should convert, got: {}", output);
    }

    #[test]
    fn test_module_struct_literal_in_return() {
        let output = convert_module_struct_literal("return addr.PhysAddr { val: p.val * 2 };");
        assert!(output.contains("addr::PhysAddr { val: p.val * 2 }"),
            "Struct literal in return should convert, got: {}", output);
    }

    #[test]
    fn test_module_struct_literal_preserves_method_calls() {
        // module.function() must NOT be converted — only module.UpperCase { ... }
        let output = convert_module_struct_literal("addr.make_phys(0x1000);");
        assert!(output.contains("addr.make_phys(0x1000)"),
            "Method calls should be preserved, got: {}", output);
    }

    #[test]
    fn test_module_struct_literal_preserves_field_access() {
        // p.val should NOT be converted — only when followed by { ... }
        let output = convert_module_struct_literal("let x = p.val;");
        assert!(output.contains("p.val"),
            "Field access should be preserved, got: {}", output);
    }

    #[test]
    fn test_module_struct_literal_preserves_lowercase() {
        // addr.phys_addr { ... } — lowercase after dot is NOT a struct name
        let output = convert_module_struct_literal("let p = addr.phys_addr { val: 0 };");
        assert!(!output.contains("addr::phys_addr"),
            "Lowercase after dot should NOT be converted (not a struct), got: {}", output);
    }

    #[test]
    fn test_module_struct_literal_in_string() {
        // Inside a string literal, no conversion
        let output = convert_module_struct_literal(r#"let s = "addr.PhysAddr { val: 0 }";"#);
        assert!(!output.contains("addr::PhysAddr"),
            "String content should NOT be converted, got: {}", output);
    }
}

