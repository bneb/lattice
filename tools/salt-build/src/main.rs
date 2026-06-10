//! Salt Build System — CLI Entry Point
//!
//! Usage:
//!   salt build       — Compile the project
//!   salt run         — Compile and run the project
//!   salt test        — Compile and run tests
//!   salt init <name> — Initialize a new project

mod manifest;
mod resolver;
mod compiler;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "salt", version, about = "Salt build system")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile the project
    Build {
        /// Path to project directory (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Build in release mode
        #[arg(long)]
        release: bool,
    },

    /// Compile and run the project
    Run {
        /// Path to project directory
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Arguments to pass to the program
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    /// Run tests
    Test {
        /// Path to project directory
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Filter tests by name
        #[arg(long)]
        filter: Option<String>,
    },

    /// Initialize a new Salt project
    Init {
        /// Project name
        name: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Build { path, release } => cmd_build(&path, release),
        Commands::Run { path, args } => cmd_run(&path, &args),
        Commands::Test { path, filter } => cmd_test(&path, filter.as_deref()),
        Commands::Init { name } => cmd_init(&name),
    };

    if let Err(e) = result {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}

fn cmd_build(path: &PathBuf, release: bool) -> Result<(), String> {
    let manifest_path = path.join("salt.toml");
    let manifest = manifest::load(&manifest_path)?;

    println!("📦 Building {} v{}", manifest.package.name, manifest.package.version);

    // Resolve dependencies
    let build_order = resolver::resolve(&manifest, path)?;

    println!("   {} module(s) to compile", build_order.len());

    // Compile each module in order
    for module in &build_order {
        println!("   🔨 Compiling {}", module.display());
        compiler::compile_module(module, path, release)?;
    }

    // Link final binary
    let output = compiler::link(&manifest, path, release)?;
    println!("✅ Built: {}", output.display());

    Ok(())
}

fn cmd_run(path: &PathBuf, args: &[String]) -> Result<(), String> {
    cmd_build(path, false)?;

    let manifest_path = path.join("salt.toml");
    let manifest = manifest::load(&manifest_path)?;

    let binary = compiler::output_path(&manifest, path, false);
    println!("🚀 Running {}", binary.display());

    let status = std::process::Command::new(&binary)
        .args(args)
        .status()
        .map_err(|e| format!("Failed to run: {}", e))?;

    if !status.success() {
        return Err(format!("Process exited with status: {}", status));
    }

    Ok(())
}

fn find_test_files(path: &Path, filter: Option<&str>) -> Result<Vec<PathBuf>, String> {
    let test_dir = path.join("tests");
    if !test_dir.exists() { return Ok(Vec::new()); }
    let mut files: Vec<PathBuf> = std::fs::read_dir(&test_dir)
        .map_err(|e| format!("Failed to read tests/: {}", e))?
        .filter_map(|e| e.ok()).map(|e| e.path())
        .filter(|p| p.extension().map_or(false, |e| e == "salt")).collect();
    if let Some(f) = filter {
        files.retain(|p| p.file_stem().map_or(false, |s| s.to_string_lossy().contains(f)));
    }
    files.sort();
    Ok(files)
}

fn cmd_test(path: &PathBuf, filter: Option<&str>) -> Result<(), String> {
    let manifest = manifest::load(&path.join("salt.toml"))?;
    let test_files = find_test_files(path, filter)?;
    if test_files.is_empty() {
        println!("No tests directory found or no tests match");
        return Ok(());
    }

    println!("🧪 Running {} test(s) for {}", test_files.len(), manifest.package.name);
    let mut passed = 0; let mut failed = 0;
    for test_file in &test_files {
        let name = test_file.file_stem().unwrap().to_string_lossy();
        print!("   {} ... ", name);
        match compiler::run_test(test_file, path) {
            Ok(_) => { println!("✅ PASS"); passed += 1; }
            Err(e) => { println!("❌ FAIL: {}", e); failed += 1; }
        }
    }
    println!("\nResults: {} passed, {} failed", passed, failed);
    if failed > 0 { Err(format!("{} test(s) failed", failed)) } else { Ok(()) }
}

fn generate_manifest(name: &str) -> String {
    format!("[package]\nname = \"{}\"\nversion = \"0.1.0\"\nentry = \"src/main.salt\"\n", name)
}

fn cmd_init(name: &str) -> Result<(), String> {
    let project_dir = PathBuf::from(name);
    if project_dir.exists() { return Err(format!("Directory '{}' already exists", name)); }
    std::fs::create_dir_all(project_dir.join("src")).map_err(|e| format!("Failed to create src/: {}", e))?;
    std::fs::create_dir_all(project_dir.join("tests")).map_err(|e| format!("Failed to create tests/: {}", e))?;
    std::fs::write(project_dir.join("salt.toml"), generate_manifest(name)).map_err(|e| format!("Failed to write salt.toml: {}", e))?;
    std::fs::write(project_dir.join("src/main.salt"), "package main\n\nfn main() -> i32 {\n    println(\"Hello, Salt!\");\n    return 0;\n}\n").map_err(|e| format!("Failed to write main.salt: {}", e))?;
    println!("✨ Created project '{}' at {}/\n   salt.toml\n   src/main.salt\n   tests/", name, project_dir.display());
    Ok(())
}
