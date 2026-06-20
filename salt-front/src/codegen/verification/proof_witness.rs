//! Proof Witness Module - Actionable Diagnostics for Z3 Verification
//!
//! When Z3 finds a counterexample (SAT), this module:
//! 1. Classifies the failing constraint by pattern (bounds, null, overflow)
//! 2. Extracts relevant variable values from the model
//! 3. Generates actionable hints (requires/assert suggestions)
//!
//! ## The "Helpful Shadow" Philosophy
//! Instead of just saying "No", we tell the developer exactly what
//! mathematical property the compiler needs to be satisfied.

use std::fmt;

/// A structured suggestion for resolving a verification failure
#[derive(Debug, Clone)]
pub enum ProofHint {
    /// Add a precondition to the function signature
    /// e.g., "add 'requires idx < len' to the signature"
    AddRequires(String),
    
    /// Add a local assertion before the operation
    /// e.g., "add 'assert ptr != null' before this line"
    AddAssert(String),
    
    /// Specific bounds check hint with index and length names
    AddBoundsCheck { index: String, bound: String },
    
    /// Suggest a safer type or operation
    NarrowType(String),
    
    /// Generic hint for edge cases
    Note(String),
}

impl fmt::Display for ProofHint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProofHint::AddRequires(cond) => 
                write!(f, "add 'requires {}' to the function signature", cond),
            ProofHint::AddAssert(cond) => 
                write!(f, "add 'assert {}' before this line", cond),
            ProofHint::AddBoundsCheck { index, bound } => 
                write!(f, "add 'requires {} < {}' OR 'assert {} < {}'", index, bound, index, bound),
            ProofHint::NarrowType(suggestion) => 
                write!(f, "{}", suggestion),
            ProofHint::Note(msg) => 
                write!(f, "{}", msg),
        }
    }
}

/// Structured verification failure with context and hints
#[derive(Debug)]
pub struct VerificationFailure {
    /// The constraint that couldn't be proven
    pub constraint: String,
    /// The context where the failure occurred (e.g., "array index access")
    pub context: String,
    /// Actionable suggestions for the developer
    pub hints: Vec<ProofHint>,
    /// Optional counterexample values from Z3
    pub counterexample: Option<Counterexample>,
}

/// A counterexample from Z3 showing values that violate the constraint
#[derive(Debug, Clone)]
pub struct Counterexample {
    /// Variable name -> value pairs
    pub values: Vec<(String, i64)>,
}

impl VerificationFailure {
    /// Create a new verification failure with default hints based on constraint pattern
    pub fn new(constraint: String, context: String) -> Self {
        let hints = classify_constraint(&constraint);
        Self {
            constraint,
            context,
            hints,
            counterexample: None,
        }
    }
    
    /// Create with an explicit counterexample from Z3
    pub fn with_counterexample(
        constraint: String, 
        context: String, 
        values: Vec<(String, i64)>
    ) -> Self {
        let hints = classify_constraint(&constraint);
        Self {
            constraint,
            context,
            hints,
            counterexample: Some(Counterexample { values }),
        }
    }
    
    /// Format as a compiler error message
    pub fn format_error(&self) -> String {
        let mut msg = format!("VERIFICATION ERROR: could not prove '{}'\n", self.constraint);
        msg.push_str(&format!("  context: {}\n", self.context));
        
        if let Some(ref ce) = self.counterexample {
            msg.push_str("  counterexample:\n");
            for (name, val) in &ce.values {
                msg.push_str(&format!("    {} = {}\n", name, val));
            }
        }
        
        if !self.hints.is_empty() {
            msg.push('\n');
            for hint in &self.hints {
                msg.push_str(&format!("  = hint: {}\n", hint));
            }
        }
        
        msg
    }
}

impl fmt::Display for VerificationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_error())
    }
}

/// Classify a constraint string and generate appropriate hints
fn classify_constraint(constraint: &str) -> Vec<ProofHint> {
    let mut hints = Vec::new();
    
    // Pattern: Bounds check (idx < len, i < capacity, etc.)
    if constraint.contains(" < ") {
        let parts: Vec<&str> = constraint.split(" < ").collect();
        if parts.len() == 2 {
            let index = parts[0].trim().to_string();
            let bound = parts[1].trim().to_string();
            hints.push(ProofHint::AddBoundsCheck { 
                index: index.clone(), 
                bound: bound.clone() 
            });
            hints.push(ProofHint::Note(format!(
                "The caller must guarantee {} is within bounds before calling this function",
                index
            )));
        }
    }
    
    // Pattern: Null check (ptr != null, x != 0)
    if constraint.contains("!= null") || constraint.contains("!= 0") {
        let var_name = constraint.split("!=").next()
            .map(|s| s.trim())
            .unwrap_or("ptr");
        hints.push(ProofHint::AddRequires(format!("{} != null", var_name)));
        hints.push(ProofHint::AddAssert(format!("{} != null", var_name)));
    }
    
    // Pattern: Capacity/length relationship
    if constraint.contains("capacity") || constraint.contains("len") {
        hints.push(ProofHint::Note(
            "Ensure the container has sufficient capacity before the operation".to_string()
        ));
    }
    
    // Pattern: Overflow concerns (addition, multiplication)
    if constraint.contains("overflow") || constraint.contains("+ ") && constraint.contains("<=") {
        hints.push(ProofHint::NarrowType(
            "consider using checked_add() or widening to i128 to prevent overflow".to_string()
        ));
    }
    
    // Default hint if no pattern matched
    if hints.is_empty() {
        hints.push(ProofHint::AddRequires(constraint.to_string()));
        hints.push(ProofHint::AddAssert(constraint.to_string()));
    }
    
    hints
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // ========================================================================
    // ProofHint Display Tests - Cover all enum variants
    // ========================================================================
    
    #[test]
    fn test_proof_hint_display_add_requires() {
        let hint = ProofHint::AddRequires("x < 10".to_string());
        let output = format!("{}", hint);
        assert!(output.contains("requires"));
        assert!(output.contains("x < 10"));
        assert!(output.contains("function signature"));
    }
    
    #[test]
    fn test_proof_hint_display_add_assert() {
        let hint = ProofHint::AddAssert("ptr != null".to_string());
        let output = format!("{}", hint);
        assert!(output.contains("assert"));
        assert!(output.contains("ptr != null"));
        assert!(output.contains("before this line"));
    }
    
    #[test]
    fn test_proof_hint_display_add_bounds_check() {
        let hint = ProofHint::AddBoundsCheck { 
            index: "i".to_string(), 
            bound: "len".to_string() 
        };
        let output = format!("{}", hint);
        assert!(output.contains("i < len"));
        assert!(output.contains("OR"));
    }
    
    #[test]
    fn test_proof_hint_display_narrow_type() {
        let hint = ProofHint::NarrowType("use u32 instead".to_string());
        let output = format!("{}", hint);
        assert_eq!(output, "use u32 instead");
    }
    
    #[test]
    fn test_proof_hint_display_note() {
        let hint = ProofHint::Note("important info".to_string());
        let output = format!("{}", hint);
        assert_eq!(output, "important info");
    }
    
    // ========================================================================
    // classify_constraint Tests - All pattern branches
    // ========================================================================
    
    #[test]
    fn test_bounds_check_classification() {
        let hints = classify_constraint("idx < len");
        assert!(hints.iter().any(|h| matches!(h, ProofHint::AddBoundsCheck { .. })));
        // Also should have Note about caller guarantee
        assert!(hints.iter().any(|h| matches!(h, ProofHint::Note(_))));
    }
    
    #[test]
    fn test_bounds_check_with_whitespace() {
        let hints = classify_constraint("  i  <  capacity  ");
        assert!(hints.iter().any(|h| {
            if let ProofHint::AddBoundsCheck { index, bound } = h {
                index == "i" && bound == "capacity"
            } else {
                false
            }
        }));
    }
    
    #[test]
    fn test_null_check_classification() {
        let hints = classify_constraint("ptr != null");
        assert!(hints.iter().any(|h| matches!(h, ProofHint::AddRequires(_))));
        assert!(hints.iter().any(|h| matches!(h, ProofHint::AddAssert(_))));
    }
    
    #[test]
    fn test_null_check_with_zero() {
        let hints = classify_constraint("value != 0");
        // Should match != 0 pattern
        assert!(hints.iter().any(|h| {
            if let ProofHint::AddRequires(cond) = h {
                cond.contains("value")
            } else {
                false
            }
        }));
    }
    
    #[test]
    fn test_capacity_pattern() {
        let hints = classify_constraint("new_size <= capacity");
        assert!(hints.iter().any(|h| {
            if let ProofHint::Note(msg) = h {
                msg.contains("capacity")
            } else {
                false
            }
        }));
    }
    
    #[test]
    fn test_len_pattern() {
        let hints = classify_constraint("idx < self.len()");
        assert!(hints.iter().any(|h| {
            if let ProofHint::Note(msg) = h {
                msg.contains("container") || msg.contains("capacity")
            } else {
                false
            }
        }));
    }
    
    #[test]
    fn test_overflow_pattern_explicit() {
        let hints = classify_constraint("a + b may overflow");
        assert!(hints.iter().any(|h| {
            if let ProofHint::NarrowType(msg) = h {
                msg.contains("checked_add") || msg.contains("i128")
            } else {
                false
            }
        }));
    }
    
    #[test]
    fn test_overflow_pattern_addition_with_bound() {
        let hints = classify_constraint("x + y <= MAX");
        assert!(hints.iter().any(|h| matches!(h, ProofHint::NarrowType(_))));
    }
    
    #[test]
    fn test_default_fallback_for_unknown_pattern() {
        // A constraint that doesn't match any known pattern
        let hints = classify_constraint("some_weird_condition");
        assert!(hints.iter().any(|h| matches!(h, ProofHint::AddRequires(_))));
        assert!(hints.iter().any(|h| matches!(h, ProofHint::AddAssert(_))));
    }
    
    #[test]
    fn test_single_less_than_without_space_no_match() {
        // "a<b" should NOT match " < " pattern (requires spaces)
        let hints = classify_constraint("a<b");
        // Should fall through to default
        assert!(hints.iter().any(|h| {
            if let ProofHint::AddRequires(cond) = h {
                cond == "a<b"
            } else {
                false
            }
        }));
    }
    
    // ========================================================================
    // VerificationFailure Tests
    // ========================================================================
    
    #[test]
    fn test_verification_failure_new() {
        let failure = VerificationFailure::new(
            "idx < capacity".to_string(),
            "array index in Vec::get".to_string(),
        );
        assert_eq!(failure.constraint, "idx < capacity");
        assert_eq!(failure.context, "array index in Vec::get");
        assert!(!failure.hints.is_empty());
        assert!(failure.counterexample.is_none());
    }
    
    #[test]
    fn test_verification_failure_display() {
        let failure = VerificationFailure::new(
            "idx < capacity".to_string(),
            "array index in Vec::get".to_string(),
        );
        let output = failure.format_error();
        assert!(output.contains("could not prove"));
        assert!(output.contains("idx < capacity"));
        assert!(output.contains("context:"));
        assert!(output.contains("hint:"));
    }
    
    #[test]
    fn test_verification_failure_display_trait() {
        let failure = VerificationFailure::new(
            "x > 0".to_string(),
            "division".to_string(),
        );
        let display_output = format!("{}", failure);
        let format_output = failure.format_error();
        assert_eq!(display_output, format_output);
    }
    
    #[test]
    fn test_counterexample_display() {
        let failure = VerificationFailure::with_counterexample(
            "x < 10".to_string(),
            "bounds check".to_string(),
            vec![("x".to_string(), 15)],
        );
        let output = failure.format_error();
        assert!(output.contains("counterexample"));
        assert!(output.contains("x = 15"));
    }
    
    #[test]
    fn test_counterexample_multiple_values() {
        let failure = VerificationFailure::with_counterexample(
            "a + b < max".to_string(),
            "overflow check".to_string(),
            vec![
                ("a".to_string(), 100),
                ("b".to_string(), 200),
                ("max".to_string(), 255),
            ],
        );
        let output = failure.format_error();
        assert!(output.contains("a = 100"));
        assert!(output.contains("b = 200"));
        assert!(output.contains("max = 255"));
    }
    
    #[test]
    fn test_counterexample_empty_values() {
        let failure = VerificationFailure::with_counterexample(
            "always_false".to_string(),
            "logic error".to_string(),
            vec![],
        );
        let output = failure.format_error();
        assert!(output.contains("counterexample"));
        // Should still format correctly even with no values
    }
    
    #[test]
    fn test_verification_failure_no_hints() {
        // Create manually to test empty hints branch
        let failure = VerificationFailure {
            constraint: "unknown".to_string(),
            context: "test".to_string(),
            hints: vec![],
            counterexample: None,
        };
        let output = failure.format_error();
        assert!(output.contains("could not prove"));
        // Should NOT contain hint section
        assert!(!output.contains("= hint:"));
    }
    
    // ========================================================================
    // Edge Cases and Bug Hunting
    // ========================================================================
    
    #[test]
    fn test_complex_constraint_with_multiple_patterns() {
        // Constraint that matches BOTH bounds AND capacity patterns
        let hints = classify_constraint("idx < capacity");
        
        // Should have bounds check hint
        assert!(hints.iter().any(|h| matches!(h, ProofHint::AddBoundsCheck { .. })));
        
        // Should ALSO have capacity note
        assert!(hints.iter().any(|h| {
            if let ProofHint::Note(msg) = h {
                msg.contains("capacity")
            } else {
                false
            }
        }));
    }
    
    #[test]
    fn test_constraint_with_all_patterns() {
        // Edge case: constraint matches multiple patterns
        let hints = classify_constraint("ptr != null && idx < len");
        // Should have null check hints
        assert!(hints.iter().any(|h| matches!(h, ProofHint::AddRequires(_))));
        // Should also have bounds hints from len pattern
        assert!(hints.iter().any(|h| matches!(h, ProofHint::Note(_))));
    }
    
    #[test]
    fn test_negative_counterexample_values() {
        let failure = VerificationFailure::with_counterexample(
            "x >= 0".to_string(),
            "non-negative check".to_string(),
            vec![("x".to_string(), -42)],
        );
        let output = failure.format_error();
        assert!(output.contains("x = -42"));
    }
    
    #[test]
    fn test_special_characters_in_constraint() {
        let hints = classify_constraint("arr[i] < MAX_SIZE");
        // Should fall through to default since pattern doesn't match cleanly
        assert!(!hints.is_empty());
    }
    
    #[test]
    fn test_unicode_in_constraint() {
        let hints = classify_constraint("δ < ε");
        assert!(hints.iter().any(|h| matches!(h, ProofHint::AddBoundsCheck { .. })));
    }
    
    // ========================================================================
    // Edge Cases for Full Branch Coverage
    // ========================================================================
    
    #[test]
    fn test_multiple_less_than_splits_to_default() {
        // "a < b < c" splits into 3 parts, so parts.len() != 2
        // Should NOT match bounds check pattern, falls to default
        let hints = classify_constraint("a < b < c");
        // The constraint contains " < " so it enters the if block,
        // but parts.len() == 3, so it skips AddBoundsCheck
        // However it still matches default because hints remains empty from that block
        assert!(!hints.is_empty());
    }
    
    #[test]
    fn test_addition_without_bound_check_no_overflow_hint() {
        // "x + y" has addition but no "<=" so should NOT match overflow pattern  
        let hints = classify_constraint("x + y");
        // Should fall through to default
        assert!(hints.iter().any(|h| {
            if let ProofHint::AddRequires(cond) = h {
                cond == "x + y"
            } else {
                false
            }
        }));
        // Should NOT have overflow hint
        assert!(!hints.iter().any(|h| matches!(h, ProofHint::NarrowType(_))));
    }
    
    #[test]
    fn test_just_overflow_keyword() {
        // "may overflow" should match just the "overflow" check
        let hints = classify_constraint("integer may overflow");
        assert!(hints.iter().any(|h| matches!(h, ProofHint::NarrowType(_))));
    }
    
    #[test]
    fn test_empty_constraint() {
        // Edge case: empty constraint
        let hints = classify_constraint("");
        // Should fall through to default
        assert!(hints.iter().any(|h| matches!(h, ProofHint::AddRequires(_))));
    }
    
    #[test]
    fn test_constraint_is_just_less_than() {
        // Edge case: constraint is just " < "
        let hints = classify_constraint(" < ");
        // Split produces ["", ""], parts.len() == 2
        // index and bound would be empty strings
        assert!(hints.iter().any(|h| matches!(h, ProofHint::AddBoundsCheck { .. })));
    }
}
