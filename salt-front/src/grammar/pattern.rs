/// Pattern AST for Salt match expressions and let-else destructuring
/// 
/// This module defines the pattern language used in:
/// - `match expr { Pattern => body, ... }`
/// - `let Pattern = expr else { diverging_block }`
/// - Future: `if let Pattern = expr { ... }`
use syn::{Ident, Token, Lit};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;

/// Pattern AST for match arms and let-else destructuring
#[derive(Clone, Debug, PartialEq)]
pub enum Pattern {
    /// Wildcard: `_`
    Wildcard,
    
    /// Literal: `0`, `"hello"`, `true`, `42i32`
    Literal(Lit),
    
    /// Identifier binding: `x`, `mut x`
    Ident {
        name: Ident,
        mutable: bool,
    },
    
    /// Enum variant: `Some(x)`, `None`, `Err(e)`, `Result::Ok(v)`
    Variant {
        path: Vec<Ident>,
        fields: Option<Vec<Pattern>>,  // None for unit variants like `None`
    },
    
    /// Tuple: `(a, b, c)`
    Tuple(Vec<Pattern>),
    
    /// Struct destructuring: `Point { x, y }`, `Point { x: a, y: b }`
    Struct {
        name: Ident,
        fields: Vec<PatternField>,
    },
    
    /// Or pattern: `A | B | C` (for matching multiple variants)
    Or(Vec<Pattern>),
    
    /// Rest pattern: `..` (for ignoring remaining fields)
    Rest,
}

/// A single field in a struct pattern
#[derive(Clone, Debug, PartialEq)]
pub struct PatternField {
    pub name: Ident,
    pub pattern: Option<Pattern>,  // None means shorthand `{ x }` == `{ x: x }`
}

impl Parse for Pattern {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Check for wildcard: `_`
        if input.peek(Token![_]) {
            input.parse::<Token![_]>()?;
            return Ok(Pattern::Wildcard);
        }
        
        // Check for rest pattern: `..`
        if input.peek(Token![..]) {
            input.parse::<Token![..]>()?;
            return Ok(Pattern::Rest);
        }
        
        // Check for literal
        if input.peek(Lit) {
            let lit: Lit = input.parse()?;
            return Ok(Pattern::Literal(lit));
        }
        
        // Check for tuple: `(a, b, c)`
        if input.peek(syn::token::Paren) {
            let content;
            syn::parenthesized!(content in input);
            let patterns: Punctuated<Pattern, Token![,]> = 
                content.parse_terminated(Pattern::parse, Token![,])?;
            return Ok(Pattern::Tuple(patterns.into_iter().collect()));
        }
        
        // Check for mutable binding: `mut x`
        if input.peek(Token![mut]) {
            input.parse::<Token![mut]>()?;
            let name: Ident = input.parse()?;
            return Ok(Pattern::Ident { name, mutable: true });
        }
        
        // Must be an identifier, variant, or struct pattern
        let first_ident: Ident = input.parse()?;
        
        // Check for path continuation: `Result::Ok`
        let mut path = vec![first_ident.clone()];
        while input.peek(Token![::]) {
            input.parse::<Token![::]>()?;
            let segment: Ident = input.parse()?;
            path.push(segment);
        }
        
        // Check for variant fields: `Some(x)` or `Point { x, y }`
        if input.peek(syn::token::Paren) {
            // Variant with tuple fields: `Some(x)`, `Ok(value)`
            let content;
            syn::parenthesized!(content in input);
            let patterns: Punctuated<Pattern, Token![,]> = 
                content.parse_terminated(Pattern::parse, Token![,])?;
            return Ok(Pattern::Variant {
                path,
                fields: Some(patterns.into_iter().collect()),
            });
        } else if input.peek(syn::token::Brace) {
            // Struct pattern: `Point { x, y }`
            let content;
            syn::braced!(content in input);
            let mut fields = Vec::new();
            
            while !content.is_empty() {
                if content.peek(Token![..]) {
                    // Rest pattern in struct: `Point { x, .. }`
                    content.parse::<Token![..]>()?;
                    // We could track this, but for now just consume it
                    break;
                }
                
                let field_name: Ident = content.parse()?;
                let pattern = if content.peek(Token![:]) {
                    content.parse::<Token![:]>()?;
                    Some(content.parse()?)
                } else {
                    None  // Shorthand: `{ x }` means `{ x: x }`
                };
                
                fields.push(PatternField { name: field_name, pattern });
                
                if content.peek(Token![,]) {
                    content.parse::<Token![,]>()?;
                } else {
                    break;
                }
            }
            
            // For struct patterns, use the last segment as the struct name
            let name = path.pop().unwrap_or(first_ident);
            return Ok(Pattern::Struct { name, fields });
        }
        
        // Check for Or pattern: `A | B`
        if input.peek(Token![|]) {
            let mut alternatives = vec![Pattern::Variant { path: path.clone(), fields: None }];
            while input.peek(Token![|]) {
                input.parse::<Token![|]>()?;
                let alt: Pattern = input.parse()?;
                alternatives.push(alt);
            }
            return Ok(Pattern::Or(alternatives));
        }
        
        // Simple identifier or unit variant
        if path.len() == 1 {
            // Could be a binding or unit variant
            // We treat single lowercase identifiers as bindings
            let name = &path[0];
            let first_char = name.to_string().chars().next().unwrap_or('a');
            if first_char.is_lowercase() && name != "true" && name != "false" {
                return Ok(Pattern::Ident { name: name.clone(), mutable: false });
            }
        }
        
        // Unit variant like `None`
        Ok(Pattern::Variant { path, fields: None })
    }
}

impl Pattern {
    /// Returns true if this pattern is irrefutable (always matches)
    pub fn is_irrefutable(&self) -> bool {
        match self {
            Pattern::Wildcard => true,
            Pattern::Ident { .. } => true,
            Pattern::Rest => true,
            Pattern::Tuple(pats) => pats.iter().all(|p| p.is_irrefutable()),
            Pattern::Struct { fields, .. } => {
                fields.iter().all(|f| {
                    f.pattern.as_ref().is_none_or(|p| p.is_irrefutable())
                })
            }
            Pattern::Variant { .. } => false,  // Variants are refutable
            Pattern::Literal(_) => false,       // Literals are refutable
            Pattern::Or(_) => false,            // Or patterns are refutable
        }
    }
    
    /// Extract all bound variable names from this pattern
    pub fn bound_names(&self) -> Vec<(Ident, bool)> {
        let mut names = Vec::new();
        self.collect_bound_names(&mut names);
        names
    }
    
    fn collect_bound_names(&self, names: &mut Vec<(Ident, bool)>) {
        match self {
            Pattern::Wildcard | Pattern::Rest | Pattern::Literal(_) => {}
            Pattern::Ident { name, mutable } => {
                names.push((name.clone(), *mutable));
            }
            Pattern::Variant { fields, .. } => {
                if let Some(pats) = fields {
                    for pat in pats {
                        pat.collect_bound_names(names);
                    }
                }
            }
            Pattern::Tuple(pats) => {
                for pat in pats {
                    pat.collect_bound_names(names);
                }
            }
            Pattern::Struct { fields, .. } => {
                for field in fields {
                    if let Some(pat) = &field.pattern {
                        pat.collect_bound_names(names);
                    } else {
                        // Shorthand: `{ x }` binds `x`
                        names.push((field.name.clone(), false));
                    }
                }
            }
            Pattern::Or(alts) => {
                // All alternatives must bind the same names
                if let Some(first) = alts.first() {
                    first.collect_bound_names(names);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_wildcard() {
        let pat: Pattern = syn::parse_str("_").unwrap();
        assert!(matches!(pat, Pattern::Wildcard));
    }
    
    #[test]
    fn test_parse_literal() {
        let pat: Pattern = syn::parse_str("42").unwrap();
        assert!(matches!(pat, Pattern::Literal(_)));
    }
    
    #[test]
    fn test_parse_ident() {
        let pat: Pattern = syn::parse_str("x").unwrap();
        assert!(matches!(pat, Pattern::Ident { mutable: false, .. }));
    }
    
    #[test]
    fn test_parse_mut_ident() {
        let pat: Pattern = syn::parse_str("mut x").unwrap();
        assert!(matches!(pat, Pattern::Ident { mutable: true, .. }));
    }
    
    #[test]
    fn test_parse_tuple() {
        let pat: Pattern = syn::parse_str("(a, b, c)").unwrap();
        if let Pattern::Tuple(pats) = pat {
            assert_eq!(pats.len(), 3);
        } else {
            panic!("Expected tuple pattern");
        }
    }
    
    #[test]
    fn test_parse_variant_with_fields() {
        let pat: Pattern = syn::parse_str("Some(x)").unwrap();
        if let Pattern::Variant { path, fields } = pat {
            assert_eq!(path.len(), 1);
            assert_eq!(path[0].to_string(), "Some");
            assert!(fields.is_some());
        } else {
            panic!("Expected variant pattern");
        }
    }
    
    #[test]
    fn test_parse_unit_variant() {
        let pat: Pattern = syn::parse_str("None").unwrap();
        if let Pattern::Variant { path, fields } = pat {
            assert_eq!(path[0].to_string(), "None");
            assert!(fields.is_none());
        } else {
            panic!("Expected unit variant pattern");
        }
    }
    
    #[test]
    fn test_bound_names() {
        let pat: Pattern = syn::parse_str("Some(x)").unwrap();
        let names = pat.bound_names();
        assert_eq!(names.len(), 1);
        assert_eq!(names[0].0.to_string(), "x");
    }

    #[test]
    fn test_parse_struct_pattern() {
        let pat: Pattern = syn::parse_str("Point { x, y }").unwrap();
        if let Pattern::Struct { name, fields } = pat {
            assert_eq!(name.to_string(), "Point");
            assert_eq!(fields.len(), 2);
        } else {
            panic!("Expected struct pattern");
        }
    }

    #[test]
    fn test_parse_or_pattern() {
        let pat: Pattern = syn::parse_str("A | B").unwrap();
        assert!(matches!(pat, Pattern::Or(_)));
    }

    #[test]
    fn test_parse_rest_pattern() {
        let pat: Pattern = syn::parse_str("..").unwrap();
        assert!(matches!(pat, Pattern::Rest));
    }

    #[test]
    fn test_is_irrefutable_wildcard() {
        let pat: Pattern = syn::parse_str("_").unwrap();
        assert!(pat.is_irrefutable());
    }

    #[test]
    fn test_is_irrefutable_ident_false() {
        let pat: Pattern = syn::parse_str("Some(x)").unwrap();
        assert!(!pat.is_irrefutable());
    }
}
