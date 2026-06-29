//! Lockfile generation for reproducible builds.
//!
//! The salt.lock file records exact versions and content hashes for
//! all packages in the dependency tree.

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A lockfile entry for a single package.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LockedPackage {
    pub version: String,
    /// Content hash in format "sha256:<hex>"
    pub hash: String,
    #[serde(default)]
    pub deps: Vec<String>,
}

/// The complete lockfile.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Lockfile {
    pub packages: HashMap<String, LockedPackage>,
}

impl Lockfile {
    /// Create an empty lockfile.
    pub fn new() -> Self {
        Self {
            packages: HashMap::new(),
        }
    }

    /// Load a lockfile from disk.
    #[allow(dead_code)]
    pub fn load(path: &Path) -> Result<Self, String> {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("failed to read lockfile: {}", e))?;
        toml::from_str(&content).map_err(|e| format!("failed to parse lockfile: {}", e))
    }

    /// Save the lockfile to disk.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create lockfile dir: {}", e))?;
        }
        let content = toml::to_string_pretty(self)
            .map_err(|e| format!("failed to serialize lockfile: {}", e))?;
        std::fs::write(path, &content).map_err(|e| format!("failed to write lockfile: {}", e))?;
        Ok(())
    }

    /// Insert or update a package entry.
    pub fn add_package(&mut self, name: &str, version: &str, hash: &str, deps: Vec<String>) {
        self.packages.insert(
            name.to_string(),
            LockedPackage {
                version: version.to_string(),
                hash: hash.to_string(),
                deps,
            },
        );
    }
}

/// Compute the content hash (SHA-256) for a package's source files.
///
/// Hashes all .salt files in the project's source directory, sorted by path
/// for determinism. Returns "sha256:<hex>".
pub fn compute_content_hash(project_dir: &Path) -> Result<String, String> {
    let mut hasher = Sha256::new();

    let src_dir = project_dir.join("src");
    if src_dir.exists() {
        hash_source_files(&mut hasher, &src_dir)?;
    } else {
        // Hash the entry file directly
        let entry = project_dir.join("src/main.salt");
        if entry.exists() {
            let content =
                std::fs::read(&entry).map_err(|e| format!("failed to read {}: {}", entry.display(), e))?;
            hasher.update(&content);
        }
    }

    Ok(format!("sha256:{}", hex::encode(hasher.finalize())))
}

/// Recursively hash all .salt files in a directory, sorted for determinism.
fn hash_source_files(hasher: &mut Sha256, dir: &Path) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }

    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("failed to read {}: {}", dir.display(), e))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();

    entries.sort();

    for entry in entries {
        if entry.is_dir() {
            let name = entry.file_name().unwrap().to_string_lossy();
            if !name.starts_with('.') && name != "target" {
                hash_source_files(hasher, &entry)?;
            }
        } else if entry.extension().is_some_and(|e| e == "salt") {
            hasher.update(entry.file_name().unwrap().to_string_lossy().as_bytes());
            let content =
                std::fs::read(&entry).map_err(|e| format!("failed to read {}: {}", entry.display(), e))?;
            hasher.update(&content);
        }
    }

    Ok(())
}

/// Generate a lockfile for a project and its dependencies.
pub fn generate(
    manifest: &crate::manifest::Manifest,
    project_dir: &Path,
    resolved_deps: &[crate::resolver::ResolvedDep],
) -> Result<Lockfile, String> {
    let mut lockfile = Lockfile::new();

    // Main package
    let main_hash = compute_content_hash(project_dir)?;
    let dep_names: Vec<String> = manifest.dependencies.keys().cloned().collect();
    lockfile.add_package(
        &manifest.package.name,
        &manifest.package.version,
        &main_hash,
        dep_names,
    );

    // Each resolved dependency
    for dep in resolved_deps {
        let dep_hash = compute_content_hash(&dep.root_path)?;

        // Get sub-deps from the dep's manifest
        let dep_manifest_path = dep.root_path.join("salt.toml");
        let sub_deps = if dep_manifest_path.exists() {
            if let Ok(sub_manifest) = crate::manifest::load(&dep_manifest_path) {
                sub_manifest.dependencies.keys().cloned().collect()
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        let version = dep
            .resolved_version
            .clone()
            .unwrap_or_else(|| "0.0.0".to_string());
        lockfile.add_package(&dep.name, &version, &dep_hash, sub_deps);
    }

    Ok(lockfile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_lockfile_roundtrip() {
        let tmp = std::env::temp_dir().join("sp_test_lockfile");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let mut lf = Lockfile::new();
        lf.add_package("test", "0.1.0", "sha256:abc123", vec![]);

        let lf_path = tmp.join("salt.lock");
        lf.save(&lf_path).unwrap();

        let loaded = Lockfile::load(&lf_path).unwrap();
        let pkg = loaded.packages.get("test").unwrap();
        assert_eq!(pkg.version, "0.1.0");
        assert_eq!(pkg.hash, "sha256:abc123");

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_content_hash_stable() {
        let tmp = std::env::temp_dir().join("sp_test_hash");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("src")).unwrap();
        fs::write(tmp.join("src/main.salt"), "package main\nfn main() {}").unwrap();

        let hash1 = compute_content_hash(&tmp).unwrap();
        let hash2 = compute_content_hash(&tmp).unwrap();
        assert_eq!(hash1, hash2);
        assert!(hash1.starts_with("sha256:"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_content_hash_differs() {
        let tmp1 = std::env::temp_dir().join("sp_test_hash1");
        let tmp2 = std::env::temp_dir().join("sp_test_hash2");
        let _ = fs::remove_dir_all(&tmp1);
        let _ = fs::remove_dir_all(&tmp2);
        fs::create_dir_all(tmp1.join("src")).unwrap();
        fs::create_dir_all(tmp2.join("src")).unwrap();
        fs::write(tmp1.join("src/main.salt"), "package main\nfn main() { return 0; }").unwrap();
        fs::write(tmp2.join("src/main.salt"), "package main\nfn main() { return 1; }").unwrap();

        let hash1 = compute_content_hash(&tmp1).unwrap();
        let hash2 = compute_content_hash(&tmp2).unwrap();
        assert_ne!(hash1, hash2);

        let _ = fs::remove_dir_all(&tmp1);
        let _ = fs::remove_dir_all(&tmp2);
    }
}
