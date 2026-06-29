/// Post-process the formatted lines: merge standalone braces, ensure blank lines between items.
pub fn post_process(lines: &[String]) -> String {
    let mut merged = merge_standalone_braces(lines);
    ensure_blank_lines_between_items(&mut merged);
    merged.join("\n")
}

/// Merge lines that consist only of `{` into the previous line (opening brace on same line).
fn merge_standalone_braces(lines: &[String]) -> Vec<String> {
    let mut result: Vec<String> = Vec::with_capacity(lines.len());
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "{" && !result.is_empty() {
            let last = result.pop().unwrap_or_default();
            let last_trimmed = last.trim_end();
            if last_trimmed.ends_with('{') {
                result.push(last);
                result.push(line.to_string());
            } else {
                result.push(format!("{} {{", last_trimmed));
            }
        } else {
            result.push(line.to_string());
        }
    }
    result
}

/// Ensure exactly one blank line between top-level items (depth-0 items).
/// Also collapses consecutive blank lines.
fn ensure_blank_lines_between_items(lines: &mut Vec<String>) {
    if lines.is_empty() {
        return;
    }

    // Collapse consecutive blank lines to at most one
    let mut collapsed: Vec<String> = Vec::with_capacity(lines.len());
    let mut prev_blank = false;
    for line in lines.drain(..) {
        let is_blank = line.trim().is_empty();
        if is_blank && prev_blank {
            continue;
        }
        prev_blank = is_blank;
        collapsed.push(line);
    }
    *lines = collapsed;

    // Ensure blank line before top-level items (unless preceded by blank or at file start)
    let mut with_blanks: Vec<String> = Vec::with_capacity(lines.len());
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let is_top = !trimmed.is_empty()
            && !trimmed.starts_with("//")
            && !trimmed.starts_with("///")
            && !trimmed.starts_with("//!")
            && !trimmed.starts_with('{')
            && !trimmed.starts_with('}')
            && has_top_level_keyword(trimmed);

        if is_top && idx > 0 {
            let prev_blank = with_blanks.last().map_or(true, |l| l.trim().is_empty());
            if !prev_blank {
                with_blanks.push(String::new());
            }
        }
        with_blanks.push(line.to_string());
    }
    *lines = with_blanks;
}

/// Check if trimmed content starts with a top-level declaration keyword.
fn has_top_level_keyword(s: &str) -> bool {
    s.starts_with("fn ")
        || s.starts_with("pub fn ")
        || s.starts_with("struct ")
        || s.starts_with("pub struct ")
        || s.starts_with("enum ")
        || s.starts_with("pub enum ")
        || s.starts_with("impl")
        || s.starts_with("use ")
        || s.starts_with("extern")
        || s.starts_with("const ")
        || s.starts_with("pub const ")
        || s.starts_with("type ")
        || s.starts_with("pub type ")
        || s.starts_with("union ")
        || s.starts_with("pub union ")
        || s.starts_with("mod ")
        || s.starts_with("pub mod ")
        || s.starts_with("trait ")
        || s.starts_with("pub trait ")
}
