use swc_common::sync::Lrc;
use swc_common::{FileName, SourceMap};
use swc_ecma_parser::{lexer::Lexer, StringInput, Syntax, Token};
use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <js_file>", args[0]);
        std::process::exit(1);
    }

    let source = fs::read_to_string(&args[1]).expect("Failed to read file");
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(Lrc::new(FileName::Custom("input.js".to_string())), source);

    let lexer = Lexer::new(
        Syntax::default(),
        Default::default(),
        StringInput::from(&*fm),
        None,
    );

    println!("== JS LEXER IR ==");

    for token in lexer {
        use swc_ecma_parser::token::Token::*;
        let name = match token.token {
            Word(w) => match w {
                swc_ecma_parser::token::Word::Keyword(k) => match k {
                    swc_ecma_parser::token::Keyword::Let => "Keyword(Let)",
                    swc_ecma_parser::token::Keyword::Var => "Keyword(Var)",
                    swc_ecma_parser::token::Keyword::Const => "Keyword(Const)",
                    _ => "Unknown",
                },
                swc_ecma_parser::token::Word::Ident(_) => "Identifier",
                _ => "Unknown",
            },
            Num { .. } => "Number",
            BinOp(op) => match op {
                swc_ecma_parser::token::BinOpToken::Add => "Punctuation(Plus)",
                swc_ecma_parser::token::BinOpToken::Sub => "Punctuation(Minus)",
                swc_ecma_parser::token::BinOpToken::Mul => "Punctuation(Asterisk)",
                swc_ecma_parser::token::BinOpToken::Div => "Punctuation(Slash)",
                _ => "Unknown",
            },
            Assign(_) => "Punctuation(Assign)",
            LParen => "Punctuation(OpenParen)",
            RParen => "Punctuation(CloseParen)",
            Semi => "Punctuation(SemiColon)",
            _ => "Unknown",
        };
        
        println!("TOKEN {}", name);
    }
}
