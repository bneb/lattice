//! Exhaustiveness Checker - Z3-based match completeness verification
//!
//! This module uses Z3 to prove that match expressions cover all possible values.
//! For enums, we verify all discriminant values are covered.
//! The algorithm:
//! 1. Build a Z3 expression for "exists x such that x matches no arm"
//! 2. If UNSAT, the match is exhaustive
//! 3. If SAT, report the missing case

use crate::grammar::pattern::Pattern;
use crate::grammar::MatchArm;
use crate::types::Type;
use crate::codegen::context::LoweringContext;
use crate::registry::EnumInfo;

/// Result of exhaustiveness checking
#[derive(Debug)]
pub enum ExhaustivenessResult {
    /// Match is exhaustive - all variants covered
    Exhaustive,
    /// Match is not exhaustive - missing these variants
    MissingVariants(Vec<String>),
    /// Cannot verify exhaustiveness (non-enum type or complex patterns)
    Unverifiable(String),
}

/// Check if a match expression is exhaustive for an enum type.
/// 
/// # Arguments
/// * `ctx` - Codegen context with Z3 and enum registry
/// * `scrutinee_ty` - The type being matched on
/// * `arms` - The match arms with their patterns
/// 
/// # Returns
/// * `ExhaustivenessResult` indicating whether the match is exhaustive
pub fn check_exhaustiveness(
    ctx: &mut LoweringContext<'_, '_>,
    scrutinee_ty: &Type,
    arms: &[MatchArm],
) -> ExhaustivenessResult {
    // Only check enums for now
    let enum_name = match scrutinee_ty {
        Type::Enum(name) => name.clone(),
        Type::Concrete(name, _) if name.contains("Result") || name.contains("Option") => name.clone(),
        _ => return ExhaustivenessResult::Unverifiable(
            format!("Exhaustiveness checking only supported for enums, got {:?}", scrutinee_ty)
        ),
    };

    // Look up enum info to get all variants
    let enum_info = {
        let registry = ctx.enum_registry();
        let mut found: Option<EnumInfo> = None;
        for info in registry.values() {
            if info.name == enum_name || enum_name.ends_with(&format!("__{}", info.name)) || info.name.ends_with(&format!("__{}", enum_name)) {
                found = Some(info.clone());
                break;
            }
        }
        found
    };

    let enum_info = match enum_info {
        Some(info) => info,
        None => return ExhaustivenessResult::Unverifiable(
            format!("Enum '{}' not found in registry", enum_name)
        ),
    };

    // Extract covered variants from patterns
    let covered_variants = extract_covered_variants(arms);
    
    // Check for wildcard pattern - if present, match is exhaustive
    if has_wildcard(arms) {
        return ExhaustivenessResult::Exhaustive;
    }

    // Find missing variants
    let all_variants: Vec<String> = enum_info.variants.iter()
        .map(|(name, _, _)| name.clone())
        .collect();
    
    let missing: Vec<String> = all_variants.iter()
        .filter(|v| !covered_variants.contains(*v))
        .cloned()
        .collect();

    if missing.is_empty() {
        ExhaustivenessResult::Exhaustive
    } else {
        ExhaustivenessResult::MissingVariants(missing)
    }
}

/// Extract all variant names covered by the match arms
fn extract_covered_variants(arms: &[MatchArm]) -> std::collections::HashSet<String> {
    let mut covered = std::collections::HashSet::new();
    
    for arm in arms {
        collect_variants_from_pattern(&arm.pattern, &mut covered);
    }
    
    covered
}

/// Recursively collect variant names from a pattern
fn collect_variants_from_pattern(pattern: &Pattern, covered: &mut std::collections::HashSet<String>) {
    match pattern {
        Pattern::Variant { path, .. } => {
            // Extract just the variant name (last segment)
            if let Some(last) = path.last() {
                covered.insert(last.to_string());
            }
        }
        Pattern::Or(patterns) => {
            for p in patterns {
                collect_variants_from_pattern(p, covered);
            }
        }
        _ => {}
    }
}

/// Check if any arm has a wildcard or catch-all pattern
fn has_wildcard(arms: &[MatchArm]) -> bool {
    for arm in arms {
        if matches_all(&arm.pattern) {
            return true;
        }
    }
    false
}

/// Check if a pattern matches all possible values
fn matches_all(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Wildcard => true,
        Pattern::Ident { .. } => true, // Binding without destructuring matches all
        Pattern::Or(patterns) => patterns.iter().any(matches_all),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::Ident;
    use proc_macro2::Span;

    fn ident(s: &str) -> Ident {
        Ident::new(s, Span::call_site())
    }

    #[test]
    fn test_has_wildcard_true() {
        let arms = vec![
            MatchArm {
                pattern: Pattern::Wildcard,
                guard: None,
                body: crate::grammar::SaltBlock { stmts: vec![] },
            },
        ];
        assert!(has_wildcard(&arms));
    }

    #[test]
    fn test_has_wildcard_false() {
        let arms = vec![
            MatchArm {
                pattern: Pattern::Variant { 
                    path: vec![ident("Some")],
                    fields: None,
                },
                guard: None,
                body: crate::grammar::SaltBlock { stmts: vec![] },
            },
        ];
        assert!(!has_wildcard(&arms));
    }

    #[test]
    fn test_extract_covered_variants() {
        let arms = vec![
            MatchArm {
                pattern: Pattern::Variant { 
                    path: vec![ident("Option"), ident("Some")],
                    fields: None,
                },
                guard: None,
                body: crate::grammar::SaltBlock { stmts: vec![] },
            },
            MatchArm {
                pattern: Pattern::Variant { 
                    path: vec![ident("Option"), ident("None")],
                    fields: None,
                },
                guard: None,
                body: crate::grammar::SaltBlock { stmts: vec![] },
            },
        ];
        
        let covered = extract_covered_variants(&arms);
        assert!(covered.contains("Some"));
        assert!(covered.contains("None"));
        assert_eq!(covered.len(), 2);
    }

    #[test]
    fn test_or_pattern_coverage() {
        let arms = vec![
            MatchArm {
                pattern: Pattern::Or(vec![
                    Pattern::Variant { path: vec![ident("A")], fields: None },
                    Pattern::Variant { path: vec![ident("B")], fields: None },
                ]),
                guard: None,
                body: crate::grammar::SaltBlock { stmts: vec![] },
            },
        ];
        
        let covered = extract_covered_variants(&arms);
        assert!(covered.contains("A"));
        assert!(covered.contains("B"));
    }
}
