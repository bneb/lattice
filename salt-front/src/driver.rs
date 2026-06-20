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

use std::path::PathBuf;

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
        // Use llvm@18 by default — matches the benchmark toolchain.
        // llvm (latest, e.g. 21) does not support all flags we need.
        let base = PathBuf::from("/opt/homebrew/opt/llvm@18/bin");
        Self {
            mlir_opt: base.join("mlir-opt"),
            mlir_translate: base.join("mlir-translate"),
            llc: base.join("llc"),
            clang: base.join("clang"),
            lld: base.join("ld.lld"),
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
    /// Bare-metal ARM64 ELF for KeuOS OS kernel/userspace
    KeuOSArm64,
    /// Bare-metal x86_64 ELF for KeuOS OS kernel/userspace
    KeuOSX86_64,
}

impl Default for DriverTarget {
    fn default() -> Self {
        if cfg!(target_os = "macos") {
            DriverTarget::DarwinArm64
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
            DriverTarget::KeuOSArm64 => "aarch64-unknown-none-elf",
            DriverTarget::KeuOSX86_64 => "x86_64-unknown-none-elf",
        }
    }

    /// Parse a target name from CLI string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "darwin-arm64" | "macos" => Some(DriverTarget::DarwinArm64),
            "linux-arm64" => Some(DriverTarget::LinuxArm64),
            "keuos" | "keuos-arm64" => Some(DriverTarget::KeuOSArm64),
            "keuos-x86" | "keuos-x86_64" => Some(DriverTarget::KeuOSX86_64),
            _ => None,
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
        Self {
            target: DriverTarget::default(),
            build_dir: build_dir.clone(),
            toolchain: ToolchainPaths::default(),
            runtime_obj: build_dir.join("keuos_rt.o"),
            debug_info: false,
            keuos_mode: false,
        }
    }

    pub fn with_target(mut self, target: DriverTarget) -> Self {
        self.target = target;
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
                        DriverTarget::KeuOSX86_64 => {
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

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_driver() -> SaltDriver {
        SaltDriver::new(PathBuf::from("/tmp/salt-build"))
    }

    #[test]
    fn test_pipeline_has_4_steps() {
        let driver = test_driver();
        let steps = driver.build_pipeline("echo_test");

        assert_eq!(steps.len(), 4,
            "Pipeline must have exactly 4 steps: mlir-opt, mlir-translate, llc, link");
        assert_eq!(steps[0].name, "mlir-opt");
        assert_eq!(steps[1].name, "mlir-translate");
        assert_eq!(steps[2].name, "llc");
        assert_eq!(steps[3].name, "link");
    }

    #[test]
    fn test_llc_step_reserves_x19_in_keuos_mode() {
        let driver = test_driver();
        let steps = driver.build_pipeline("echo_test");
        let llc = &steps[2];

        assert!(!llc.has_flag("-reserved-reg=aarch64:x19"),
            "llc step must NOT reserve x19 by default (requires custom LLVM)");

        // KeuOS mode enables x19 reservation
        let sov_driver = SaltDriver::new(PathBuf::from("/tmp/salt-build"));
        // Can't use with_keuos_mode yet, set directly
        let mut sov = sov_driver;
        sov.keuos_mode = true;
        let sov_steps = sov.build_pipeline("echo_test");
        let sov_llc = &sov_steps[2];
        assert!(sov_llc.has_flag("-reserved-reg=aarch64:x19"),
            "llc step MUST reserve x19 in keuos mode");
    }

    #[test]
    fn test_llc_step_enables_lse() {
        let driver = test_driver();
        let steps = driver.build_pipeline("echo_test");
        let llc = &steps[2];

        assert!(llc.has_flag("+lse"),
            "llc step MUST enable LSE atomics for M4 CAS/LDADD");
    }

    #[test]
    fn test_link_step_uses_nostdlib() {
        let driver = test_driver();
        let steps = driver.build_pipeline("echo_test");
        let link = &steps[3];

        assert!(link.has_flag("-nostdlib"),
            "Link step MUST use -nostdlib to eliminate C runtime tax");
    }

    #[test]
    fn test_link_step_includes_runtime() {
        let driver = test_driver();
        let steps = driver.build_pipeline("echo_test");
        let link = &steps[3];

        assert!(link.has_flag("keuos_rt.o"),
            "Link step MUST include keuos_rt.o runtime object");
    }

    #[test]
    fn test_toolchain_paths_default() {
        let paths = ToolchainPaths::default();

        assert!(paths.mlir_opt.to_str().unwrap().contains("/opt/homebrew/opt/llvm@18/bin/"),
            "Default mlir-opt path must be in /opt/homebrew/opt/llvm@18/bin/");
        assert!(paths.llc.to_str().unwrap().contains("llc"),
            "Default llc path must contain 'llc'");
        assert!(paths.clang.to_str().unwrap().contains("clang"),
            "Default clang path must contain 'clang'");
    }

    // =================================================================
    // P4: Object-only pipeline TDD tests
    // =================================================================

    #[test]
    fn test_object_pipeline_has_3_steps() {
        let driver = test_driver();
        let steps = driver.build_object_pipeline("kernel_main");

        assert_eq!(steps.len(), 3,
            "Object pipeline must have exactly 3 steps: mlir-opt, mlir-translate, llc (no link)");
        assert_eq!(steps[0].name, "mlir-opt");
        assert_eq!(steps[1].name, "mlir-translate");
        assert_eq!(steps[2].name, "llc");
    }

    #[test]
    fn test_object_pipeline_stops_before_link() {
        let driver = test_driver();
        let steps = driver.build_object_pipeline("kernel_main");

        for step in &steps {
            assert_ne!(step.name, "link",
                "Object pipeline must NOT contain a link step — we produce .o, not binaries");
        }
    }

    #[test]
    fn test_object_output_is_dot_o() {
        let driver = test_driver();
        let steps = driver.build_object_pipeline("kernel_main");
        let last = steps.last().unwrap();

        assert!(last.output.to_str().unwrap().ends_with(".o"),
            "Object pipeline output must end with .o, got: {:?}", last.output);
    }

    #[test]
    fn test_object_pipeline_shares_flags_with_binary() {
        let driver = test_driver();
        let obj_steps = driver.build_object_pipeline("test");
        let bin_steps = driver.build_pipeline("test");

        // Steps 0-2 must be identical between object and binary pipelines
        for i in 0..3 {
            assert_eq!(obj_steps[i].name, bin_steps[i].name,
                "Step {} name must match between object and binary pipelines", i);
            assert_eq!(obj_steps[i].args, bin_steps[i].args,
                "Step {} args must match between object and binary pipelines", i);
        }
    }

    // =================================================================
    // P3: DWARF debug info TDD tests
    // =================================================================

    #[test]
    fn test_debug_driver_mlir_opt_has_debuginfo_flag() {
        let driver = test_driver().with_debug_info(true);
        let steps = driver.build_object_pipeline("test_debug");
        let mlir_opt = &steps[0];

        assert!(mlir_opt.has_flag("--mlir-print-debuginfo"),
            "mlir-opt step MUST pass --mlir-print-debuginfo when debug_info is enabled");
    }

    #[test]
    fn test_no_debug_driver_mlir_opt_has_no_debuginfo_flag() {
        let driver = test_driver(); // debug_info defaults to false
        let steps = driver.build_object_pipeline("test_release");
        let mlir_opt = &steps[0];

        assert!(!mlir_opt.has_flag("--mlir-print-debuginfo"),
            "mlir-opt step must NOT pass --mlir-print-debuginfo by default");
    }

    // =================================================================
    // KeuOS ELF target TDD tests
    // =================================================================

    #[test]
    fn test_keuos_target_produces_elf_triple() {
        assert_eq!(DriverTarget::KeuOSArm64.triple(), "aarch64-unknown-none-elf",
            "KeuOSArm64 must use bare-metal ELF triple for kernel loader");
    }

    #[test]
    fn test_keuos_x86_target_produces_elf_triple() {
        assert_eq!(DriverTarget::KeuOSX86_64.triple(), "x86_64-unknown-none-elf",
            "KeuOSX86_64 must use bare-metal ELF triple for kernel loader");
    }

    #[test]
    fn test_keuos_target_in_pipeline() {
        let driver = SaltDriver::new(PathBuf::from("/tmp/salt-build"))
            .with_target(DriverTarget::KeuOSArm64);
        let steps = driver.build_object_pipeline("kernel_main");
        let llc = &steps[2];

        assert!(llc.has_flag("aarch64-unknown-none-elf"),
            "llc step must pass bare-metal ELF triple when targeting KeuOS");
        assert!(!llc.has_flag("apple"),
            "KeuOS target must NOT reference Apple/macOS");
    }

    #[test]
    fn test_target_from_str() {
        assert_eq!(DriverTarget::from_str("keuos"), Some(DriverTarget::KeuOSArm64));
        assert_eq!(DriverTarget::from_str("keuos-arm64"), Some(DriverTarget::KeuOSArm64));
        assert_eq!(DriverTarget::from_str("keuos-x86_64"), Some(DriverTarget::KeuOSX86_64));
        assert_eq!(DriverTarget::from_str("macos"), Some(DriverTarget::DarwinArm64));
        assert_eq!(DriverTarget::from_str("bogus"), None);
    }

    #[test]
    fn test_darwin_target_produces_macho_triple() {
        assert!(DriverTarget::DarwinArm64.triple().contains("apple"),
            "DarwinArm64 must use Apple triple for Mach-O output");
    }

    // =================================================================
    // Step 1: KeuOS binary pipeline TDD tests
    // =================================================================
    // These tests define expected behavior BEFORE implementation.
    // The build_keuos_binary_pipeline() method does not exist yet.

    #[test]
    fn test_keuos_x86_binary_pipeline_has_4_steps() {
        let driver = SaltDriver::new(PathBuf::from("/tmp/salt-build"))
            .with_target(DriverTarget::KeuOSX86_64);
        let steps = driver.build_keuos_binary_pipeline("hello_user");

        assert_eq!(steps.len(), 4,
            "KeuOS binary pipeline must have 4 steps: mlir-opt → mlir-translate → llc → lld");
        assert_eq!(steps[0].name, "mlir-opt");
        assert_eq!(steps[1].name, "mlir-translate");
        assert_eq!(steps[2].name, "llc");
        assert_eq!(steps[3].name, "lld-link");
    }

    #[test]
    fn test_keuos_x86_link_uses_lld() {
        let driver = SaltDriver::new(PathBuf::from("/tmp/salt-build"))
            .with_target(DriverTarget::KeuOSX86_64);
        let steps = driver.build_keuos_binary_pipeline("hello_user");
        let link_step = &steps[3];

        assert!(link_step.tool.to_str().unwrap().contains("ld.lld"),
            "KeuOS link step must use ld.lld, not clang");
    }

    #[test]
    fn test_keuos_x86_link_has_nostdlib_and_entry() {
        let driver = SaltDriver::new(PathBuf::from("/tmp/salt-build"))
            .with_target(DriverTarget::KeuOSX86_64);
        let steps = driver.build_keuos_binary_pipeline("hello_user");
        let link_step = &steps[3];

        assert!(link_step.has_flag("-nostdlib"),
            "KeuOS link must be freestanding (-nostdlib)");
        assert!(link_step.has_flag("-e"),
            "KeuOS link must specify entry point (-e)");
        assert!(link_step.has_flag("--image-base"),
            "KeuOS link must set user-space image base");
    }

    #[test]
    fn test_keuos_x86_link_produces_elf_executable() {
        let driver = SaltDriver::new(PathBuf::from("/tmp/salt-build"))
            .with_target(DriverTarget::KeuOSX86_64);
        let steps = driver.build_keuos_binary_pipeline("hello_user");
        let link_step = &steps[3];

        // Output should be an ELF executable (no extension, not .o)
        let output = link_step.output.to_str().unwrap();
        assert!(!output.ends_with(".o"),
            "KeuOS binary output must not be .o (it's a linked executable)");
        assert!(output.ends_with("hello_user"),
            "KeuOS binary output should be the bare name");
    }

    #[test]
    fn test_keuos_x86_llc_uses_x86_triple() {
        let driver = SaltDriver::new(PathBuf::from("/tmp/salt-build"))
            .with_target(DriverTarget::KeuOSX86_64);
        let steps = driver.build_keuos_binary_pipeline("hello_user");
        let llc = &steps[2];

        assert!(llc.has_flag("x86_64-unknown-none-elf"),
            "KeuOS x86_64 pipeline must use bare-metal x86_64 ELF triple");
    }
}
