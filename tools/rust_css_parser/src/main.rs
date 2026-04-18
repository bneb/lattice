use cssparser::{Parser, ParserInput, Token};
use std::env;
use std::fs;

/// FNV-1a hash — must match Salt's css_fnv1a exactly.
fn fnv1a(data: &[u8]) -> u32 {
    let mut hash: u32 = 2166136261;
    for &byte in data {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    hash
}

/// Parse a simple integer value (e.g., "100" or "100px" → 100), matching Salt's parse_int.
fn parse_int(s: &str) -> i32 {
    let s = s.trim();
    let mut result: i32 = 0;
    let mut neg = false;
    let mut started = false;

    for (i, ch) in s.chars().enumerate() {
        if i == 0 && ch == '-' {
            neg = true;
            continue;
        }
        if ch.is_ascii_digit() {
            result = result * 10 + (ch as i32 - '0' as i32);
            started = true;
        } else {
            break; // Stop at non-digit (px, %, etc.)
        }
    }

    if !started { return -1; }
    if neg { -result } else { result }
}

struct Rule {
    hash: u32,
    specificity: u16,
    display: u8,
    flex_grow: i32,
    width: i32,
    height: i32,
    flex_dir: u8,
    position: u8,
    top: i32,
    right: i32,
    bottom: i32,
    left: i32,
    z_index: i32,
    z_index_set: bool,
    bg_r: u8,
    bg_g: u8,
    bg_b: u8,
    bg_set: bool,
}

impl Rule {
    fn new(hash: u32, specificity: u16) -> Self {
        Rule {
            hash,
            specificity,
            display: 255,
            flex_grow: -1,
            width: -1,
            height: -1,
            flex_dir: 255,
            position: 255,
            top: -1,
            right: -1,
            bottom: -1,
            left: -1,
            z_index: 0,
            z_index_set: false,
            bg_r: 0,
            bg_g: 0,
            bg_b: 0,
            bg_set: false,
        }
    }
}

fn parse_css(css: &str) -> Vec<Rule> {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    let mut rules = Vec::new();

    loop {
        // Skip whitespace
        parser.skip_whitespace();

        // Try to read a selector (everything before '{')
        let selector_start = parser.position();
        let mut found_block = false;

        // Consume tokens until we hit '{' or EOF
        let mut selector_tokens: Vec<String> = Vec::new();
        loop {
            let next = parser.next_including_whitespace();
            match next {
                Ok(token) => {
                    match token {
                        Token::CurlyBracketBlock => {
                            found_block = true;
                            break;
                        }
                        Token::Ident(ref s) => selector_tokens.push(s.to_string()),
                        Token::Delim('.') => selector_tokens.push(".".to_string()),
                        Token::IDHash(ref s) => selector_tokens.push(format!("#{}", s)),
                        Token::Hash(ref s) => selector_tokens.push(format!("#{}", s)),
                        Token::WhiteSpace(_) => selector_tokens.push(" ".to_string()),
                        _ => {
                            // Other tokens in the selector
                            selector_tokens.push(format!("{:?}", token));
                        }
                    }
                }
                Err(_) => break, // EOF
            }
        }

        if !found_block {
            break;
        }

        // Reconstruct the selector string
        let selector = selector_tokens.join("").trim().to_string();
        if selector.is_empty() {
            continue;
        }

        // Determine hash and specificity (matching Salt's css_lex_stylesheet)
        let (hash, specificity) = if selector.starts_with('.') {
            (fnv1a(selector[1..].as_bytes()), 10u16)
        } else if selector.starts_with('#') {
            (fnv1a(selector[1..].as_bytes()), 100u16)
        } else {
            (fnv1a(selector.as_bytes()), 1u16)
        };

        let mut rule = Rule::new(hash, specificity);

        // Parse the block contents (declarations)
        let _ = parser.parse_nested_block(|inner: &mut Parser<'_, '_>| -> Result<(), cssparser::ParseError<'_, ()>> {
            loop {
                inner.skip_whitespace();

                // Read property name
                let prop_name = match inner.next() {
                    Ok(Token::Ident(name)) => name.to_string().to_lowercase(),
                    Ok(Token::Semicolon) => continue,
                    Err(_) => break,
                    _ => {
                        // Skip unknown tokens
                        continue;
                    }
                };

                // Expect ':'
                match inner.next() {
                    Ok(Token::Colon) => {}
                    _ => continue,
                }

                inner.skip_whitespace();

                // Read value (everything until ';' or '}')
                let mut value_parts: Vec<String> = Vec::new();
                loop {
                    match inner.next() {
                        Ok(Token::Semicolon) => break,
                        Ok(Token::Ident(ref s)) => value_parts.push(s.to_string()),
                        Ok(Token::Number { int_value: Some(n), .. }) => value_parts.push(n.to_string()),
                        Ok(Token::Number { value, .. }) => value_parts.push(format!("{}", value)),
                        Ok(Token::Dimension { int_value: Some(n), unit, .. }) => {
                            value_parts.push(format!("{}{}", n, unit));
                        }
                        Ok(Token::Dimension { value, unit, .. }) => {
                            value_parts.push(format!("{}{}", value, unit));
                        }
                        Ok(Token::Hash(ref s)) => value_parts.push(format!("#{}", s)),
                        Ok(Token::IDHash(ref s)) => value_parts.push(format!("#{}", s)),
                        Ok(Token::WhiteSpace(_)) => value_parts.push(" ".to_string()),
                        Ok(Token::Delim(c)) => value_parts.push(c.to_string()),
                        Err(_) => break,
                        _ => {}
                    }
                }

                let value = value_parts.join("").trim().to_lowercase();

                // Apply property matching Salt's apply_rule_property
                match prop_name.as_str() {
                    "display" => {
                        rule.display = match value.as_str() {
                            "none" => 0,
                            "block" => 1,
                            "flex" => 2,
                            "inline" => 3,
                            _ => 255,
                        };
                    }
                    "flex-grow" => {
                        rule.flex_grow = parse_int(&value);
                    }
                    "flex-direction" => {
                        rule.flex_dir = match value.as_str() {
                            "row" => 0,
                            "column" => 1,
                            _ => 255,
                        };
                    }
                    "width" => {
                        rule.width = parse_int(&value);
                    }
                    "height" => {
                        rule.height = parse_int(&value);
                    }
                    "position" => {
                        rule.position = match value.as_str() {
                            "static" => 0,
                            "relative" => 1,
                            "absolute" => 2,
                            "fixed" => 3,
                            "sticky" => 4,
                            _ => 255,
                        };
                    }
                    "top" => { rule.top = parse_int(&value); }
                    "right" => { rule.right = parse_int(&value); }
                    "bottom" => { rule.bottom = parse_int(&value); }
                    "left" => { rule.left = parse_int(&value); }
                    "z-index" => {
                        rule.z_index = parse_int(&value);
                        rule.z_index_set = true;
                    }
                    "background-color" | "background" => {
                        if value.starts_with('#') && value.len() >= 7 {
                            if let (Ok(r), Ok(g), Ok(b)) = (
                                u8::from_str_radix(&value[1..3], 16),
                                u8::from_str_radix(&value[3..5], 16),
                                u8::from_str_radix(&value[5..7], 16),
                            ) {
                                rule.bg_r = r;
                                rule.bg_g = g;
                                rule.bg_b = b;
                                rule.bg_set = true;
                            }
                        }
                    }
                    _ => {} // Unsupported property — skip
                }
            }
            Ok(())
        });

        rules.push(rule);
    }

    rules
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <css_file>", args[0]);
        std::process::exit(1);
    }

    let css = fs::read_to_string(&args[1]).expect("Failed to read file");
    let rules = parse_css(&css);

    println!("== CSS LEXER IR ");
    for rule in &rules {
        print!("RULE H={} S={} D={} FG={}", rule.hash, rule.specificity, rule.display, rule.flex_grow);
        print!(" W={} HT={}", rule.width, rule.height);
        print!(" FD={}", rule.flex_dir);
        print!(" POS={} T={} R={} B={} L={}", rule.position, rule.top, rule.right, rule.bottom, rule.left);
        if rule.z_index_set {
            print!(" Z={}", rule.z_index);
        }
        if rule.bg_set {
            print!(" BG={},{},{}", rule.bg_r, rule.bg_g, rule.bg_b);
        }
        println!();
    }
}
