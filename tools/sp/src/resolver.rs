//! Dependency Resolver — resolves the dependency graph and constructs search roots
//!
//! For Phase 1, this handles local path dependencies. Future phases will add
//! PubGrub version solving and registry resolution.

use crate::manifest::{Manifest, Dependency};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Resolved dependency information.
#[derive(Debug)]
pub struct ResolvedDep {
    pub name: String,
    pub source: String,
    pub root_path: PathBuf,
}

/// Resolve the build order and search roots for a project.
///
/// Returns:
///   - build_order: list of .salt files in compilation order (deps first)
///   - search_roots: list of paths for the compiler's `--roots` flag
pub fn resolve(
    manifest: &Manifest,
    project_dir: &Path,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>), String> {
    let mut build_order = Vec::new();
    let mut search_roots = Vec::new();
    let mut resolved_names = HashSet::new();

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

    Ok((build_order, search_roots))
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
            });

            Ok(result)
        }

        Dependency::Version(ver) => {
            // Phase 1: registry deps are not yet supported
            Err(format!(
                "registry dependencies not yet supported ({}@{}). Use path dependencies for now.",
                name, ver
            ))
        }

        Dependency::Full { version, .. } => {
            Err(format!(
                "registry dependencies not yet supported ({}@{}). Use path dependencies for now.",
                name, version
            ))
        }

        Dependency::Git { git, .. } => {
            Err(format!(
                "git dependencies not yet supported ({}@{}). Use path dependencies for now.",
                name, git
            ))
        }
    }
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

    // Fallback: hardcoded keuos path
    let fallback = PathBuf::from("/Users/kevin/projects/keuos/salt-front/std");
    if fallback.exists() {
        return Some(fallback);
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
        } else if path.extension().map_or(false, |e| e == "salt") {
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
        let (build_order, search_roots) = resolve(&manifest, &tmp).unwrap();

        assert_eq!(build_order.len(), 1, "should have 1 source file");
        assert!(!search_roots.is_empty(), "should have at least 1 search root");

        let _ = fs::remove_dir_all(&tmp);
    }
}
