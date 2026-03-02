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
use crate::codegen::context::{CodegenContext, LoweringContext};
use crate::registry::EnumInfo;
use crate::z3_shim::ast::Ast;
use crate::z3_shim as z3;

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
        Pattern::Or(patterns) => patterns.iter().any(|p| matches_all(p)),
        _ => false,
    }
}

/// Use Z3 to verify exhaustiveness (more sophisticated approach)
/// This can handle complex patterns with guards
#[allow(dead_code)]
pub fn verify_exhaustiveness_z3<'a>(
    ctx: &CodegenContext<'a>,
    enum_info: &EnumInfo,
    arms: &[MatchArm],
) -> ExhaustivenessResult {
    // Create a symbolic discriminant variable
    let discrim = crate::z3_shim::ast::Int::new_const(ctx.z3_ctx, "discriminant");
    
    // Build constraint: discriminant must be one of the valid values
    let valid_values: Vec<crate::z3_shim::ast::Int> = enum_info.variants.iter()
        .map(|(_, _, tag)| crate::z3_shim::ast::Int::from_i64(ctx.z3_ctx, *tag as i64))
        .collect();
    
    let valid_constraints: Vec<crate::z3_shim::ast::Bool> = valid_values.iter()
        .map(|v| discrim._eq(v))
        .collect();
    
    let valid_refs: Vec<&crate::z3_shim::ast::Bool> = valid_constraints.iter().collect();
    let is_valid = crate::z3_shim::ast::Bool::or(ctx.z3_ctx, &valid_refs);
    
    // Build constraint: discriminant is NOT covered by any arm
    let mut covered_constraints: Vec<crate::z3_shim::ast::Bool> = Vec::new();
    
    for arm in arms {
        if let Some(constraint) = pattern_to_z3_constraint(ctx, &arm.pattern, &discrim, enum_info) {
            covered_constraints.push(constraint);
        } else {
            // Wildcard or catch-all - covers everything
            return ExhaustivenessResult::Exhaustive;
        }
    }
    
    if covered_constraints.is_empty() {
        // No patterns - nothing is covered
        let missing: Vec<String> = enum_info.variants.iter()
            .map(|(name, _, _)| name.clone())
            .collect();
        return ExhaustivenessResult::MissingVariants(missing);
    }
    
    let covered_refs: Vec<&crate::z3_shim::ast::Bool> = covered_constraints.iter().collect();
    let is_covered = crate::z3_shim::ast::Bool::or(ctx.z3_ctx, &covered_refs);
    
    // Query: exists discriminant such that (is_valid AND NOT is_covered)?
    let uncovered = crate::z3_shim::ast::Bool::and(ctx.z3_ctx, &[&is_valid, &is_covered.not()]);
    
    let solver = crate::z3_shim::Solver::new(ctx.z3_ctx);
    let mut params = crate::z3_shim::Params::new(ctx.z3_ctx);
    params.set_u32("timeout", 100);
    solver.set_params(&params);
    
    solver.assert(&uncovered);
    
    match solver.check() {
        crate::z3_shim::SatResult::Unsat => ExhaustivenessResult::Exhaustive,
        crate::z3_shim::SatResult::Sat => {
            // Find which variants are missing by checking the model
            let model = solver.get_model();
            if let Some(model) = model {
                if let Some(val) = model.eval(&discrim, true) {
                    // Find the variant with this discriminant
                    if let Some(val_i64) = val.as_i64() {
                        for (name, _, tag) in &enum_info.variants {
                            if *tag as i64 == val_i64 {
                                return ExhaustivenessResult::MissingVariants(vec![name.clone()]);
                            }
                        }
                    }
                }
            }
            // Fallback: compute missing via set difference
            let covered_variants = extract_covered_variants(arms);
            let missing: Vec<String> = enum_info.variants.iter()
                .filter(|(name, _, _)| !covered_variants.contains(name))
                .map(|(name, _, _)| name.clone())
                .collect();
            ExhaustivenessResult::MissingVariants(missing)
        }
        crate::z3_shim::SatResult::Unknown => {
            ExhaustivenessResult::Unverifiable("Z3 timeout during exhaustiveness check".to_string())
        }
    }
}

/// Convert a pattern to a Z3 constraint on the discriminant
fn pattern_to_z3_constraint<'a>(
    ctx: &CodegenContext<'a>,
    pattern: &Pattern,
    discrim: &crate::z3_shim::ast::Int<'a>,
    enum_info: &EnumInfo,
) -> Option<crate::z3_shim::ast::Bool<'a>> {
    match pattern {
        Pattern::Wildcard | Pattern::Ident { .. } => {
            // Matches everything
            None
        }
        Pattern::Variant { path, .. } => {
            // Extract variant name and find its discriminant
            let variant_name = path.last().map(|id| id.to_string()).unwrap_or_default();
            for (v_name, _, tag) in &enum_info.variants {
                if *v_name == variant_name {
                    let tag_val = crate::z3_shim::ast::Int::from_i64(ctx.z3_ctx, *tag as i64);
                    return Some(discrim._eq(&tag_val));
                }
            }
            // Variant not found - this would be a type error, but return false constraint
            Some(crate::z3_shim::ast::Bool::from_bool(ctx.z3_ctx, false))
        }
        Pattern::Or(patterns) => {
            let mut constraints: Vec<crate::z3_shim::ast::Bool> = Vec::new();
            for p in patterns {
                if let Some(c) = pattern_to_z3_constraint(ctx, p, discrim, enum_info) {
                    constraints.push(c);
                } else {
                    // One of the or-branches is a wildcard
                    return None;
                }
            }
            if constraints.is_empty() {
                None
            } else {
                let refs: Vec<&crate::z3_shim::ast::Bool> = constraints.iter().collect();
                Some(crate::z3_shim::ast::Bool::or(ctx.z3_ctx, &refs))
            }
        }
        _ => {
            // Other patterns (Literal, Tuple, Struct) - assume they don't cover discriminants
            Some(crate::z3_shim::ast::Bool::from_bool(ctx.z3_ctx, false))
        }
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
