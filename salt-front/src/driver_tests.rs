#[cfg(test)]
mod tests {
    use crate::driver::*;
    use std::path::PathBuf;
    use std::str::FromStr;

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

        // Default target on macOS is DarwinArm64 → runtime.o
        let expected_rt = SaltDriver::runtime_for_target(&DriverTarget::default(), &PathBuf::from("/tmp/salt-build"));
        let rt_name = expected_rt.file_name().unwrap().to_str().unwrap();
        assert!(link.has_flag(rt_name),
            "Link step MUST include {} runtime object", rt_name);
    }

    #[test]
    fn test_toolchain_paths_default() {
        let paths = ToolchainPaths::default();

        let expected_dir = if cfg!(target_os = "macos") {
            "/opt/homebrew/opt/llvm@18/bin/"
        } else if cfg!(target_os = "windows") {
            "C:\\Program Files\\LLVM\\bin\\"
        } else {
            "/usr/bin/"
        };
        assert!(paths.mlir_opt.to_str().unwrap().contains(expected_dir),
            "Default mlir-opt path must be in {}: got {:?}", expected_dir, paths.mlir_opt);
        assert!(paths.llc.to_str().unwrap().contains("llc"),
            "Default llc path must contain 'llc': {:?}", paths.llc);
        assert!(paths.clang.to_str().unwrap().contains("clang"),
            "Default clang path must contain 'clang': {:?}", paths.clang);
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
    fn test_windows_runtime_selection() {
        let build_dir = PathBuf::from("/tmp/salt-build");
        // Windows target selects runtime_win.o
        let driver = test_driver().with_target(DriverTarget::WindowsX86_64);
        let rt_name = driver.runtime_obj.file_name().unwrap().to_str().unwrap();
        assert_eq!(rt_name, "runtime_win.o",
            "Windows target must link runtime_win.o");
        assert_eq!(driver.target.exe_suffix(), ".exe",
            "Windows target must use .exe suffix");
        assert!(driver.target.triple().contains("windows"),
            "Windows target triple must contain 'windows': {}", driver.target.triple());
    }

    #[test]
    fn test_runtime_source_selection() {
        assert_eq!(SaltDriver::runtime_source(&DriverTarget::WindowsX86_64), "runtime_win.c");
        assert_eq!(SaltDriver::runtime_source(&DriverTarget::DarwinArm64), "runtime.c");
        assert_eq!(SaltDriver::runtime_source(&DriverTarget::KeuOSX86_64), "keuos_rt.c");
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
        assert_eq!(DriverTarget::from_str("keuos"), Ok(DriverTarget::KeuOSArm64));
        assert_eq!(DriverTarget::from_str("keuos-arm64"), Ok(DriverTarget::KeuOSArm64));
        assert_eq!(DriverTarget::from_str("keuos-x86_64"), Ok(DriverTarget::KeuOSX86_64));
        assert_eq!(DriverTarget::from_str("macos"), Ok(DriverTarget::DarwinArm64));
        assert!(DriverTarget::from_str("bogus").is_err());
    }

    #[test]
    fn test_darwin_target_produces_macho_triple() {
        assert!(DriverTarget::DarwinArm64.triple().contains("apple"),
            "DarwinArm64 must use Apple triple for Mach-O output");
    }

    // =================================================================
    // Step 1: KeuOS binary pipeline TDD tests
    // =================================================================

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
