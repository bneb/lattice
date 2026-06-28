//! Compiler Orchestration — drives salt-front directly
//!
//! sp invokes salt-front directly rather than shelling out to run_test.sh.
//! This gives us control over flags, caching, and error formatting.

use crate::manifest::Manifest;
use std::path::{Path, PathBuf};
use std::process::Command;

fn extract_error_message(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let error_lines: Vec<&str> = stderr.lines().chain(stdout.lines())
        .filter(|l| l.contains("error") || l.contains("Error")
            || l.contains("FAIL") || l.contains("undefined")
            || l.contains("cannot find"))
        .take(10).collect();
    if error_lines.is_empty() {
        format!("{}\n{}", stdout.trim(), stderr.trim())
    } else {
        error_lines.join("\n")
    }
}

/// Compile a Salt project by invoking salt-front directly.
pub fn build(
    manifest: &Manifest,
    project_dir: &Path,
    release: bool,
    search_roots: &[PathBuf],
) -> Result<PathBuf, String> {
    let entry = project_dir.join(&manifest.package.entry);
    if !entry.exists() {
        return Err(format!("entry point not found: {}", entry.display()));
    }

    let salt_front = find_salt_front(project_dir)?;
    let mut cmd = Command::new(&salt_front);
    cmd.arg(&entry);

    // Output path
    let out = output_path(manifest, project_dir, release);
    let output_dir = out.parent().unwrap();
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("failed to create output dir: {}", e))?;
    cmd.arg("-o");
    cmd.arg(&out);

    if release {
        cmd.arg("--release");
    }

    // Pass search roots for dependency resolution
    if !search_roots.is_empty() {
        let roots_str: Vec<String> = search_roots
            .iter()
            .map(|r| r.to_string_lossy().to_string())
            .collect();
        cmd.env("SALT_SEARCH_ROOTS", roots_str.join(":"));
    }

    let output = cmd.output()
        .map_err(|e| format!("failed to run {}: {}", salt_front.display(), e))?;

    if !output.status.success() {
        return Err(format!("compilation failed:\n{}", extract_error_message(&output)));
    }

    Ok(out)
}

/// Compile and run a test file directly via salt-front.
pub fn run_test(test_file: &Path, project_dir: &Path) -> Result<(), String> {
    let salt_front = find_salt_front(project_dir)?;
    let output_bin = std::env::temp_dir().join(
        test_file.file_stem().unwrap().to_string_lossy().to_string()
    );

    let output = Command::new(&salt_front)
        .arg(test_file)
        .arg("-o")
        .arg(&output_bin)
        .output()
        .map_err(|e| format!("failed to run test: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!("{}\n{}", stdout.trim(), stderr.trim()));
    }

    // Run the compiled test binary
    if output_bin.exists() {
        let run = Command::new(&output_bin)
            .output()
            .map_err(|e| format!("failed to execute test: {}", e))?;
        if !run.status.success() {
            return Err(format!("test failed:\n{}",
                String::from_utf8_lossy(&run.stderr)));
        }
    }

    Ok(())
}

/// Verify contracts without producing a binary.
pub fn check(
    manifest: &Manifest,
    project_dir: &Path,
    search_roots: &[PathBuf],
) -> Result<(), String> {
    let entry = project_dir.join(&manifest.package.entry);
    if !entry.exists() {
        return Err(format!("entry point not found: {}", entry.display()));
    }

    let salt_front = find_salt_front(project_dir)?;
    let mut cmd = Command::new(&salt_front);
    cmd.arg(&entry);
    cmd.arg("--lib");
    cmd.arg("--disable-alias-scopes");

    if !search_roots.is_empty() {
        for root in search_roots {
            if root.exists() {
                cmd.env("SALT_INCLUDE", root.to_string_lossy().to_string());
            }
        }
    }

    let output = cmd.output()
        .map_err(|e| format!("failed to run saltc: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("verification failed:\n{}", stderr.trim()));
    }

    Ok(())
}

/// Get the output binary path for a project.
pub fn output_path(manifest: &Manifest, project_dir: &Path, release: bool) -> PathBuf {
    let target_dir = if release { "target/release" } else { "target/debug" };
    project_dir.join(target_dir).join(&manifest.package.name)
}

/// Find the saltc binary by searching upward from the project directory.
pub fn find_salt_front(project_dir: &Path) -> Result<PathBuf, String> {
    let mut dir = project_dir
        .canonicalize()
        .unwrap_or_else(|_| project_dir.to_path_buf());

    loop {
        for name in &["saltc", "salt-front/target/release/saltc", "salt-front/target/debug/saltc"] {
            let candidate = dir.join(name);
            if candidate.exists() && candidate.is_file() {
                return Ok(candidate);
            }
        }

        if !dir.pop() { break; }
    }

    // Check SALT_REPO_ROOT environment variable
    if let Ok(repo_root) = std::env::var("SALT_REPO_ROOT") {
        let env_bin = PathBuf::from(&repo_root)
            .join("salt-front/target/release/saltc");
        if env_bin.exists() {
            return Ok(env_bin);
        }
    }

    Err("could not find saltc binary — build it with: cd salt-front && cargo build --release\n\
         Or set SALT_REPO_ROOT to the repository root.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_path_debug() {
        let manifest = Manifest {
            package: crate::manifest::Package {
                name: "my_app".to_string(),
                version: "0.1.0".to_string(),
                edition: "2026".to_string(),
                entry: "src/main.salt".to_string(),
                description: None,
                license: None,
                repository: None,
            },
            dependencies: Default::default(),
            dev_dependencies: Default::default(),
            build: None,
            workspace: None,
        };
        let path = output_path(&manifest, Path::new("/tmp/project"), false);
        assert_eq!(path, PathBuf::from("/tmp/project/target/debug/my_app"));
    }

    #[test]
    fn test_output_path_release() {
        let manifest = Manifest {
            package: crate::manifest::Package {
                name: "my_app".to_string(),
                version: "0.1.0".to_string(),
                edition: "2026".to_string(),
                entry: "src/main.salt".to_string(),
                description: None,
                license: None,
                repository: None,
            },
            dependencies: Default::default(),
            dev_dependencies: Default::default(),
            build: None,
            workspace: None,
        };
        let path = output_path(&manifest, Path::new("/tmp/project"), true);
        assert_eq!(path, PathBuf::from("/tmp/project/target/release/my_app"));
    }
}
