// =============================================================================
// Iron Driver — MLIR → Native Binary Pipeline
//
// Orchestrates the 4-step LLVM toolchain to produce KeuOS binaries:
//   1. mlir-opt:       MLIR → LLVM dialect
//   2. mlir-translate: LLVM dialect → LLVM IR (.ll)
//   3. llc:            LLVM IR → Object code (.o) with x19 reservation
//   4. clang:          Link with keuos_rt.o → Mach-O/ELF binary
//
// The driver is testable via dry-run: `build_pipeline()` returns the steps
// without executing them, so TDD tests can verify flag correctness.
// =============================================================================

use std::path::{Path, PathBuf};

/// Paths to the LLVM toolchain binaries.
#[derive(Debug, Clone)]
pub struct ToolchainPaths {
    pub mlir_opt: PathBuf,
    pub mlir_translate: PathBuf,
    pub llc: PathBuf,
    pub clang: PathBuf,
    pub lld: PathBuf,
}

impl Default for ToolchainPaths {
    fn default() -> Self {
        // Probe the OS-default LLVM installation path.
        // On macOS, Homebrew installs to /opt/homebrew/opt/llvm@18/bin.
        // On Linux, the tools are on PATH (symlinked by CI or system packages).
        // On Windows, the official LLVM installer places them in Program Files.
        let base = if cfg!(target_os = "macos") {
            PathBuf::from("/opt/homebrew/opt/llvm@18/bin")
        } else if cfg!(target_os = "windows") {
            PathBuf::from("C:\\Program Files\\LLVM\\bin")
        } else {
            // Linux: assume tools are on PATH via symlinks or apt
            PathBuf::from("/usr/bin")
        };
        let exe_suffix = if cfg!(target_os = "windows") { ".exe" } else { "" };
        Self {
            mlir_opt: base.join(format!("mlir-opt{}", exe_suffix)),
            mlir_translate: base.join(format!("mlir-translate{}", exe_suffix)),
            llc: base.join(format!("llc{}", exe_suffix)),
            clang: base.join(format!("clang{}", exe_suffix)),
            lld: base.join(format!("ld.lld{}", exe_suffix)),
        }
    }
}

/// A single step in the MLIR → binary pipeline.
#[derive(Debug, Clone)]
pub struct PipelineStep {
    pub name: &'static str,
    pub tool: PathBuf,
    pub args: Vec<String>,
    pub input: PathBuf,
    pub output: PathBuf,
}

impl PipelineStep {
    /// Check if this step's args contain a specific flag.
    pub fn has_flag(&self, flag: &str) -> bool {
        self.args.iter().any(|a| a.contains(flag))
    }
}

/// Target platform for the produced binary.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DriverTarget {
    DarwinArm64,
    LinuxArm64,
    /// Windows x86_64, PE/COFF format
    WindowsX86_64,
    /// Bare-metal ARM64 ELF for KeuOS OS kernel/userspace
    KeuOSArm64,
    /// Bare-metal x86_64 ELF for KeuOS OS kernel/userspace
    KeuOSX86_64,
}

impl Default for DriverTarget {
    fn default() -> Self {
        if cfg!(target_os = "macos") {
            DriverTarget::DarwinArm64
        } else if cfg!(target_os = "windows") {
            DriverTarget::WindowsX86_64
        } else {
            DriverTarget::LinuxArm64
        }
    }
}

impl DriverTarget {
    /// Returns the LLVM target triple for this target.
    pub fn triple(&self) -> &'static str {
        match self {
            DriverTarget::DarwinArm64 => "arm64-apple-macosx14.0.0",
            DriverTarget::LinuxArm64 => "aarch64-unknown-linux-gnu",
            DriverTarget::WindowsX86_64 => "x86_64-pc-windows-msvc",
            DriverTarget::KeuOSArm64 => "aarch64-unknown-none-elf",
            DriverTarget::KeuOSX86_64 => "x86_64-unknown-none-elf",
        }
    }

    /// Returns the output file extension for executables on this target.
    pub fn exe_suffix(&self) -> &'static str {
        match self {
            DriverTarget::WindowsX86_64 => ".exe",
            _ => "",
        }
    }

}

impl std::str::FromStr for DriverTarget {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "darwin-arm64" | "macos" => Ok(DriverTarget::DarwinArm64),
            "linux-arm64" => Ok(DriverTarget::LinuxArm64),
            "windows" | "windows-x86_64" | "win32" => Ok(DriverTarget::WindowsX86_64),
            "keuos" | "keuos-arm64" => Ok(DriverTarget::KeuOSArm64),
            "keuos-x86" | "keuos-x86_64" => Ok(DriverTarget::KeuOSX86_64),
            _ => Err(format!("unknown target: {}", s)),
        }
    }
}

/// The Iron Driver: orchestrates MLIR → native binary production.
#[derive(Debug, Clone)]
pub struct SaltDriver {
    pub target: DriverTarget,
    pub build_dir: PathBuf,
    pub toolchain: ToolchainPaths,
    pub runtime_obj: PathBuf,
    pub debug_info: bool,
    /// When true, adds -reserved-reg=aarch64:x19 for KeuOS kernel builds.
    /// Requires a custom LLVM build; standard llvm@18 does not support this flag.
    pub keuos_mode: bool,
}

impl SaltDriver {
    pub fn new(build_dir: PathBuf) -> Self {
        let target = DriverTarget::default();
        let runtime_obj = Self::runtime_for_target(&target, &build_dir);
        Self {
            target,
            build_dir: build_dir.clone(),
            toolchain: ToolchainPaths::default(),
            runtime_obj,
            debug_info: false,
            keuos_mode: false,
        }
    }

    /// The runtime object that should be linked for a given target.
    /// Returns the path where the compiled .o file is expected.
    pub fn runtime_for_target(target: &DriverTarget, build_dir: &Path) -> PathBuf {
        match target {
            DriverTarget::KeuOSArm64 | DriverTarget::KeuOSX86_64 => build_dir.join("keuos_rt.o"),
            DriverTarget::WindowsX86_64 => build_dir.join("runtime_win.o"),
            _ => build_dir.join("runtime.o"),
        }
    }

    /// The runtime source file for a given target.
    pub fn runtime_source(target: &DriverTarget) -> &'static str {
        match target {
            DriverTarget::KeuOSArm64 | DriverTarget::KeuOSX86_64 => "keuos_rt.c",
            DriverTarget::WindowsX86_64 => "runtime_win.c",
            _ => "runtime.c",
        }
    }

    pub fn with_target(mut self, target: DriverTarget) -> Self {
        self.target = target;
        self.runtime_obj = Self::runtime_for_target(&target, &self.build_dir);
        self
    }

    pub fn with_toolchain(mut self, toolchain: ToolchainPaths) -> Self {
        self.toolchain = toolchain;
        self
    }

    pub fn with_runtime(mut self, runtime_obj: PathBuf) -> Self {
        self.runtime_obj = runtime_obj;
        self
    }

    pub fn with_debug_info(mut self, debug: bool) -> Self {
        self.debug_info = debug;
        self
    }

    /// Build the pipeline steps without executing them (dry-run for TDD).
    /// Returns the 4 steps: mlir-opt → mlir-translate → llc → link.
    pub fn build_pipeline(&self, output_name: &str) -> Vec<PipelineStep> {
        let mlir_file = self.build_dir.join(format!("{}.mlir", output_name));
        let _scf_file = self.build_dir.join(format!("{}_scf.mlir", output_name));
        let opt_file = self.build_dir.join(format!("{}_opt.mlir", output_name));
        let ll_file = self.build_dir.join(format!("{}.ll", output_name));
        let obj_file = self.build_dir.join(format!("{}.o", output_name));
        let bin_file = self.build_dir.join(output_name);

        let target_triple = self.target.triple();

        vec![
            // Step 1: mlir-opt — lower to LLVM dialect
            PipelineStep {
                name: "mlir-opt",
                tool: self.toolchain.mlir_opt.clone(),
                args: {
                    let mut opt_args = vec![
                        "--convert-linalg-to-loops".into(),
                        "--cse".into(),
                        "--lower-affine".into(),
                        "--convert-vector-to-scf".into(),
                        "--convert-scf-to-cf".into(),
                        "--convert-cf-to-llvm".into(),
                        "--convert-vector-to-llvm".into(),
                        "--convert-math-to-llvm".into(),
                        "--convert-arith-to-llvm".into(),
                        "--finalize-memref-to-llvm".into(),
                        "--convert-func-to-llvm".into(),
                        "--reconcile-unrealized-casts".into(),
                    ];
                    if self.debug_info {
                        opt_args.push("--mlir-print-debuginfo".into());
                    }
                    opt_args
                },
                input: mlir_file,
                output: opt_file.clone(),
            },
            // Step 2: mlir-translate — MLIR → LLVM IR
            PipelineStep {
                name: "mlir-translate",
                tool: self.toolchain.mlir_translate.clone(),
                args: vec!["--mlir-to-llvmir".into()],
                input: opt_file,
                output: ll_file.clone(),
            },
            // Step 3: llc — LLVM IR → object code with target-specific flags
            PipelineStep {
                name: "llc",
                tool: self.toolchain.llc.clone(),
                args: {
                    let mut llc_args = vec![
                        "-O3".into(),
                        format!("-mtriple={}", target_triple),
                    ];
                    // CPU and feature flags are target-dependent
                    match self.target {
                        DriverTarget::DarwinArm64 | DriverTarget::LinuxArm64 => {
                            llc_args.push("-mcpu=apple-m4".into());
                            llc_args.push("-mattr=+lse".into());
                        }
                        DriverTarget::KeuOSArm64 => {
                            llc_args.push("-mcpu=cortex-a76".into());
                            llc_args.push("-mattr=+lse".into());
                        }
                        DriverTarget::KeuOSX86_64 | DriverTarget::WindowsX86_64 => {
                            // Generic x86_64 — no special CPU flags needed
                        }
                    }
                    llc_args.push("--frame-pointer=none".into());
                    llc_args.push("-filetype=obj".into());
                    if self.keuos_mode {
                        llc_args.push("-reserved-reg=aarch64:x19".into());
                    }
                    llc_args
                },
                input: ll_file,
                output: obj_file.clone(),
            },
            // Step 4: clang — link with keuos_rt.o, no libc
            PipelineStep {
                name: "link",
                tool: self.toolchain.clang.clone(),
                args: {
                    let mut args = vec![
                        "-nostdlib".into(),
                        "-static".into(),
                        "-O3".into(),
                    ];
                    args.push(self.runtime_obj.to_string_lossy().into_owned());
                    args
                },
                input: obj_file,
                output: bin_file,
            },
        ]
    }

    /// Build the object-only pipeline (steps 1-3, no link).
    /// Produces a .o file suitable for the kernel's ELF loader.
    pub fn build_object_pipeline(&self, output_name: &str) -> Vec<PipelineStep> {
        let full = self.build_pipeline(output_name);
        // Take only mlir-opt, mlir-translate, llc — drop the link step
        full.into_iter().take(3).collect()
    }

    /// Build the KeuOS OS binary pipeline: MLIR → LLVM IR → .o → linked ELF.
    /// Uses ld.lld (not clang) for freestanding bare-metal linking.
    /// Produces a fully linked ELF executable at a fixed user-space base address.
    pub fn build_keuos_binary_pipeline(&self, output_name: &str) -> Vec<PipelineStep> {
        let mut steps = self.build_object_pipeline(output_name);
        let obj_file = self.build_dir.join(format!("{}.o", output_name));
        let elf_file = self.build_dir.join(output_name);

        steps.push(PipelineStep {
            name: "lld-link",
            tool: self.toolchain.lld.clone(),
            args: vec![
                "-nostdlib".into(),
                "-static".into(),
                "-e".into(),
                "main".into(),
                "--image-base=0x400000".into(),
            ],
            input: obj_file,
            output: elf_file,
        });

        steps
    }

    /// Execute the object-only pipeline: write MLIR → run 3 steps → produce .o file.
    pub fn compile_object(&self, mlir_source: &str, output_name: &str) -> Result<PathBuf, String> {
        let mlir_path = self.build_dir.join(format!("{}.mlir", output_name));

        std::fs::create_dir_all(&self.build_dir)
            .map_err(|e| format!("Failed to create build dir: {}", e))?;

        std::fs::write(&mlir_path, mlir_source)
            .map_err(|e| format!("Failed to write MLIR: {}", e))?;

        let steps = self.build_object_pipeline(output_name);

        for step in &steps {
            let mut cmd = std::process::Command::new(&step.tool);
            cmd.args(&step.args);
            cmd.arg(step.input.to_str().unwrap());
            cmd.arg("-o");
            cmd.arg(step.output.to_str().unwrap());

            let status = cmd.status()
                .map_err(|e| format!("{} failed to execute: {}", step.name, e))?;

            if !status.success() {
                return Err(format!("{} failed with exit code: {:?}", step.name, status.code()));
            }
        }

        let output = steps.last().unwrap().output.clone();
        Ok(output)
    }

    /// Execute the KeuOS OS binary pipeline: write MLIR → run 4 steps → produce linked ELF.
    pub fn compile_keuos_binary(&self, mlir_source: &str, output_name: &str) -> Result<PathBuf, String> {
        let mlir_path = self.build_dir.join(format!("{}.mlir", output_name));

        std::fs::create_dir_all(&self.build_dir)
            .map_err(|e| format!("Failed to create build dir: {}", e))?;

        std::fs::write(&mlir_path, mlir_source)
            .map_err(|e| format!("Failed to write MLIR: {}", e))?;

        let steps = self.build_keuos_binary_pipeline(output_name);

        for step in &steps {
            let mut cmd = std::process::Command::new(&step.tool);
            cmd.args(&step.args);

            if step.name == "lld-link" {
                // lld takes: ld.lld [flags] input.o -o output
                cmd.arg(step.input.to_str().unwrap());
                cmd.arg("-o");
                cmd.arg(step.output.to_str().unwrap());
            } else {
                // mlir-opt/mlir-translate/llc take: tool [flags] input -o output
                cmd.arg(step.input.to_str().unwrap());
                cmd.arg("-o");
                cmd.arg(step.output.to_str().unwrap());
            }

            let output = cmd.output()
                .map_err(|e| format!("{} failed to execute: {}", step.name, e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("{} failed (exit {:?}): {}", step.name, output.status.code(), stderr));
            }
        }

        let output = steps.last().unwrap().output.clone();
        Ok(output)
    }

    /// Execute the full pipeline: write MLIR → run steps → produce binary.
    pub fn compile(&self, mlir_source: &str, output_name: &str) -> Result<PathBuf, String> {
        let mlir_path = self.build_dir.join(format!("{}.mlir", output_name));

        // Ensure build dir exists
        std::fs::create_dir_all(&self.build_dir)
            .map_err(|e| format!("Failed to create build dir: {}", e))?;

        // Write MLIR source
        std::fs::write(&mlir_path, mlir_source)
            .map_err(|e| format!("Failed to write MLIR: {}", e))?;

        let steps = self.build_pipeline(output_name);

        for step in &steps {
            let mut cmd = std::process::Command::new(&step.tool);
            cmd.args(&step.args);

            // For mlir-opt and mlir-translate: input via arg, output via -o
            match step.name {
                "mlir-opt" => {
                    cmd.arg(step.input.to_str().unwrap());
                    cmd.arg("-o");
                    cmd.arg(step.output.to_str().unwrap());
                }
                "mlir-translate" => {
                    cmd.arg(step.input.to_str().unwrap());
                    cmd.arg("-o");
                    cmd.arg(step.output.to_str().unwrap());
                }
                "llc" => {
                    cmd.arg(step.input.to_str().unwrap());
                    cmd.arg("-o");
                    cmd.arg(step.output.to_str().unwrap());
                }
                "link" => {
                    cmd.arg(step.input.to_str().unwrap());
                    cmd.arg("-o");
                    cmd.arg(step.output.to_str().unwrap());
                }
                _ => {}
            }

            let status = cmd.status()
                .map_err(|e| format!("{} failed to execute: {}", step.name, e))?;

            if !status.success() {
                return Err(format!("{} failed with exit code: {:?}", step.name, status.code()));
            }
        }

        let output = steps.last().unwrap().output.clone();
        Ok(output)
    }
}