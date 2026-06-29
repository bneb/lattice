//! Semantic Tokens — syntax highlighting via LSP
//!
//! Encodes token types and modifiers for Salt source files.
//! Provides the standard semantic tokens legend and encodes a file's tokens
//! as delta-encoded integers per the LSP spec.

use tower_lsp::lsp_types::{
    SemanticToken, SemanticTokensLegend, SemanticTokenType, SemanticTokenModifier,
};

pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::NAMESPACE,   // 0
            SemanticTokenType::KEYWORD,      // 1
            SemanticTokenType::FUNCTION,     // 2
            SemanticTokenType::TYPE,         // 3
            SemanticTokenType::PARAMETER,    // 4
            SemanticTokenType::VARIABLE,     // 5
            SemanticTokenType::PROPERTY,     // 6
            SemanticTokenType::STRING,       // 7
            SemanticTokenType::NUMBER,       // 8
            SemanticTokenType::COMMENT,      // 9
            SemanticTokenType::OPERATOR,     // 10
            SemanticTokenType::DECORATOR,    // 11
            SemanticTokenType::MODIFIER,     // 12
        ],
        token_modifiers: vec![
            SemanticTokenModifier::DECLARATION,
            SemanticTokenModifier::READONLY,
            SemanticTokenModifier::DEFAULT_LIBRARY,
        ],
    }
}

// Token type indices (matching LEGEND order)
const TK_KEYWORD: u32 = 1;
const TK_TYPE: u32 = 3;
const TK_VARIABLE: u32 = 5;
const TK_STRING: u32 = 7;
const TK_NUMBER: u32 = 8;
const TK_COMMENT: u32 = 9;
const TK_DECORATOR: u32 = 11;
const TK_MODIFIER: u32 = 12;

const SALT_KEYWORDS: &[&str] = &[
    "fn", "let", "mut", "return", "if", "else", "for", "while", "loop",
    "match", "break", "continue", "move", "pub", "use", "unsafe", "impl",
    "struct", "enum", "trait", "package", "as", "where",
];

const BUILTIN_TYPES: &[&str] = &[
    "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64",
    "f32", "f64", "bool", "char", "void",
    "String", "StringView", "Ptr",
];

/// Tokenize source text into delta-encoded SemanticToken array.
pub fn tokenize(source: &str) -> Vec<SemanticToken> {
    let mut tokens: Vec<RawToken> = Vec::new();

    for (line_idx, line) in source.lines().enumerate() {
        let bytes = line.as_bytes();
        let mut col = 0usize;
        let line_u32 = line_idx as u32;

        while col < bytes.len() {
            // Skip whitespace
            if bytes[col].is_ascii_whitespace() {
                col += 1;
                continue;
            }

            // Line comments
            if col + 1 < bytes.len() && bytes[col] == b'/' && bytes[col + 1] == b'/' {
                let len = bytes.len() - col;
                tokens.push(RawToken { line: line_u32, col: col as u32, len: len as u32, ty: TK_COMMENT, mod_bits: 0 });
                break; // rest of line is comment
            }

            // Decorators (@attribute)
            if bytes[col] == b'@' {
                let start = col;
                col += 1;
                while col < bytes.len() && (bytes[col].is_ascii_alphanumeric() || bytes[col] == b'(' || bytes[col] == b')' || bytes[col] == b'_') {
                    col += 1;
                }
                tokens.push(RawToken { line: line_u32, col: start as u32, len: (col - start) as u32, ty: TK_DECORATOR, mod_bits: 0 });
                continue;
            }

            // String literals
            if bytes[col] == b'"' {
                let start = col;
                col += 1;
                while col < bytes.len() && bytes[col] != b'"' {
                    if bytes[col] == b'\\' { col += 1; } // skip escape
                    col += 1;
                }
                if col < bytes.len() { col += 1; } // closing quote
                tokens.push(RawToken { line: line_u32, col: start as u32, len: (col - start) as u32, ty: TK_STRING, mod_bits: 0 });
                continue;
            }

            // Number literals
            if bytes[col].is_ascii_digit() {
                let start = col;
                while col < bytes.len() && (bytes[col].is_ascii_digit() || bytes[col] == b'.' || bytes[col] == b'_') {
                    col += 1;
                }
                tokens.push(RawToken { line: line_u32, col: start as u32, len: (col - start) as u32, ty: TK_NUMBER, mod_bits: 0 });
                continue;
            }

            // Identifiers and keywords
            if is_ident_start(bytes[col]) {
                let start = col;
                while col < bytes.len() && is_ident_cont(bytes[col]) {
                    col += 1;
                }
                let word = &line[start..col];
                let (ty, mod_bits) = classify_word(word);
                tokens.push(RawToken { line: line_u32, col: start as u32, len: (col - start) as u32, ty, mod_bits });
                continue;
            }

            col += 1; // skip operators/punctuation
        }
    }

    delta_encode(&tokens)
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_ident_cont(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Classify an identifier word into a token type and modifier bits.
fn classify_word(word: &str) -> (u32, u32) {
    if SALT_KEYWORDS.contains(&word) {
        return (TK_KEYWORD, 0);
    }
    if BUILTIN_TYPES.contains(&word) {
        return (TK_TYPE, 1 << 2); // defaultLibrary
    }
    if word == "requires" || word == "ensures" || word == "invariant" {
        return (TK_MODIFIER, 0);
    }
    // Heuristic: capitalized words are types
    if word.chars().next().is_some_and(|c| c.is_uppercase()) {
        return (TK_TYPE, 0);
    }
    (TK_VARIABLE, 0)
}

struct RawToken {
    line: u32,
    col: u32,
    len: u32,
    ty: u32,
    mod_bits: u32,
}

/// Delta-encode raw tokens into LSP SemanticToken array.
fn delta_encode(raw: &[RawToken]) -> Vec<SemanticToken> {
    if raw.is_empty() { return vec![]; }

    let mut result = Vec::with_capacity(raw.len());
    result.push(SemanticToken {
        delta_line: raw[0].line,
        delta_start: raw[0].col,
        length: raw[0].len,
        token_type: raw[0].ty,
        token_modifiers_bitset: raw[0].mod_bits,
    });

    for i in 1..raw.len() {
        let prev = &raw[i - 1];
        let curr = &raw[i];
        result.push(SemanticToken {
            delta_line: curr.line - prev.line,
            delta_start: if curr.line == prev.line {
                curr.col - prev.col
            } else {
                curr.col
            },
            length: curr.len,
            token_type: curr.ty,
            token_modifiers_bitset: curr.mod_bits,
        });
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_keywords() {
        let src = "fn main() -> i32 {\n    return 0;\n}";
        let tokens = tokenize(src);
        assert!(!tokens.is_empty(), "should produce tokens");
        // Find the function keyword
        let has_fn = tokens.iter().any(|t| t.token_type == TK_KEYWORD);
        assert!(has_fn, "should have keyword tokens");
    }

    #[test]
    fn test_tokenize_types() {
        let src = "let x: i32 = 42;";
        let tokens = tokenize(src);
        let has_type = tokens.iter().any(|t| t.token_type == TK_TYPE);
        assert!(has_type, "should have type tokens");
    }

    #[test]
    fn test_tokenize_string() {
        let src = r#"let s = "hello";"#;
        let tokens = tokenize(src);
        let has_string = tokens.iter().any(|t| t.token_type == TK_STRING);
        assert!(has_string, "should have string tokens");
    }

    #[test]
    fn test_tokenize_comment() {
        let src = "// this is a comment\nlet x = 1;";
        let tokens = tokenize(src);
        let has_comment = tokens.iter().any(|t| t.token_type == TK_COMMENT);
        assert!(has_comment, "should have comment tokens");
    }

    #[test]
    fn test_tokenize_decorator() {
        let src = "@export\nfn foo() -> i32 { return 0; }";
        let tokens = tokenize(src);
        let has_decorator = tokens.iter().any(|t| t.token_type == TK_DECORATOR);
        assert!(has_decorator, "should have decorator tokens");
    }

    #[test]
    fn test_tokenize_contract() {
        let src = "fn div(a: i32, b: i32) -> i32\n    requires(b != 0)\n{ return a / b; }";
        let tokens = tokenize(src);
        let has_requires = tokens.iter().any(|t| t.token_type == TK_MODIFIER);
        assert!(has_requires, "should have requires as modifier token");
    }

    #[test]
    fn test_delta_encoding_same_line() {
        let raw = vec![
            RawToken { line: 0, col: 0, len: 2, ty: TK_KEYWORD, mod_bits: 0 },
            RawToken { line: 0, col: 3, len: 4, ty: TK_KEYWORD, mod_bits: 0 },
        ];
        let encoded = delta_encode(&raw);
        assert_eq!(encoded.len(), 2);
        assert_eq!(encoded[0].delta_line, 0);
        assert_eq!(encoded[0].delta_start, 0);
        assert_eq!(encoded[1].delta_line, 0);    // same line as previous
        assert_eq!(encoded[1].delta_start, 3);   // col 3 - col 0
    }

    #[test]
    fn test_empty_source() {
        let tokens = tokenize("");
        assert!(tokens.is_empty());
    }
}
