use std::path::Path;

use saltc::preprocess;
use saltc::grammar::SaltFile;

pub mod format;
pub mod spacing;
pub mod post;

/// Run the `fmt` subcommand.
/// Expected args: ["fmt", "<path>", "--check"?]
pub fn run_fmt(args: &[String]) -> anyhow::Result<()> {
    let mut path_str: Option<String> = None;
    let mut check = false;

    let mut i = 1; // skip "fmt"
    while i < args.len() {
        match args[i].as_str() {
            "--check" => check = true,
            p => path_str = Some(p.to_string()),
        }
        i += 1;
    }

    let path_str = path_str.ok_or_else(|| {
        anyhow::anyhow!(
            "Usage: saltc fmt <path> [--check]\n  <path> can be a .salt file or a directory"
        )
    })?;
    let path = Path::new(&path_str);

    if path.is_dir() {
        format_directory(path, check)
    } else if path.is_file() {
        let ok = format_file(path, check)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        if check && !ok {
            eprintln!("{}: would reformat", path.display());
            std::process::exit(1);
        }
        Ok(())
    } else {
        anyhow::bail!("Path '{}' does not exist", path_str);
    }
}

fn format_directory(dir: &Path, check: bool) -> anyhow::Result<()> {
    let mut files = Vec::new();
    collect_salt_files(dir, &mut files).map_err(|e| anyhow::anyhow!("{}", e))?;
    files.sort_unstable();

    let mut needs_reformat = false;
    for path in &files {
        let ok = format_file(path, true)
            .map_err(|e| anyhow::anyhow!("{}: {}", path.display(), e))?;
        if !ok {
            eprintln!("{}: would reformat", path.display());
            needs_reformat = true;
        }
    }

    if !check && needs_reformat {
        for path in &files {
            let _ = format_file(path, false);
        }
    }

    if check && needs_reformat {
        std::process::exit(1);
    }
    Ok(())
}

fn collect_salt_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> std::io::Result<()> {
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                collect_salt_files(&path, out)?;
            } else if path.extension().map_or(false, |e| e == "salt") {
                out.push(path);
            }
        }
    }
    Ok(())
}

/// Format a single .salt file.
/// If `check` is true, returns Ok(true) if already formatted, Ok(false) if would reformat.
/// If `check` is false, formats in place and returns Ok(true).
pub fn format_file(path: &Path, check: bool) -> Result<bool, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;

    let formatted = format_salt(&source).map_err(|e| format!("{}: {}", path.display(), e))?;

    if source == formatted {
        return Ok(true);
    }

    if check {
        return Ok(false);
    }

    std::fs::write(path, &formatted)
        .map_err(|e| format!("Cannot write {}: {}", path.display(), e))?;
    eprintln!("Formatted: {}", path.display());
    Ok(true)
}

/// Format Salt source code.
/// Returns the formatted source or an error message.
pub fn format_salt(source: &str) -> Result<String, String> {
    // Validate by parsing through the Salt preprocessor + syn parser
    let processed = preprocess(source);
    if let Err(e) = syn::parse_str::<SaltFile>(&processed) {
        return Err(format!("Parse error: {}", e));
    }

    let lines = format::format_lines(source);
    let mut formatted = post::post_process(&lines);

    // Ensure trailing newline
    if !formatted.ends_with('\n') {
        formatted.push('\n');
    }

    Ok(formatted)
}

#[cfg(test)]
mod tests;
