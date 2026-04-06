//! Compiler Orchestration — drives the salt-front → MLIR → LLVM pipeline
//!
//! sp acts as a thin orchestration layer over the existing compilation
//! infrastructure. It constructs search roots from resolved dependencies
//! and invokes salt-front with the right flags.

use crate::manifest::Manifest;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Compile a Salt project through the full pipeline.
///
/// This invokes the existing `scripts/run_test.sh` script with the entry
/// point, passing resolved search roots so the compiler can find dependencies.
pub fn build(
    manifest: &Manifest,
    project_dir: &Path,
    release: bool,
    search_roots: &[PathBuf],
) -> Result<PathBuf, String> {
    let entry = project_dir.join(&manifest.package.entry);
    if !entry.exists() {
        return Err(format!(
            "entry point not found: {}",
            entry.display()
        ));
    }

    let script = find_build_script(project_dir)?;

    let mut cmd = Command::new(&script);
    cmd.arg(&entry);

    if release {
        cmd.env("SALT_RELEASE", "1");
    }

    // Pass search roots as extra include paths
    // The run_test.sh script forwards these to salt-front
    if !search_roots.is_empty() {
        let roots_str: Vec<String> = search_roots
            .iter()
            .map(|r| r.to_string_lossy().to_string())
            .collect();
        cmd.env("SALT_SEARCH_ROOTS", roots_str.join(":"));
    }

    let output = cmd
        .output()
        .map_err(|e| format!("failed to run {}: {}", script.display(), e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Extract the most useful error lines
        let error_lines: Vec<&str> = stderr
            .lines()
            .chain(stdout.lines())
            .filter(|l| {
                l.contains("error") || l.contains("Error") || l.contains("FAIL")
                    || l.contains("undefined") || l.contains("cannot find")
            })
            .take(10)
            .collect();

        let error_msg = if error_lines.is_empty() {
            format!("{}\n{}", stdout.trim(), stderr.trim())
        } else {
            error_lines.join("\n")
        };

        return Err(format!("compilation failed:\n{}", error_msg));
    }

    // The build script produces the binary in /tmp/salt_build/
    let binary_name = entry
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let built_path = PathBuf::from(format!("/tmp/salt_build/{}", binary_name));

    // Copy to the project output directory
    let out = output_path(manifest, project_dir, release);
    let output_dir = out.parent().unwrap();
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("failed to create output dir: {}", e))?;

    if built_path.exists() {
        std::fs::copy(&built_path, &out)
            .map_err(|e| format!("failed to copy binary: {}", e))?;
    }

    Ok(out)
}

/// Compile and run a test file.
pub fn run_test(test_file: &Path, project_dir: &Path) -> Result<(), String> {
    let script = find_build_script(project_dir)?;

    let output = Command::new(&script)
        .arg(test_file)
        .output()
        .map_err(|e| format!("failed to run test: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!("{}\n{}", stdout.trim(), stderr.trim()));
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
        return Err(format!(
            "entry point not found: {}",
            entry.display()
        ));
    }

    // Find the salt-front binary directly for contract checking
    let salt_front = find_salt_front(project_dir)?;

    let mut cmd = Command::new(&salt_front);
    cmd.arg(&entry);
    cmd.arg("--verify");

    // Add search roots  
    if !search_roots.is_empty() {
        // salt-front doesn't have --roots yet, but we set CWD context
        // so module_loader can find deps
        for root in search_roots {
            if root.exists() {
                cmd.env("SALT_INCLUDE", root.to_string_lossy().to_string());
            }
        }
    }

    let output = cmd
        .output()
        .map_err(|e| format!("failed to run salt-front: {}", e))?;

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

/// Find the run_test.sh script by searching upward from the project directory.
fn find_build_script(project_dir: &Path) -> Result<PathBuf, String> {
    let mut dir = project_dir
        .canonicalize()
        .unwrap_or_else(|_| project_dir.to_path_buf());

    loop {
        let candidate = dir.join("scripts/run_test.sh");
        if candidate.exists() {
            return Ok(candidate);
        }

        if !dir.pop() {
            break;
        }
    }

    // Fallback: keuos project structure
    let keuos_script = PathBuf::from("/Users/kevin/projects/keuos/scripts/run_test.sh");
    if keuos_script.exists() {
        return Ok(keuos_script);
    }

    Err("could not find scripts/run_test.sh — are you in a Salt project?".to_string())
}

/// Find the salt-front binary.
fn find_salt_front(project_dir: &Path) -> Result<PathBuf, String> {
    let mut dir = project_dir
        .canonicalize()
        .unwrap_or_else(|_| project_dir.to_path_buf());

    loop {
        // Check for debug build
        let candidate = dir.join("salt-front/target/debug/salt-front");
        if candidate.exists() {
            return Ok(candidate);
        }

        // Check for release build
        let candidate_rel = dir.join("salt-front/target/release/salt-front");
        if candidate_rel.exists() {
            return Ok(candidate_rel);
        }

        if !dir.pop() {
            break;
        }
    }

    Err("could not find salt-front binary — build it with: cd salt-front && cargo build".to_string())
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
