//! Salt Manifest Parser — reads and validates salt.toml
//!
//! Supports the full sp manifest specification:
//!   [package]      — name, version, edition, entry, description, license, repository
//!   [dependencies] — path deps, version deps, git deps, features
//!   [dev-dependencies]
//!   [build]        — target, release/debug profiles
//!   [workspace]    — members, shared dependencies

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Top-level salt.toml manifest structure.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Manifest {
    pub package: Package,
    #[serde(default)]
    pub dependencies: HashMap<String, Dependency>,
    #[serde(default, rename = "dev-dependencies")]
    pub dev_dependencies: HashMap<String, Dependency>,
    #[serde(default)]
    pub build: Option<BuildConfig>,
    #[serde(default)]
    pub workspace: Option<Workspace>,
}

/// Package metadata.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Package {
    pub name: String,
    pub version: String,
    #[serde(default = "default_edition")]
    pub edition: String,
    /// Entry point (relative to project dir)
    #[serde(default = "default_entry")]
    pub entry: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub repository: Option<String>,
}

fn default_entry() -> String {
    "src/main.salt".to_string()
}

fn default_edition() -> String {
    "2026".to_string()
}

/// A dependency specification — supports multiple source types.
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum Dependency {
    /// Inline table: { path = "../foo" }
    Path { path: String },
    /// Inline table with git source: { git = "...", rev = "..." }
    Git {
        git: String,
        #[serde(default)]
        rev: Option<String>,
        #[serde(default)]
        branch: Option<String>,
        #[serde(default)]
        tag: Option<String>,
    },
    /// Inline table with version + features: { version = "1.0", features = ["x"] }
    Full {
        version: String,
        #[serde(default)]
        features: Vec<String>,
    },
    /// Simple version string: "0.2"
    Version(String),
}

impl Dependency {
    /// Get the local filesystem path for a path dependency.
    pub fn local_path(&self) -> Option<&str> {
        match self {
            Dependency::Path { path } => Some(path),
            _ => None,
        }
    }

    /// Get the version string, if any.
    pub fn version(&self) -> Option<&str> {
        match self {
            Dependency::Version(v) => Some(v),
            Dependency::Full { version, .. } => Some(version),
            _ => None,
        }
    }

    /// Human-readable source description.
    pub fn source_display(&self) -> String {
        match self {
            Dependency::Path { path } => format!("path:{}", path),
            Dependency::Git { git, rev, .. } => {
                if let Some(r) = rev {
                    format!("git+{}?rev={}", git, &r[..7.min(r.len())])
                } else {
                    format!("git+{}", git)
                }
            }
            Dependency::Version(v) => format!("v{}", v),
            Dependency::Full { version, .. } => format!("v{}", version),
        }
    }
}

/// Build configuration profiles.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct BuildConfig {
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub release: Option<ProfileConfig>,
    #[serde(default)]
    pub debug: Option<ProfileConfig>,
}

/// Per-profile build settings.
#[derive(Debug, Deserialize)]
pub struct ProfileConfig {
    #[serde(default)]
    pub opt: Option<String>,
    #[serde(default)]
    pub lto: Option<bool>,
    #[serde(default)]
    pub verify: Option<bool>,
}

/// Workspace configuration.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Workspace {
    #[serde(default)]
    pub members: Vec<String>,
    #[serde(default)]
    pub dependencies: HashMap<String, Dependency>,
}

/// Load and parse a salt.toml manifest file.
pub fn load(path: &Path) -> Result<Manifest, String> {
    if !path.exists() {
        return Err(format!(
            "no salt.toml found at {}. Run `sp new <name>` to create a project.",
            path.display()
        ));
    }

    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;

    let manifest: Manifest = toml::from_str(&content)
        .map_err(|e| format!("failed to parse salt.toml: {}", e))?;

    // Validate
    if manifest.package.name.is_empty() {
        return Err("package.name cannot be empty".to_string());
    }

    if manifest.package.version.is_empty() {
        return Err("package.version cannot be empty".to_string());
    }

    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_manifest() {
        let toml = r#"
[package]
name = "my_app"
version = "0.1.0"
"#;
        let manifest: Manifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.package.name, "my_app");
        assert_eq!(manifest.package.version, "0.1.0");
        assert_eq!(manifest.package.edition, "2026");
        assert_eq!(manifest.package.entry, "src/main.salt");
    }

    #[test]
    fn test_parse_full_manifest() {
        let toml = r#"
[package]
name = "lettuce"
version = "0.3.0"
edition = "2026"
description = "Redis-compatible in-memory data store"
license = "MIT"
entry = "src/server.salt"

[dependencies]
http = "0.2"
json = { version = "1.0", features = ["streaming"] }
crypto = { git = "https://github.com/nicebyte/salt-crypto", rev = "a1b2c3d" }
my_lib = { path = "../my_lib" }

[dev-dependencies]
bench = "0.1"

[build]
target = "aarch64-apple-darwin"

[workspace]
members = ["lettuce", "kernel"]
"#;
        let manifest: Manifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.package.name, "lettuce");
        assert_eq!(manifest.dependencies.len(), 4);
        assert_eq!(manifest.dev_dependencies.len(), 1);

        // Test path dep
        let my_lib = manifest.dependencies.get("my_lib").unwrap();
        assert_eq!(my_lib.local_path(), Some("../my_lib"));

        // Test version dep
        let http = manifest.dependencies.get("http").unwrap();
        assert_eq!(http.version(), Some("0.2"));

        // Test workspace
        assert!(manifest.workspace.is_some());
        assert_eq!(manifest.workspace.as_ref().unwrap().members.len(), 2);
    }

    #[test]
    fn test_parse_custom_entry() {
        let toml = r#"
[package]
name = "lib"
version = "1.0.0"
entry = "lib.salt"
"#;
        let manifest: Manifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.package.entry, "lib.salt");
    }

    #[test]
    fn test_dependency_source_display() {
        let path_dep = Dependency::Path {
            path: "../foo".to_string(),
        };
        assert_eq!(path_dep.source_display(), "path:../foo");

        let ver_dep = Dependency::Version("0.2".to_string());
        assert_eq!(ver_dep.source_display(), "v0.2");

        let git_dep = Dependency::Git {
            git: "https://example.com/repo".to_string(),
            rev: Some("a1b2c3d4e5".to_string()),
            branch: None,
            tag: None,
        };
        assert_eq!(
            git_dep.source_display(),
            "git+https://example.com/repo?rev=a1b2c3d"
        );
    }
}
