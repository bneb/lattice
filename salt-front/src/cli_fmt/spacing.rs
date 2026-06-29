/// Normalize spacing around binary operators on a single line.
/// Done character-by-character, tracking string/comment context.
pub fn normalize_spacing(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let bytes = line.as_bytes();
    let len = line.len();
    let mut i = 0;
    let mut in_string = false;

    while i < len {
        // Comment — copy rest verbatim
        if !in_string && i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            out.push_str(&line[i..]);
            break;
        }

        // String tracking
        if bytes[i] == b'"' && (i == 0 || bytes[i - 1] != b'\\') {
            in_string = !in_string;
            out.push('"');
            i += 1;
            continue;
        }
        if in_string {
            out.push(bytes[i] as char);
            i += 1;
            continue;
        }

        // Multi-char operators
        if i + 1 < len {
            let two = [bytes[i], bytes[i + 1]];
            let two_str = unsafe { std::str::from_utf8_unchecked(&two) };

            if matches!(
                two_str,
                "==" | "!=" | "<=" | ">=" | "&&" | "||" | "->" | "=>"
                    | "+=" | "-=" | "*=" | "/=" | "%="
                    | "&=" | "|=" | "^=" | "<<" | ">>"
            ) {
                if !out.is_empty() && !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push_str(two_str);
                if i + 2 < len && bytes[i + 2] != b' ' {
                    out.push(' ');
                }
                i += 2;
                continue;
            }
        }

        // Single-char operators
        let c = bytes[i] as char;
        match c {
            '=' => {
                handle_assign(&mut out);
                out.push('=');
                if i + 1 < len {
                    let n = bytes[i + 1] as char;
                    if n.is_ascii_alphanumeric() || n == '_' || n == '(' || n == '&' || n == '*'
                    {
                        out.push(' ');
                    }
                }
                i += 1;
            }
            '+' | '-' if i > 0 && i + 1 < len => {
                let prev = bytes[i - 1] as char;
                let next = bytes[i + 1] as char;
                let prev_is_expr =
                    prev.is_ascii_alphanumeric() || prev == '_' || prev == ')' || prev == ']';
                let next_is_expr =
                    next.is_ascii_alphanumeric() || next == '_' || next == '(' || next == '[';
                if prev_is_expr || next_is_expr {
                    if prev_is_expr && !out.ends_with(' ') {
                        out.push(' ');
                    }
                    out.push(c);
                    if next_is_expr && next != ' ' {
                        out.push(' ');
                    }
                } else {
                    out.push(c);
                }
                i += 1;
            }
            '*' | '/' | '%' => {
                if i > 0 && i + 1 < len {
                    let prev = bytes[i - 1] as char;
                    let next = bytes[i + 1] as char;
                    let pe = prev.is_ascii_alphanumeric()
                        || prev == '_'
                        || prev == ')'
                        || prev == ']';
                    let ne = next.is_ascii_alphanumeric()
                        || next == '_'
                        || next == '('
                        || next == '[';
                    if pe && ne {
                        if !out.ends_with(' ') {
                            out.push(' ');
                        }
                        out.push(c);
                        if next != ' ' {
                            out.push(' ');
                        }
                        i += 1;
                        continue;
                    }
                }
                out.push(c);
                i += 1;
            }
            '>' | '<' => {
                if !out.is_empty() && !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push(c);
                let nok = i + 1 < len
                    && bytes[i + 1] != b' '
                    && bytes[i + 1] != b'>'
                    && bytes[i + 1] != b'<';
                if nok {
                    out.push(' ');
                }
                i += 1;
            }
            '&' | '|' | '^' => {
                if i > 0 {
                    let p = bytes[i - 1] as char;
                    if (p.is_ascii_alphanumeric() || p == '_' || p == ')' || p == ']')
                        && !out.ends_with(' ')
                    {
                        out.push(' ');
                    }
                }
                out.push(c);
                if i + 1 < len {
                    let n = bytes[i + 1] as char;
                    if n.is_ascii_alphanumeric() || n == '_' || n == '(' || n == '[' {
                        out.push(' ');
                    }
                }
                i += 1;
            }
            '{' => {
                // Ensure space before opening brace when preceded by non-space
                if !out.is_empty() && !out.ends_with(' ') && !out.ends_with('{') {
                    out.push(' ');
                }
                out.push('{');
                i += 1;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }

    out
}

/// Ensure a space before `=` in assignment context.
fn handle_assign(out: &mut String) {
    if !out.is_empty() && !out.ends_with(' ') {
        out.push(' ');
    }
}
