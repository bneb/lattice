use super::spacing;

/// Core line-by-line formatting pipeline.
/// Produces a Vec of formatted lines from Salt source text.
pub fn format_lines(source: &str) -> Vec<String> {
    let lines: Vec<&str> = source.lines().collect();
    let mut result: Vec<String> = Vec::with_capacity(lines.len());
    let mut depth: usize = 0;
    let mut in_multiline_string = false;

    for line in &lines {
        // Handle multi-line string state (pass through untouched)
        if in_multiline_string {
            let trimmed = line.trim_end().to_string();
            if trimmed.trim_end().ends_with('"') {
                in_multiline_string = false;
            }
            result.push(trimmed);
            continue;
        }

        // Check if this line starts a multi-line string
        if is_multiline_string_start(line) {
            let trimmed = line.trim_end().to_string();
            if !trimmed.trim_end().ends_with('"') {
                in_multiline_string = true;
            }
            result.push(trimmed);
            continue;
        }

        let (formatted, _) = format_line(line, depth);
        result.push(formatted);

        // Update depth based on this line (outside strings/comments)
        let stripped = line.trim_start();
        if !stripped.is_empty() && !stripped.starts_with("//") {
            let (opens, closes) = count_braces(line);
            depth = depth.saturating_add(opens).saturating_sub(closes);
        }
    }

    result
}

/// Format a single line: strip trailing whitespace, re-indent, normalize spacing.
/// Returns (formatted_line, was_meaningful).
pub fn format_line(line: &str, depth: usize) -> (String, bool) {
    let trimmed = line.trim_end();
    let stripped = trimmed.trim_start();

    // Blank line
    if stripped.is_empty() {
        return (String::new(), false);
    }

    // Comment-only lines: re-indent to current depth
    if stripped.starts_with("//") || stripped.starts_with("///") || stripped.starts_with("//!") {
        return (indent_line(stripped, depth), false);
    }

    // Count braces before formatting
    let (opens, closes) = count_braces(trimmed);

    // Effective depth: if line starts with `}`, dedent by number of leading `}`
    let effective_depth = {
        let close_count = stripped.chars().take_while(|c| *c == '}').count();
        if close_count > 0 {
            depth.saturating_sub(close_count)
        } else {
            depth
        }
    };

    // Re-indent
    let code = format!("{:indent$}{}", "", stripped, indent = effective_depth * 4);

    // Normalize spacing around operators (outside strings/comments)
    let result = spacing::normalize_spacing(&code);

    // silence unused warning for `closes` — still used conceptually
    let _ = closes;
    let _ = opens;

    (result, true)
}

/// Count opening and closing braces outside of strings and comments on a line.
pub fn count_braces(line: &str) -> (usize, usize) {
    let mut in_string = false;
    let mut opens = 0;
    let mut closes = 0;
    let bytes = line.as_bytes();
    let mut i = 0;

    while i < line.len() {
        if !in_string && i + 1 < line.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            break;
        }
        if bytes[i] == b'"' && (i == 0 || bytes[i - 1] != b'\\') {
            in_string = !in_string;
            i += 1;
            continue;
        }
        if in_string {
            i += 1;
            continue;
        }
        match bytes[i] {
            b'{' => opens += 1,
            b'}' => closes += 1,
            _ => {}
        }
        i += 1;
    }
    (opens, closes)
}

/// Check if trimmed line starts with a closing brace.
#[allow(dead_code)]
pub fn starts_with_close_brace(s: &str) -> bool {
    s.starts_with('}')
}

/// Indent content to the given depth (4 spaces per level).
pub fn indent_line(content: &str, depth: usize) -> String {
    if content.is_empty() {
        String::new()
    } else {
        format!("{:indent$}{}", "", content, indent = depth * 4)
    }
}

/// Return true if this line contains a string that crosses line boundaries.
pub fn is_multiline_string_start(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with('"') {
        let mut in_str = false;
        for (i, c) in trimmed.char_indices() {
            if c == '"' && (i == 0 || trimmed.as_bytes()[i - 1] != b'\\') {
                in_str = !in_str;
            }
        }
        in_str
    } else {
        false
    }
}
