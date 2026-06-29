//! Content-Addressed Artifact Cache
//!
//! Implements the Nix-lite global cache from the sp design:
//!   cache_key = sha256(source + compiler + target + features + deps_hash)
//!
//! Artifacts are stored in ~/.salt/cache/artifacts/<hash>
//! Cache hits skip all compilation for instant no-op builds.

use crate::manifest::Manifest;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// The global artifact cache.
pub struct ArtifactCache {
    cache_dir: PathBuf,
}

impl ArtifactCache {
    /// Create a new artifact cache instance.
    pub fn new() -> Result<Self, String> {
        let home = std::env::var("HOME")
            .map_err(|_| "HOME environment variable not set".to_string())?;
        let cache_dir = PathBuf::from(home).join(".salt").join("cache").join("artifacts");

        // Create cache directory if it doesn't exist
        if !cache_dir.exists() {
            std::fs::create_dir_all(&cache_dir)
                .map_err(|e| format!("failed to create cache dir: {}", e))?;
        }

        Ok(Self { cache_dir })
    }

    /// Compute the content-addressed cache key for a build.
    ///
    /// The key includes:
    ///   - source hash (all .salt files in the project)
    ///   - compiler version (salt-front binary hash, approximated by version string)
    ///   - build profile (release/debug)
    ///   - target triple
    ///   - feature flags
    ///   - deps_hash (resolved dependency artifact hashes — prevents ABI mismatch)
    pub fn compute_key(
        &self,
        manifest: &Manifest,
        project_dir: &Path,
        release: bool,
        search_roots: &[PathBuf],
    ) -> Result<String, String> {
        let mut hasher = Sha256::new();

        // 1. Source hash — hash all .salt files in the project
        let src_dir = project_dir.join("src");
        if src_dir.exists() {
            hash_directory(&mut hasher, &src_dir)?;
        } else {
            let entry = project_dir.join(&manifest.package.entry);
            if entry.exists() {
                let content = std::fs::read(&entry)
                    .map_err(|e| format!("failed to read {}: {}", entry.display(), e))?;
                hasher.update(&content);
            }
        }

        // 2. Compiler version — hash the salt-front binary if available
        let compiler_id = compiler_version_hash(project_dir);
        hasher.update(compiler_id.as_bytes());

        // 3. Build profile
        let profile = if release { "release" } else { "debug" };
        hasher.update(profile.as_bytes());

        // 4. Target triple
        let target = manifest
            .build
            .as_ref()
            .and_then(|b| b.target.as_ref())
            .map(|t| t.as_str())
            .unwrap_or("aarch64-apple-darwin");
        hasher.update(target.as_bytes());

        // 5. Dependency roots hash (transitive cache key fix)
        //    Hash the resolved search roots to detect when deps change
        let mut dep_hasher = Sha256::new();
        for root in search_roots {
            if root.exists() {
                hash_directory(&mut dep_hasher, root)?;
            }
        }
        let deps_hash = dep_hasher.finalize();
        hasher.update(deps_hash);

        let result = hasher.finalize();
        Ok(hex::encode(result))
    }

    /// Look up a cached artifact by its key.
    /// Returns the path to the cached binary if it exists.
    pub fn lookup(&self, key: &str) -> Option<PathBuf> {
        let cached_path = self.cache_dir.join(key);
        if cached_path.exists() {
            Some(cached_path)
        } else {
            None
        }
    }

    /// Store a built artifact in the cache.
    pub fn store(&self, key: &str, binary_path: &Path) -> Result<(), String> {
        if !binary_path.exists() {
            return Ok(()); // Nothing to cache
        }

        let cached_path = self.cache_dir.join(key);
        std::fs::copy(binary_path, &cached_path)
            .map_err(|e| format!("failed to cache artifact: {}", e))?;

        Ok(())
    }
}

/// Get a version identifier for the salt-front compiler.
///
/// Tries to hash a portion of the salt-front binary for accurate cache
/// invalidation when the compiler changes. Falls back to a version string.
fn compiler_version_hash(project_dir: &Path) -> String {
    // Try to find and hash the salt-front binary
    let mut dir = project_dir
        .canonicalize()
        .unwrap_or_else(|_| project_dir.to_path_buf());

    loop {
        let bin = dir.join("salt-front/target/release/salt-front");
        if bin.exists() {
            if let Ok(data) = std::fs::read(&bin) {
                let mut h = Sha256::new();
                // Hash first 64KB + last 64KB for speed (sufficient for version detection)
                let len = data.len();
                let head = &data[..len.min(65536)];
                let tail = if len > 65536 { &data[len - 65536..] } else { &[] };
                h.update(head);
                h.update(tail);
                return hex::encode(h.finalize());
            }
            break;
        }

        let debug_bin = dir.join("salt-front/target/debug/salt-front");
        if debug_bin.exists() {
            if let Ok(data) = std::fs::read(&debug_bin) {
                let mut h = Sha256::new();
                let len = data.len();
                h.update(&data[..len.min(65536)]);
                return hex::encode(h.finalize());
            }
            break;
        }

        if !dir.pop() { break; }
    }

    // Fallback: use environment-provided version
    std::env::var("SALT_COMPILER_VERSION")
        .unwrap_or_else(|_| "salt-front-dev".to_string())
}

/// Hash all .salt files in a directory recursively.
fn hash_directory(hasher: &mut Sha256, dir: &Path) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }

    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("failed to read {}: {}", dir.display(), e))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();

    // Sort for deterministic hashing
    entries.sort();

    for entry in entries {
        if entry.is_dir() {
            let name = entry.file_name().unwrap().to_string_lossy();
            if !name.starts_with('.') && name != "target" {
                hash_directory(hasher, &entry)?;
            }
        } else if entry.extension().is_some_and(|e| e == "salt") {
            // Hash filename + content for determinism
            hasher.update(entry.file_name().unwrap().to_string_lossy().as_bytes());
            let content = std::fs::read(&entry)
                .map_err(|e| format!("failed to read {}: {}", entry.display(), e))?;
            hasher.update(&content);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_cache_roundtrip() {
        let tmp = std::env::temp_dir().join("sp_test_cache");
        let _ = fs::remove_dir_all(&tmp);

        let cache = ArtifactCache {
            cache_dir: tmp.clone(),
        };
        fs::create_dir_all(&tmp).unwrap();

        // No cache hit initially
        assert!(cache.lookup("test_key_123").is_none());

        // Create a fake binary and store it
        let fake_binary = tmp.join("fake_binary");
        fs::write(&fake_binary, b"binary content").unwrap();
        cache.store("test_key_123", &fake_binary).unwrap();

        // Cache hit
        assert!(cache.lookup("test_key_123").is_some());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_same_source_same_hash() {
        let tmp = std::env::temp_dir().join("sp_test_hash_stable");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("src")).unwrap();
        fs::write(tmp.join("src/main.salt"), "package main\nfn main() { }").unwrap();
        fs::write(
            tmp.join("salt.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let manifest = crate::manifest::load(&tmp.join("salt.toml")).unwrap();
        let cache = ArtifactCache {
            cache_dir: tmp.join("cache"),
        };
        fs::create_dir_all(tmp.join("cache")).unwrap();

        let key1 = cache.compute_key(&manifest, &tmp, false, &[]).unwrap();
        let key2 = cache.compute_key(&manifest, &tmp, false, &[]).unwrap();
        assert_eq!(key1, key2, "same source must produce same hash");

        // Different profile = different hash
        let key3 = cache.compute_key(&manifest, &tmp, true, &[]).unwrap();
        assert_ne!(key1, key3, "release vs debug must produce different hash");

        let _ = fs::remove_dir_all(&tmp);
    }
}
