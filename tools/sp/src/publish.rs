//! Package publishing — creates .tar.gz archives for distribution.
//!
//! Published packages are stored locally in ~/.salt/publish/.
//! The resolver extracts them to ~/.salt/packages/<name>-<version>/
//! for use as dependencies.

use crate::manifest::Manifest;
use crate::semver;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Get the publish directory path (~/.salt/publish/).
fn publish_dir() -> Result<PathBuf, String> {
    let home =
        std::env::var("HOME").map_err(|_| "HOME environment variable not set".to_string())?;
    let dir = PathBuf::from(home).join(".salt").join("publish");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("failed to create publish dir: {}", e))?;
    Ok(dir)
}

/// Get the extracted packages cache directory (~/.salt/packages/).
fn packages_dir() -> Result<PathBuf, String> {
    let home =
        std::env::var("HOME").map_err(|_| "HOME environment variable not set".to_string())?;
    Ok(PathBuf::from(home).join(".salt").join("packages"))
}

/// Publish the current project as a .tar.gz archive.
///
/// Output: ~/.salt/publish/<name>-<version>.tar.gz
/// Archive contains: salt.toml, all .salt files from src/
pub fn publish(manifest: &Manifest, project_dir: &Path) -> Result<(), String> {
    let pub_dir = publish_dir()?;
    let archive_name = format!("{}-{}.tar.gz", manifest.package.name, manifest.package.version);
    let archive_path = pub_dir.join(&archive_name);

    // Collect source files
    let mut files = Vec::new();
    collect_source_files(project_dir, &mut files)?;

    // Include salt.toml
    let manifest_path = project_dir.join("salt.toml");
    if manifest_path.exists() {
        files.push(manifest_path);
    }

    // Create tar.gz
    let file =
        std::fs::File::create(&archive_path).map_err(|e| format!("failed to create archive: {}", e))?;
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut tar = tar::Builder::new(encoder);

    // Build file manifest for salt-publish.toml
    let mut manifest_lines = String::new();
    manifest_lines.push_str(&format!(
        "[package]\nname = \"{}\"\nversion = \"{}\"\n\n[files]\n",
        manifest.package.name, manifest.package.version
    ));

    for file_path in &files {
        let relative = file_path
            .strip_prefix(project_dir)
            .map_err(|_| "failed to compute relative path".to_string())?;
        let relative_str = relative.to_string_lossy().to_string();

        // Hash file content
        let content =
            std::fs::read(file_path).map_err(|e| format!("failed to read {}: {}", file_path.display(), e))?;
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let hash = hex::encode(hasher.finalize());

        manifest_lines.push_str(&format!("{} = \"sha256:{}\"\n", relative_str, hash));

        // Add to tar archive
        tar.append_path_with_name(file_path, &relative_str)
            .map_err(|e| format!("failed to add {} to archive: {}", file_path.display(), e))?;
    }

    // Add the manifest file to the archive
    let manifest_bytes = manifest_lines.into_bytes();
    let mut header = tar::Header::new_gnu();
    header.set_size(manifest_bytes.len() as u64);
    header.set_mode(0o644);
    header.set_entry_type(tar::EntryType::Regular);
    header.set_cksum();
    tar.append_data(&mut header, "salt-publish.toml", manifest_bytes.as_slice())
        .map_err(|e| format!("failed to write manifest: {}", e))?;

    tar.finish().map_err(|e| format!("failed to finalize archive: {}", e))?;

    println!(
        "📦 Published \x1b[1m{}\x1b[0m v{} to {}",
        manifest.package.name,
        manifest.package.version,
        archive_path.display()
    );

    Ok(())
}

/// Collect all .salt files under a directory.
fn collect_source_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }

    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("failed to read {}: {}", dir.display(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read error: {}", e))?;
        let path = entry.path();

        if path.is_dir() {
            let name = path.file_name().unwrap().to_string_lossy();
            if !name.starts_with('.') && name != "target" && name != "tests" {
                collect_source_files(&path, files)?;
            }
        } else if path.extension().is_some_and(|e| e == "salt") {
            files.push(path);
        }
    }
    Ok(())
}

/// Find all published versions of a package.
///
/// Returns sorted list of (version, archive_path) for all published
/// versions of the given package name.
pub fn find_published(name: &str) -> Result<Vec<(semver::Version, PathBuf)>, String> {
    let dir = publish_dir()?;
    let mut results = Vec::new();

    if !dir.exists() {
        return Ok(results);
    }

    let entries =
        std::fs::read_dir(&dir).map_err(|e| format!("failed to read publish dir: {}", e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read error: {}", e))?;
        let path = entry.path();

        if path.extension().is_some_and(|e| e == "gz") {
            let filename = path
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.strip_suffix(".tar"))
                .unwrap_or("");

            // Parse "<name>-<version>" from filename
            let prefix = format!("{}-", name);
            if let Some(ver_str) = filename.strip_prefix(&prefix) {
                if let Ok(ver) = semver::parse_version(ver_str) {
                    results.push((ver, path));
                }
            }
        }
    }

    results.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(results)
}

/// Extract a published package to the local packages cache.
///
/// Returns the path to the extracted package directory.
pub fn extract_package(name: &str, version: &str) -> Result<PathBuf, String> {
    let pkg_dir = packages_dir()?.join(format!("{}-{}", name, version));

    // Skip if already extracted
    if pkg_dir.join("salt.toml").exists() {
        return Ok(pkg_dir);
    }

    let pub_dir = publish_dir()?;
    let archive_path = pub_dir.join(format!("{}-{}.tar.gz", name, version));

    if !archive_path.exists() {
        return Err(format!(
            "published package '{}-{}' not found in {}",
            name,
            version,
            pub_dir.display()
        ));
    }

    // Extract
    let file = std::fs::File::open(&archive_path)
        .map_err(|e| format!("failed to open {}: {}", archive_path.display(), e))?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    std::fs::create_dir_all(&pkg_dir)
        .map_err(|e| format!("failed to create package dir: {}", e))?;

    archive
        .unpack(&pkg_dir)
        .map_err(|e| format!("failed to extract {}: {}", archive_path.display(), e))?;

    Ok(pkg_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_collect_source_files_finds_salt_files() {
        let tmp = std::env::temp_dir().join("sp_test_publish_collect");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("src")).unwrap();
        fs::write(tmp.join("src/main.salt"), "package main").unwrap();
        fs::write(tmp.join("src/lib.salt"), "package lib").unwrap();
        fs::write(tmp.join("README.md"), "# readme").unwrap();

        let mut files = Vec::new();
        collect_source_files(&tmp, &mut files).unwrap();

        assert!(files.iter().any(|f| f.ends_with("src/main.salt")));
        assert!(files.iter().any(|f| f.ends_with("src/lib.salt")));
        // README should not be included
        assert!(!files.iter().any(|f| f.ends_with("README.md")));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_find_published_empty_dir() {
        // Should return empty list for non-existent publish dir
        let results = find_published("nonexistent").unwrap_or_default();
        assert!(results.is_empty());
    }
}
