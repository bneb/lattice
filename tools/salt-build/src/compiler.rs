//! Compiler Orchestration — drives salt-front → MLIR → LLVM pipeline
//!
//! Delegates to the existing `scripts/run_test.sh` pipeline for each file,
//! which handles MLIR-to-LLVM compilation and linking with the runtime.

use crate::manifest::Manifest;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Compile a single Salt module through the full pipeline.
pub fn compile_module(module: &Path, project_dir: &Path, release: bool) -> Result<(), String> {
    // Use the existing run_test.sh script in compile-only mode
    let script = find_build_script(project_dir)?;

    let mut cmd = Command::new(&script);
    cmd.arg(module);
    cmd.arg("--compile-only");

    if release {
        cmd.env("SALT_RELEASE", "1");
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run {}: {}", script.display(), e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "Compilation failed for {}:\n{}\n{}",
            module.display(),
            stdout,
            stderr
        ));
    }

    Ok(())
}

/// Link compiled object files into a final binary.
pub fn link(manifest: &Manifest, project_dir: &Path, release: bool) -> Result<PathBuf, String> {
    let output = output_path(manifest, project_dir, release);

    // The run_test.sh script handles linking automatically.
    // For multi-module projects, we compile the entry point which pulls in deps.
    let entry = project_dir.join(&manifest.package.entry);
    let script = find_build_script(project_dir)?;

    let mut cmd = Command::new(&script);
    cmd.arg(&entry);

    let cmd_output = cmd
        .output()
        .map_err(|e| format!("Failed to link: {}", e))?;

    if !cmd_output.status.success() {
        let stderr = String::from_utf8_lossy(&cmd_output.stderr);
        return Err(format!("Linking failed:\n{}", stderr));
    }

    // The build script produces the binary in /tmp/salt_build/
    let built_name = entry
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let built_path = PathBuf::from(format!("/tmp/salt_build/{}", built_name));

    if built_path.exists() {
        // Copy to the project output directory
        let output_dir = output.parent().unwrap();
        std::fs::create_dir_all(output_dir)
            .map_err(|e| format!("Failed to create output dir: {}", e))?;
        std::fs::copy(&built_path, &output)
            .map_err(|e| format!("Failed to copy binary: {}", e))?;
    }

    Ok(output)
}

/// Run a test file through the pipeline.
pub fn run_test(test_file: &Path, project_dir: &Path) -> Result<(), String> {
    let script = find_build_script(project_dir)?;

    let output = Command::new(&script)
        .arg(test_file)
        .output()
        .map_err(|e| format!("Failed to run test: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!("{}\n{}", stdout.trim(), stderr.trim()));
    }

    Ok(())
}

/// Get the output binary path for a project.
pub fn output_path(manifest: &Manifest, project_dir: &Path, release: bool) -> PathBuf {
    let target_dir = if release { "target/release" } else { "target/debug" };
    project_dir
        .join(target_dir)
        .join(&manifest.package.name)
}

/// Find the run_test.sh script by searching upward from the project directory.
fn find_build_script(project_dir: &Path) -> Result<PathBuf, String> {
    let mut dir = project_dir.to_path_buf();

    // Try the project dir first, then walk up
    loop {
        let candidate = dir.join("scripts/run_test.sh");
        if candidate.exists() {
            return Ok(candidate);
        }

        // Also check for the script relative to the keuos root
        let candidate2 = dir.join("run_test.sh");
        if candidate2.exists() {
            return Ok(candidate2);
        }

        if !dir.pop() {
            break;
        }
    }

    // Fallback: check SALT_REPO_ROOT environment variable
    if let Ok(repo_root) = std::env::var("SALT_REPO_ROOT") {
        let env_script = PathBuf::from(&repo_root).join("scripts/run_test.sh");
        if env_script.exists() {
            return Ok(env_script);
        }
    }

    Err("Could not find scripts/run_test.sh. Set SALT_REPO_ROOT or run from within a Salt project.".to_string())
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
                entry: "src/main.salt".to_string(),
            },
            dependencies: Default::default(),
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
                entry: "src/main.salt".to_string(),
            },
            dependencies: Default::default(),
        };

        let path = output_path(&manifest, Path::new("/tmp/project"), true);
        assert_eq!(path, PathBuf::from("/tmp/project/target/release/my_app"));
    }
}
