use std::fs;
use std::path::PathBuf;

pub fn run_cli(args: Vec<String>) -> anyhow::Result<()> {
    let mut path_opt: Option<String> = None;
    let mut output_path: Option<String> = None;
    let mut release_mode = false;
    let mut skip_scan = false;
    let mut vverify = false;
    let mut binary_mode = false;
    let mut object_mode = false;
    let mut disable_alias_scopes = false;
    let mut no_verify = false;
    let mut lib_mode = false;
    let mut sip_mode = false;
    let mut debug_info = false;
    let mut emit_sir = false;
    let mut target_name: Option<String> = None;
    
    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--release" {
            release_mode = true;
        } else if arg == "--help" || arg == "-h" {
            println!("Usage: salt-front <file.salt> [-o output] [--release] [--binary] [-c] [--target <target>] [--lib] [-g] [--emit-sir] [--skip-scan] [--verify] [--danger-no-verify] [--disable-alias-scopes]");
            println!("");
            println!("Flags:");
            println!("  --release    Enable optimizations");
            println!("  --binary     Produce native Mach-O/ELF binary via Iron Driver");
            println!("  -c           Produce .o object file (like clang -c)");
            println!("  --target T   Target: macos, linux-arm64, keuos, keuos-x86_64");
            println!("  --verify     Run Z3 verification passes");
            println!("  --skip-scan  Skip import scanning");
            println!("  --lib        Library mode (no main entry point required)");
            println!("  --sip        Mode B SIP safety enforcement (rejects raw pointer creation)");
            println!("  -g           Emit DWARF debug info (MLIR loc annotations)");
            println!("  --debug-info Emit DWARF debug info (same as -g)");
            println!("  --disable-alias-scopes  Suppress LLVM alias scope metadata (for mlir-opt compatibility)");
            println!("  --emit-sir  Emit SIR (Salt Intermediate Representation) as JSON alongside MLIR");
            println!("  --danger-no-verify  Skip ALL Z3/ownership verification (NOT for production)");
            println!("  -o <path>    Output path (MLIR or binary)");
            return Ok(());
        } else if arg == "--skip-scan" {
            skip_scan = true;
        } else if arg == "--vverify" || arg == "--verify" {
            vverify = true;
        } else if arg == "--bench" {
            // Benchmark mode (release implied)
            release_mode = true; 
        } else if arg == "--binary" {
            // KeuOS binary mode: full MLIR → native pipeline
            binary_mode = true;
            release_mode = true; // Binary mode implies release
        } else if arg == "-c" {
            object_mode = true;
            release_mode = true; // Object mode implies release
        } else if arg == "--target" {
            if i + 1 < args.len() {
                target_name = Some(args[i+1].clone());
                i += 1;
            } else {
                anyhow::bail!("--target requires an argument (e.g. keuos, macos, linux-arm64)");
            }
        } else if arg == "--disable-alias-scopes" {
            disable_alias_scopes = true;
        } else if arg == "--danger-no-verify" {
            #[cfg(not(debug_assertions))]
            {
                panic!("FATAL: Z3 verification cannot be disabled in release builds.");
            }
            #[cfg(debug_assertions)]
            {
                eprintln!("⚠️  WARNING: --danger-no-verify disables ALL Z3 verification. NOT for production use.");
                no_verify = true;
            }
        } else if arg == "--no-verify" {
            #[cfg(not(debug_assertions))]
            {
                panic!("FATAL: Z3 verification cannot be disabled in release builds.");
            }
            #[cfg(debug_assertions)]
            {
                eprintln!("⚠️  DEPRECATED: --no-verify is deprecated. Use --danger-no-verify instead.");
                no_verify = true;
            }
        } else if arg == "--lib" {
            lib_mode = true;
        } else if arg == "--sip" {
            sip_mode = true;
            lib_mode = true; // SIPs are always libraries (no kernel main)
        } else if arg == "--emit-sir" {
            emit_sir = true;
        } else if arg == "-g" || arg == "--debug-info" {
            debug_info = true;
        } else if arg == "-o" {
            if i + 1 < args.len() {
                output_path = Some(args[i+1].clone());
                i += 1;
            } else {
                anyhow::bail!("-o requires an argument");
            }
        } else if arg.starts_with("-") {
            anyhow::bail!("Unknown argument: {}", arg);
        } else {
            path_opt = Some(arg.clone());
        }
        i += 1;
    }

    let path = match path_opt {
        Some(p) => p,
        None => {
            println!("Usage: salt-front <file.salt> [-o output] [--release] [--binary] [-c] [--target <target>] [--lib] [-g] [--skip-scan] [--verify] [--no-verify] [--disable-alias-scopes]");
            return Ok(());
        }
    };

    let code = fs::read_to_string(&path).map_err(|e| {
        anyhow::anyhow!("Failed to read source file '{}': {}", path, e)
    })?;

    let processed = crate::preprocess(&code);
    let mut file: crate::grammar::SaltFile = syn::parse_str(&processed)?;

    // Load dependencies
    let mut registry = crate::registry::Registry::new();
    
    // Pre-register main to avoid infinite recursion if any dependencies import it
    let main_pkg = if let Some(pkg) = &file.package {
        pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(".")
    } else {
        "main".to_string()
    };
    
    registry.register(crate::registry::ModuleInfo::new(&main_pkg));

    // [PRELUDE] Inject implicit stdlib imports for built-in types.
    // Ptr<T> is a built-in type whose methods (write, read, offset) live in std/core/ptr.salt.
    // Without this import, standalone files can use Ptr<T> but can't call its methods.
    // This acts as Salt's implicit prelude, similar to Rust's std::prelude.
    {
        let prelude_imports = [
            "use std::core::ptr::*;",
        ];
        for import_str in &prelude_imports {
            let processed = crate::preprocess(import_str);
            if let Ok(parsed) = syn::parse_str::<crate::grammar::SaltFile>(&processed) {
                file.imports.extend(parsed.imports);
            }
        }
    }

    load_imports(&file, &mut registry);

    match crate::compile_ast(&mut file, release_mode, Some(&registry), skip_scan, vverify, disable_alias_scopes, no_verify, lib_mode, sip_mode, debug_info, &path) {
        Ok(mlir) => {
            // SIR emission (if requested)
            if emit_sir {
                use crate::codegen::sir::types::*;
                use crate::codegen::sir::sir_emit::*;

                let module_name = std::path::Path::new(&path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("module");

                let sir_module = extract_sir_from_ast(&file, module_name);

                let sir_json = sir_module.to_json();
                let sir_path = output_path.as_deref()
                    .map(|p| format!("{}.sir.json", p.trim_end_matches(".mlir")))
                    .unwrap_or_else(|| format!("{}.sir.json", module_name));

                if let Err(e) = fs::write(&sir_path, &sir_json) {
                    eprintln!("⚠️  SIR emission failed: {}", e);
                } else {
                    eprintln!("📋 SIR emitted: {} ({} structs, {} functions, v{})",
                        sir_path, sir_module.structs.len(), sir_module.functions.len(), SIR_VERSION);
                }
            }
            if binary_mode {
                // ============================================================
                // KEUOS BINARY MODE — Full MLIR → Native Pipeline
                // ============================================================
                let basename = std::path::Path::new(&path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("output");
                
                let output_bin = output_path
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from(basename));
                
                let build_dir = std::env::temp_dir().join("salt-build");
                let mut driver = crate::driver::SaltDriver::new(build_dir);
                if let Some(ref t) = target_name {
                    driver = driver.with_target(
                        crate::driver::DriverTarget::from_str(t)
                            .ok_or_else(|| anyhow::anyhow!("Unknown target: '{}'. Valid: macos, linux-arm64, keuos, keuos-x86_64", t))?
                    );
                }
                
                eprintln!("🏛️  [KeuOS] Driving MLIR → native binary...");
                eprintln!("    Target: {:?}", driver.target);
                
                let is_keuos = matches!(driver.target,
                    crate::driver::DriverTarget::KeuOSArm64 |
                    crate::driver::DriverTarget::KeuOSX86_64
                );

                let compile_result = if is_keuos {
                    eprintln!("    Linker: ld.lld (freestanding ELF)");
                    driver.compile_keuos_binary(&mlir, basename)
                } else {
                    eprintln!("    Runtime: {:?}", driver.runtime_obj);
                    driver.compile(&mlir, basename)
                };

                match compile_result {
                    Ok(produced_path) => {
                        // Copy to requested output path if different
                        if produced_path != output_bin {
                            fs::copy(&produced_path, &output_bin).map_err(|e| {
                                anyhow::anyhow!("Failed to copy binary to {:?}: {}", output_bin, e)
                            })?;
                        }
                        
                        // Post-compilation KeuOS Audit
                        eprintln!("⚖️  [KeuOS] Running KeuOS Audit...");
                        if let Ok(output) = std::process::Command::new("otool").arg("-tV").arg(&output_bin).output() {
                            let disasm = String::from_utf8_lossy(&output.stdout);
                            let config = crate::codegen::passes::binary_audit::BinaryAuditConfig::standard(
                                crate::codegen::passes::io_backend::TargetPlatform::Darwin
                            );
                            let results = crate::codegen::passes::binary_audit::run_audit(&config, &disasm);
                            let mut all_passed = true;
                            for res in results {
                                if !res.passed {
                                    all_passed = false;
                                    eprintln!("    ❌ Rule failed: {:?}", res.rule);
                                    eprintln!("       {}", res.detail);
                                }
                            }
                            if all_passed {
                                eprintln!("    ✅ Audit passed.");
                            } else {
                                eprintln!("    ⚠️ Audit found violations.");
                            }
                        } else {
                            eprintln!("    ⚠️ Could not run otool to audit binary.");
                        }
                        eprintln!("✅  [KeuOS] Binary synthesized: {:?}", output_bin);
                        eprintln!("    Pipeline: mlir-opt → mlir-translate → llc (x19 reserved) → clang (-nostdlib)");
                    }
                    Err(e) => {
                        eprintln!("❌  [KeuOS] Binary synthesis failed: {}", e);
                        eprintln!("    Ensure LLVM tools are installed at /opt/homebrew/opt/llvm/bin/");
                        eprintln!("    Ensure keuos_rt.o is built (cd keuos_rt && make)");
                        std::process::exit(1);
                    }
                }
            } else if object_mode {
                // ============================================================
                // OBJECT MODE — MLIR → .o (like clang -c)
                // ============================================================
                let basename = std::path::Path::new(&path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("output");

                let output_obj = output_path
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from(format!("{}.o", basename)));

                let build_dir = std::env::temp_dir().join("salt-build");
                let mut driver = crate::driver::SaltDriver::new(build_dir)
                    .with_debug_info(debug_info);
                if let Some(ref t) = target_name {
                    driver = driver.with_target(
                        crate::driver::DriverTarget::from_str(t)
                            .ok_or_else(|| anyhow::anyhow!("Unknown target: '{}'. Valid: macos, linux-arm64, keuos, keuos-x86_64", t))?
                    );
                }

                eprintln!("🔧 [Object] Compiling to .o...");

                match driver.compile_object(&mlir, basename) {
                    Ok(produced_path) => {
                        if produced_path != output_obj {
                            fs::copy(&produced_path, &output_obj).map_err(|e| {
                                anyhow::anyhow!("Failed to copy object to {:?}: {}", output_obj, e)
                            })?;
                        }
                        eprintln!("✅ Object file: {:?}", output_obj);
                    }
                    Err(e) => {
                        eprintln!("❌ Object compilation failed: {}", e);
                        std::process::exit(1);
                    }
                }
            } else if let Some(out_p) = output_path {
                fs::write(out_p, mlir)?;
            } else {
                println!("{}", mlir);
            }
        },
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

pub fn load_imports(file: &crate::grammar::SaltFile, registry: &mut crate::registry::Registry) {
    use crate::grammar::Item;

    for imp in &file.imports {
        // Convert package path to file path
        // e.g. kernel.arch.x86.gdt -> kernel/arch/x86/gdt.salt
        let original_parts: Vec<String> = imp.name.iter().map(|id| id.to_string()).collect();
        let mut parts = original_parts.clone();
        
        // Loop to support fallback (peeling off the last component to find the module file)
        // e.g., std.core.ptr.Ptr -> std/core/ptr.salt
        loop {
            let pkg_name = parts.join(".");
            
            // If module is already loaded, we are good.
            if registry.modules.contains_key(&pkg_name) {
                break;
            }

            let path_str = format!("{}.salt", parts.join("/"));
            let path_str_mod = format!("{}/mod.salt", parts.join("/"));
            let path_str_lower = format!("{}.salt", parts.iter().map(|s| s.to_lowercase()).collect::<Vec<_>>().join("/"));
            let path_str_mod_lower = format!("{}/mod.salt", parts.iter().map(|s| s.to_lowercase()).collect::<Vec<_>>().join("/"));

            // Search paths: CWD, parent directories, project root
            let search_paths = vec![
                path_str.clone(),                          // CWD-relative (original case)
                path_str_mod.clone(),                      // CWD-relative/mod (original case)
                path_str_lower.clone(),                    // CWD-relative (lowercase)
                path_str_mod_lower.clone(),                // CWD-relative/mod (lowercase)
                format!("../{}", path_str),                // Parent directory
                format!("../{}", path_str_mod),
                format!("../{}", path_str_lower),          // Parent directory (lowercase)
                format!("../{}", path_str_mod_lower),
                format!("../../{}", path_str),             // Grandparent
                format!("../../{}", path_str_mod),
                format!("../../{}", path_str_lower),       // Grandparent (lowercase)
                format!("../../{}", path_str_mod_lower),
                format!("../../../{}", path_str),          // Great-grandparent
                format!("../../../{}", path_str_mod),
                format!("../../../{}", path_str_lower),    // Great-grandparent (lowercase)
                format!("../../../{}", path_str_mod_lower),
            ];

            let mut code_result = None;
            let mut found_path = path_str.clone();

            for search_path in &search_paths {
                if let Ok(code) = fs::read_to_string(search_path) {
                    code_result = Some(code);
                    found_path = search_path.clone();
                    break;
                }
            }

            if let Some(code) = code_result {
                let processed = crate::preprocess(&code);
                if let Ok(imported_file) = syn::parse_str::<crate::grammar::SaltFile>(&processed) {
                    // Register the module
                    let mut info = crate::registry::ModuleInfo::new(&pkg_name);

                    // Extract pub functions
                    for import_item in &imported_file.items {
                        fn extract_args(args: &syn::punctuated::Punctuated<crate::grammar::Arg, syn::token::Comma>) -> Vec<crate::types::Type> {
                            args.iter().filter_map(|arg| {
                                if let Some(ref syn_ty) = arg.ty {
                                    crate::types::Type::from_syn(syn_ty)
                                } else { None }
                            }).collect()
                        }

                        if let Item::Fn(f) = import_item {
                            let args = extract_args(&f.args);
                            let ret = if let Some(ref ret) = f.ret_type {
                                crate::types::Type::from_syn(ret).unwrap_or(crate::types::Type::Unit)
                            } else {
                                crate::types::Type::Unit
                            };
                            info.functions.insert(f.name.to_string(), (args, ret));
                            info.function_templates.insert(f.name.to_string(), f.clone());
                        }
                        if let Item::ExternFn(ef) = import_item {
                             let args = extract_args(&ef.args);
                             let ret = if let Some(ref ret) = ef.ret_type {
                                 crate::types::Type::from_syn(ret).unwrap_or(crate::types::Type::Unit)
                             } else {
                                 crate::types::Type::Unit
                             };
                             info.functions.insert(ef.name.to_string(), (args, ret));
                        }
                        if let Item::Const(c) = import_item {
                            let eval = crate::evaluator::Evaluator::new();
                            if let Ok(crate::evaluator::ConstValue::Integer(val)) = eval.eval_expr(&c.value) {
                                info.constants.insert(c.name.to_string(), val);
                            }
                        }
                        // Extract structs (generic -> templates, concrete -> field list)
                        if let Item::Struct(s) = import_item {
                            if s.generics.is_some() {
                                // Generic struct - store full AST as template
                                info.struct_templates.insert(s.name.to_string(), s.clone());
                            } else {
                                // Concrete struct - store fields
                                let fields: Vec<(String, crate::types::Type)> = s.fields.iter().filter_map(|f| {
                                    crate::types::Type::from_syn(&f.ty).map(|ty| (f.name.to_string(), ty))
                                }).collect();
                                info.structs.insert(s.name.to_string(), fields);
                            }
                        }
                        if let Item::Enum(e) = import_item {
                            if e.generics.is_some() {
                                info.enum_templates.insert(e.name.to_string(), e.clone());
                            }
                        }
                        if let Item::Impl(i) = import_item {
                            info.impls.push((i.clone(), imported_file.imports.clone()));
                        }
                    }
                    registry.register(info);
                    
                    // Recurse
                    load_imports(&imported_file, registry);
                    
                    // Break the fallback loop as we found the module
                    break;
                } else if let Err(e) = syn::parse_str::<crate::grammar::SaltFile>(&processed) {
                    eprintln!("Warning: Failed to parse imported file {}: {}", found_path, e);
                    // If parsing fails, we probably shouldn't try fallback? 
                    // Or maybe we should if the path ended up pointing to a non-Salt file by accident (unlikely)
                    // Let's assume hard failure on parse error for matched file.
                    break;
                }
            } else {
                // Not found. Try fallback.
                if parts.len() > 1 {
                    parts.pop();
                    // Continue loop to try parent path
                } else {
                    eprintln!("Warning: Could not find imported file: {} (scanned parents)", original_parts.join("."));
                    break;
                }
            }
        }
    }
}

