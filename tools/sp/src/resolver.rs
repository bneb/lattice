//! Dependency Resolver — resolves the dependency graph and constructs search roots
//!
//! Handles path dependencies and version dependencies resolved from the
//! local publish directory (~/.salt/publish/).

use crate::manifest::{Manifest, Dependency};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use sha2::{Digest, Sha256};

/// Resolved dependency information.
#[derive(Debug)]
#[allow(dead_code)]
pub struct ResolvedDep {
    pub name: String,
    pub source: String,
    pub root_path: PathBuf,
    /// The pinned version after resolution (None for path deps).
    pub resolved_version: Option<String>,
}

/// Resolve the build order and search roots for a project.
///
/// Returns:
///   - build_order: list of .salt files in compilation order (deps first)
///   - search_roots: list of paths for the compiler's `--roots` flag
///   - resolved_deps: resolved dependency metadata (for lockfile generation)
pub fn resolve(
    manifest: &Manifest,
    project_dir: &Path,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>, Vec<ResolvedDep>), String> {
    let mut build_order = Vec::new();
    let mut search_roots = Vec::new();
    let mut resolved_names = HashSet::new();
    let mut resolved_deps = Vec::new();

    // Resolve each dependency
    for (dep_name, dep) in &manifest.dependencies {
        let resolved = resolve_single(dep_name, dep, project_dir, &mut resolved_names)?;

        for r in &resolved {
            // Add the dep's src/ directory (or root) as a search root
            let src_dir = r.root_path.join("src");
            if src_dir.exists() {
                search_roots.push(src_dir);
            } else {
                search_roots.push(r.root_path.clone());
            }

            // Collect .salt files from the dependency
            let dep_files = collect_salt_files(&r.root_path)?;
            build_order.extend(dep_files);
        }

        // Track resolved dep metadata for lockfile
        resolved_deps.extend(resolved);
    }

    // Add the project's own source
    let src_dir = project_dir.join("src");
    if src_dir.exists() {
        search_roots.insert(0, src_dir.clone());
        let src_files = collect_salt_files(&src_dir)?;
        build_order.extend(src_files);
    } else {
        search_roots.insert(0, project_dir.to_path_buf());
        let entry = project_dir.join(&manifest.package.entry);
        if entry.exists() {
            build_order.push(entry);
        } else {
            return Err(format!(
                "entry point not found: {}",
                manifest.package.entry
            ));
        }
    }

    // Also add the stdlib root — search upward for the std/ symlink or directory
    if let Some(std_root) = find_stdlib(project_dir) {
        search_roots.push(std_root);
    }

    // Deduplicate while preserving order
    let mut seen = HashSet::new();
    build_order.retain(|f| {
        let canonical = f.canonicalize().unwrap_or_else(|_| f.clone());
        seen.insert(canonical)
    });

    Ok((build_order, search_roots, resolved_deps))
}

/// Resolve a single dependency.
fn resolve_single(
    name: &str,
    dep: &Dependency,
    project_dir: &Path,
    resolved: &mut HashSet<String>,
) -> Result<Vec<ResolvedDep>, String> {
    if resolved.contains(name) {
        return Ok(vec![]); // Already resolved
    }
    resolved.insert(name.to_string());

    match dep {
        Dependency::Path { path } => {
            let dep_dir = project_dir.join(path);
            if !dep_dir.exists() {
                return Err(format!(
                    "dependency '{}' path not found: {}",
                    name,
                    dep_dir.display()
                ));
            }

            let dep_manifest_path = dep_dir.join("salt.toml");
            let mut result = vec![];

            // If the dependency has its own salt.toml, recursively resolve its deps
            if dep_manifest_path.exists() {
                let dep_manifest = crate::manifest::load(&dep_manifest_path)?;

                // Recurse into transitive dependencies
                for (trans_name, trans_dep) in &dep_manifest.dependencies {
                    let trans_resolved = resolve_single(trans_name, trans_dep, &dep_dir, resolved)?;
                    result.extend(trans_resolved);
                }
            }

            result.push(ResolvedDep {
                name: name.to_string(),
                source: format!("path:{}", path),
                root_path: dep_dir
                    .canonicalize()
                    .unwrap_or(dep_dir),
                resolved_version: None,
            });

            Ok(result)
        }

        Dependency::Version(ver) => {
            resolve_version_dep(name, ver, resolved)
        }

        Dependency::Full { version, features: _ } => {
            resolve_version_dep(name, version, resolved)
        }

        Dependency::Git { git, rev, branch, tag } => {
            resolve_git_dep(name, git, rev.as_deref(), branch.as_deref(), tag.as_deref(), resolved)
        }
    }
}

/// Resolve a git dependency by cloning/fetching it into ~/.salt/git.
fn resolve_git_dep(
    name: &str,
    git_url: &str,
    rev: Option<&str>,
    branch: Option<&str>,
    tag: Option<&str>,
    resolved: &mut HashSet<String>,
) -> Result<Vec<ResolvedDep>, String> {
    if resolved.contains(name) {
        return Ok(vec![]);
    }
    resolved.insert(name.to_string());

    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    
    // Create a deterministic directory name for this repo
    let mut hasher = Sha256::new();
    hasher.update(git_url.as_bytes());
    let url_hash = hex::encode(hasher.finalize());
    let git_dir = PathBuf::from(home)
        .join(".salt")
        .join("git")
        .join(format!("{}-{}", name, &url_hash[..8]));

    if !git_dir.exists() {
        std::fs::create_dir_all(git_dir.parent().unwrap()).map_err(|e| e.to_string())?;
        // Clone
        let status = Command::new("git")
            .arg("clone")
            .arg(git_url)
            .arg(&git_dir)
            .status()
            .map_err(|e| format!("failed to run git clone: {}", e))?;
        
        if !status.success() {
            return Err(format!("git clone failed for '{}'", git_url));
        }
    } else {
        // Fetch updates
        let status = Command::new("git")
            .current_dir(&git_dir)
            .arg("fetch")
            .arg("--all")
            .arg("--tags")
            .status()
            .map_err(|e| format!("failed to run git fetch: {}", e))?;
            
        if !status.success() {
            return Err(format!("git fetch failed in '{}'", git_dir.display()));
        }
    }

    // Checkout the target
    let target = rev.or(tag).or(branch);
    if let Some(t) = target {
        let status = Command::new("git")
            .current_dir(&git_dir)
            .arg("checkout")
            .arg(t)
            .status()
            .map_err(|e| format!("failed to run git checkout: {}", e))?;
            
        if !status.success() {
            return Err(format!("git checkout '{}' failed in '{}'", t, git_dir.display()));
        }
    } else {
        let _ = Command::new("git")
            .current_dir(&git_dir)
            .arg("pull")
            .status();
    }

    // Get the resolved commit SHA for the lockfile
    let output = Command::new("git")
        .current_dir(&git_dir)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .map_err(|e| format!("failed to run git rev-parse: {}", e))?;
    
    let commit_sha = String::from_utf8_lossy(&output.stdout).trim().to_string();

    let dep_manifest_path = git_dir.join("salt.toml");
    let mut result = vec![];

    if dep_manifest_path.exists() {
        if let Ok(dep_manifest) = crate::manifest::load(&dep_manifest_path) {
            for (trans_name, trans_dep) in &dep_manifest.dependencies {
                let trans_resolved = resolve_single(trans_name, trans_dep, &git_dir, resolved)?;
                result.extend(trans_resolved);
            }
        }
    }

    let source_str = if let Some(r) = rev {
        format!("git+{}?rev={}", git_url, r)
    } else if let Some(t) = tag {
        format!("git+{}?tag={}", git_url, t)
    } else if let Some(b) = branch {
        format!("git+{}?branch={}", git_url, b)
    } else {
        format!("git+{}", git_url)
    };

    result.push(ResolvedDep {
        name: name.to_string(),
        source: source_str,
        root_path: git_dir.canonicalize().unwrap_or(git_dir),
        resolved_version: Some(commit_sha),
    });

    Ok(result)
}

/// Resolve a version-constrained dependency from the local publish directory.
///
/// Finds all published versions of `name`, parses `constraint_str` as semver
/// constraints, picks the highest matching version, extracts it to the
/// local packages cache, and recurses into transitive dependencies.
fn resolve_version_dep(
    name: &str,
    constraint_str: &str,
    resolved: &mut HashSet<String>,
) -> Result<Vec<ResolvedDep>, String> {
    if resolved.contains(name) {
        return Ok(vec![]);
    }
    resolved.insert(name.to_string());

    // Parse constraints
    let constraints =
        crate::semver::parse_constraints(constraint_str)
            .map_err(|e| format!("invalid version constraint for '{}': {}", name, e))?;

    // Find published versions
    let published = crate::publish::find_published(name)?;
    if published.is_empty() {
        return Err(format!(
            "no published versions found for '{}'.\n  \
             Hint: publish the package first with `sp publish` from its directory.",
            name
        ));
    }

    // Pick the best matching version
    let versions: Vec<crate::semver::Version> = published.iter().map(|(v, _)| v.clone()).collect();
    let best = crate::semver::best_match(&versions, &constraints)
        .ok_or_else(|| {
            format!(
                "no published version of '{}' matches constraints '{}'.\n  \
                 Available versions: {}",
                name,
                constraint_str,
                versions
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?
        .clone();

    // Extract the package
    let dep_dir = crate::publish::extract_package(name, &best.to_string())?;

    // Load its manifest and recurse
    let dep_manifest_path = dep_dir.join("salt.toml");
    let mut result = vec![];

    if dep_manifest_path.exists() {
        if let Ok(dep_manifest) = crate::manifest::load(&dep_manifest_path) {
            for (trans_name, trans_dep) in &dep_manifest.dependencies {
                let trans_resolved = resolve_single(trans_name, trans_dep, &dep_dir, resolved)?;
                result.extend(trans_resolved);
            }
        }
    }

    let version_str = best.to_string();
    result.push(ResolvedDep {
        name: name.to_string(),
        source: format!("v{}", version_str),
        root_path: dep_dir.canonicalize().unwrap_or(dep_dir),
        resolved_version: Some(version_str),
    });

    Ok(result)
}

/// Find the Salt stdlib by searching upward from the project directory.
fn find_stdlib(project_dir: &Path) -> Option<PathBuf> {
    let mut dir = project_dir
        .canonicalize()
        .unwrap_or_else(|_| project_dir.to_path_buf());

    loop {
        // Check for salt-front/std/ (the canonical stdlib location)
        let std_candidate = dir.join("salt-front").join("std");
        if std_candidate.exists() {
            return Some(std_candidate);
        }

        // Check for a std/ symlink or directory
        let std_direct = dir.join("std");
        if std_direct.exists() {
            return Some(std_direct);
        }

        if !dir.pop() {
            break;
        }
    }

    // Fallback: check SALT_REPO_ROOT environment variable
    if let Ok(repo_root) = std::env::var("SALT_REPO_ROOT") {
        let env_stdlib = PathBuf::from(&repo_root).join("salt-front").join("std");
        if env_stdlib.exists() {
            return Some(env_stdlib);
        }
    }

    None
}

/// Collect all .salt files in a directory recursively.
fn collect_salt_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();

    if !dir.exists() {
        return Ok(files);
    }

    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("failed to read {}: {}", dir.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("read error: {}", e))?;
        let path = entry.path();

        if path.is_dir() {
            // Skip target/ and hidden directories
            let name = path.file_name().unwrap().to_string_lossy();
            if name.starts_with('.') || name == "target" || name == "tests" {
                continue;
            }
            files.extend(collect_salt_files(&path)?);
        } else if path.extension().is_some_and(|e| e == "salt") {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_collect_salt_files() {
        let tmp = std::env::temp_dir().join("sp_test_collect");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("src")).unwrap();
        fs::write(tmp.join("src/main.salt"), "package main").unwrap();
        fs::write(tmp.join("src/lib.salt"), "package lib").unwrap();
        fs::write(tmp.join("src/readme.md"), "# readme").unwrap();

        let files = collect_salt_files(&tmp.join("src")).unwrap();
        assert_eq!(files.len(), 2, "should find exactly 2 .salt files");
        assert!(files.iter().all(|f| f.extension().unwrap() == "salt"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_resolve_simple_project() {
        let tmp = std::env::temp_dir().join("sp_test_resolve");
        let _ = fs::remove_dir_all(&tmp);

        // Create a simple project
        fs::create_dir_all(tmp.join("src")).unwrap();
        fs::write(
            tmp.join("salt.toml"),
            r#"
[package]
name = "test_app"
version = "0.1.0"
"#,
        )
        .unwrap();
        fs::write(tmp.join("src/main.salt"), "package main\nfn main() -> i32 { return 0; }").unwrap();

        let manifest = crate::manifest::load(&tmp.join("salt.toml")).unwrap();
        let (build_order, search_roots, _resolved_deps) = resolve(&manifest, &tmp).unwrap();

        assert_eq!(build_order.len(), 1, "should have 1 source file");
        assert!(!search_roots.is_empty(), "should have at least 1 search root");

        let _ = fs::remove_dir_all(&tmp);
    }
}
