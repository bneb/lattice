//! Minimal semantic versioning for sp.
//!
//! Provides version parsing, constraint parsing, and matching.
//! Supports operators: >=, <=, >, <, =, ^ (caret), ~ (tilde).

use std::cmp::Ordering;
use std::fmt;

/// A semantic version: major.minor.patch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.major
            .cmp(&other.major)
            .then_with(|| self.minor.cmp(&other.minor))
            .then_with(|| self.patch.cmp(&other.patch))
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Version constraint operator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    Exact,
    Greater,
    GreaterEq,
    Less,
    LessEq,
    Caret,
    Tilde,
}

/// A single version constraint.
#[derive(Debug, Clone)]
pub struct Constraint {
    pub op: Op,
    pub version: Version,
}

impl Constraint {
    fn allows(&self, ver: &Version) -> bool {
        match self.op {
            Op::Exact => ver == &self.version,
            Op::Greater => ver > &self.version,
            Op::GreaterEq => ver >= &self.version,
            Op::Less => ver < &self.version,
            Op::LessEq => ver <= &self.version,
            Op::Caret => caret_allows(&self.version, ver),
            Op::Tilde => tilde_allows(&self.version, ver),
        }
    }
}

fn caret_allows(base: &Version, ver: &Version) -> bool {
    if base.major > 0 {
        // ^1.2.3 means >=1.2.3, <2.0.0
        ver >= base && ver.major == base.major
    } else if base.minor > 0 {
        // ^0.1.2 means >=0.1.2, <0.2.0
        ver.major == 0 && ver.minor == base.minor && ver.patch >= base.patch
    } else {
        // ^0.0.3 means ==0.0.3
        ver == base
    }
}

fn tilde_allows(base: &Version, ver: &Version) -> bool {
    // ~1.2.3 means >=1.2.3, <1.3.0
    ver >= base && ver.major == base.major && ver.minor == base.minor
}

/// Parse a semver version string.
///
/// Accepts "1", "1.2", "1.2.3" — missing components default to 0.
pub fn parse_version(s: &str) -> Result<Version, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty version string".into());
    }
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() > 3 {
        return Err(format!("too many version components: {}", s));
    }
    let major = parts[0]
        .parse()
        .map_err(|e: std::num::ParseIntError| format!("invalid major version '{}': {}", parts[0], e))?;
    let minor = if parts.len() > 1 {
        parts[1]
            .parse()
            .map_err(|e: std::num::ParseIntError| format!("invalid minor version '{}': {}", parts[1], e))?
    } else {
        0
    };
    let patch = if parts.len() > 2 {
        parts[2]
            .parse()
            .map_err(|e: std::num::ParseIntError| format!("invalid patch version '{}': {}", parts[2], e))?
    } else {
        0
    };
    Ok(Version { major, minor, patch })
}

/// Parse a constraint string into a list of constraints.
///
/// Formats:
/// - Comma-separated: `>=1.0,<2.0`
/// - Operators: `>=`, `<=`, `>`, `<`, `=`, `^`, `~`
/// - Bare version like `1.0` is treated as `^1.0`
/// - `*` or empty string matches everything
pub fn parse_constraints(s: &str) -> Result<Vec<Constraint>, String> {
    let s = s.trim();
    if s.is_empty() || s == "*" {
        return Ok(vec![]);
    }
    let mut constraints = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (op, ver_str) = if let Some(rest) = part.strip_prefix(">=") {
            (Op::GreaterEq, rest)
        } else if let Some(rest) = part.strip_prefix("<=") {
            (Op::LessEq, rest)
        } else if let Some(rest) = part.strip_prefix('>') {
            (Op::Greater, rest)
        } else if let Some(rest) = part.strip_prefix('<') {
            (Op::Less, rest)
        } else if let Some(rest) = part.strip_prefix('=') {
            (Op::Exact, rest)
        } else if let Some(rest) = part.strip_prefix('^') {
            (Op::Caret, rest)
        } else if let Some(rest) = part.strip_prefix('~') {
            (Op::Tilde, rest)
        } else {
            (Op::Caret, part)
        };
        let ver_str = ver_str.trim();
        if ver_str.is_empty() {
            return Err(format!("empty version in constraint '{}'", part));
        }
        constraints.push(Constraint {
            op,
            version: parse_version(ver_str)?,
        });
    }
    Ok(constraints)
}

/// Check if a version satisfies all given constraints.
pub fn satisfies(version: &Version, constraints: &[Constraint]) -> bool {
    constraints.iter().all(|c| c.allows(version))
}

/// Find the highest version from a list that satisfies all constraints.
pub fn best_match<'a>(versions: &'a [Version], constraints: &[Constraint]) -> Option<&'a Version> {
    versions.iter().filter(|v| satisfies(v, constraints)).max()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version_full() {
        let v = parse_version("1.2.3").unwrap();
        assert_eq!(v, Version { major: 1, minor: 2, patch: 3 });
    }

    #[test]
    fn test_parse_version_partial() {
        let v = parse_version("1.2").unwrap();
        assert_eq!(v, Version { major: 1, minor: 2, patch: 0 });
        let v = parse_version("1").unwrap();
        assert_eq!(v, Version { major: 1, minor: 0, patch: 0 });
    }

    #[test]
    fn test_parse_version_empty() {
        assert!(parse_version("").is_err());
    }

    #[test]
    fn test_parse_version_too_many_parts() {
        assert!(parse_version("1.2.3.4").is_err());
    }

    #[test]
    fn test_parse_constraints_range() {
        let cs = parse_constraints(">=1.0,<2.0").unwrap();
        assert_eq!(cs.len(), 2);
        assert!(matches!(cs[0].op, Op::GreaterEq));
        assert!(matches!(cs[1].op, Op::Less));
    }

    #[test]
    fn test_parse_constraints_caret() {
        let cs = parse_constraints("^0.1.0").unwrap();
        assert_eq!(cs.len(), 1);
        assert!(matches!(cs[0].op, Op::Caret));
    }

    #[test]
    fn test_parse_constraints_tilde() {
        let cs = parse_constraints("~1.2").unwrap();
        assert_eq!(cs.len(), 1);
        assert!(matches!(cs[0].op, Op::Tilde));
    }

    #[test]
    fn test_parse_constraints_wildcard() {
        let cs = parse_constraints("*").unwrap();
        assert!(cs.is_empty());
    }

    #[test]
    fn test_parse_constraints_empty() {
        let cs = parse_constraints("").unwrap();
        assert!(cs.is_empty());
    }

    #[test]
    fn test_satisfies_exact() {
        let v = parse_version("1.0.0").unwrap();
        let cs = parse_constraints("=1.0").unwrap();
        assert!(satisfies(&v, &cs));
        let v2 = parse_version("1.0.1").unwrap();
        assert!(!satisfies(&v2, &cs));
    }

    #[test]
    fn test_satisfies_range() {
        let v = parse_version("1.5.0").unwrap();
        let cs = parse_constraints(">=1.0,<2.0").unwrap();
        assert!(satisfies(&v, &cs));
    }

    #[test]
    fn test_satisfies_out_of_range() {
        let v = parse_version("2.0.0").unwrap();
        let cs = parse_constraints(">=1.0,<2.0").unwrap();
        assert!(!satisfies(&v, &cs));
    }

    #[test]
    fn test_caret_zero_minor() {
        let cs = parse_constraints("^0.1.0").unwrap();
        assert!(satisfies(&parse_version("0.1.0").unwrap(), &cs));
        assert!(satisfies(&parse_version("0.1.5").unwrap(), &cs));
        assert!(!satisfies(&parse_version("0.2.0").unwrap(), &cs));
        assert!(!satisfies(&parse_version("0.3.0").unwrap(), &cs));
    }

    #[test]
    fn test_caret_major() {
        let v = parse_version("1.5.0").unwrap();
        let cs = parse_constraints("^1.2.0").unwrap();
        assert!(satisfies(&v, &cs));
        let v2 = parse_version("2.0.0").unwrap();
        assert!(!satisfies(&v2, &cs));
    }

    #[test]
    fn test_bare_version_as_caret() {
        let v = parse_version("1.5.0").unwrap();
        let cs = parse_constraints("1.0").unwrap();
        assert_eq!(cs.len(), 1);
        assert!(matches!(cs[0].op, Op::Caret));
        assert!(satisfies(&v, &cs));
    }

    #[test]
    fn test_best_match() {
        let versions = [
            parse_version("0.1.0").unwrap(),
            parse_version("1.0.0").unwrap(),
            parse_version("1.5.0").unwrap(),
            parse_version("2.0.0").unwrap(),
        ];
        let cs = parse_constraints(">=1.0,<2.0").unwrap();
        assert_eq!(best_match(&versions, &cs), Some(&versions[2]));
    }

    #[test]
    fn test_tilde() {
        let v = parse_version("1.2.3").unwrap();
        let cs = parse_constraints("~1.2").unwrap();
        assert!(satisfies(&v, &cs));
        let v2 = parse_version("1.3.0").unwrap();
        assert!(!satisfies(&v2, &cs));
    }

    #[test]
    fn test_version_comparison() {
        let v1 = parse_version("1.0.0").unwrap();
        let v2 = parse_version("2.0.0").unwrap();
        assert!(v1 < v2);
        let v3 = parse_version("1.5.0").unwrap();
        assert!(v1 < v3);
        assert!(v3 < v2);
    }

    #[test]
    fn test_no_match_empty_versions() {
        let cs = parse_constraints(">=1.0").unwrap();
        assert!(best_match(&[], &cs).is_none());
    }
}
