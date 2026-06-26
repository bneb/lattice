use std::fs;

use crate::cli_build;


pub struct CliConfig {
    pub path: String,
    pub output_path: Option<String>,
    pub release_mode: bool,
    pub skip_scan: bool,
    pub binary_mode: bool,
    pub object_mode: bool,
    pub disable_alias_scopes: bool,
    pub no_verify: bool,
    pub lib_mode: bool,
    pub sip_mode: bool,
    pub debug_info: bool,
    pub emit_sir: bool,
    pub target_name: Option<String>,
}

pub fn parse_args(args: Vec<String>) -> anyhow::Result<Option<CliConfig>> {
    let mut path_opt: Option<String> = None;
    let mut output_path: Option<String> = None;
    let mut release_mode = false;
    let mut skip_scan = false;
    let mut binary_mode = false;
    let mut object_mode = false;
    let mut disable_alias_scopes = false;
    #[cfg_attr(not(debug_assertions), allow(unused_mut))]
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
            println!("Usage: saltc <file.salt> [options]");
            println!();
            println!("Options:");
            println!("  -o <file>          Output MLIR file");
            println!("  --release          Enable optimizations");
            println!("  --binary           Produce native Mach-O/ELF binary via Iron Driver");
            println!("  -c                 Produce .o object file (like clang -c)");
            println!("  --target <triple>  Target: macos, linux-arm64, keuos, keuos-x86_64");
            println!("  --lib              Library mode (no main entry point required)");
            println!("  --sip              Mode B SIP safety enforcement (rejects raw pointer creation)");
            println!("  --skip-scan        Skip import scanning");
            println!("  -g, --debug-info   Emit DWARF debug info (MLIR loc annotations)");
            println!("  --emit-sir         Emit SIR (Salt Intermediate Representation) as JSON");
            println!("  --disable-alias-scopes  Suppress LLVM alias scope metadata");
            println!("  --danger-no-verify  Disable Z3 contract verification (verification is on by default)");
            println!("  --explain <code>        Show detailed explanation of an error code");
            println!("  --version               Show version information");
            println!("  --help                  Show this help message");
            return Ok(None);
        } else if arg == "--version" || arg == "-V" {
            println!("saltc {}", env!("CARGO_PKG_VERSION"));
            return Ok(None);
        } else if arg == "--explain" {
            if i + 1 < args.len() {
                cli_build::explain_error_code(&args[i + 1]);
                return Ok(None);
            } else {
                anyhow::bail!("[E004] --explain requires an error code argument (e.g. --explain E001)");
            }
        } else if arg == "--skip-scan" {
            skip_scan = true;
        } else if arg == "--bench" {
            release_mode = true; 
        } else if arg == "--binary" {
            binary_mode = true;
            release_mode = true;
        } else if arg == "-c" {
            object_mode = true;
            release_mode = true;
        } else if arg == "--target" {
            if i + 1 < args.len() {
                target_name = Some(args[i+1].clone());
                i += 1;
            } else {
                anyhow::bail!("[E004] --target requires an argument (e.g. keuos, macos, linux-arm64)");
            }
        } else if arg == "--disable-alias-scopes" {
            disable_alias_scopes = true;
        } else if arg == "--danger-no-verify" {
            #[cfg(not(debug_assertions))]
            {
                panic!("[E007] FATAL: Z3 verification cannot be disabled in release builds.");
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
            lib_mode = true;
        } else if arg == "--emit-sir" {
            emit_sir = true;
        } else if arg == "-g" || arg == "--debug-info" {
            debug_info = true;
        } else if arg == "-o" {
            if i + 1 < args.len() {
                output_path = Some(args[i+1].clone());
                i += 1;
            } else {
                anyhow::bail!("[E004] -o requires an argument");
            }
        } else if arg.starts_with("-") {
            anyhow::bail!("[E004] Unknown argument: {}", arg);
        } else {
            path_opt = Some(arg.clone());
        }
        i += 1;
    }

    let path = match path_opt {
        Some(p) => p,
        None => {
            println!("Usage: saltc <file.salt> [options]");
            return Ok(None);
        }
    };

    Ok(Some(CliConfig {
        path,
        output_path,
        release_mode,
        skip_scan,
        binary_mode,
        object_mode,
        disable_alias_scopes,
        no_verify,
        lib_mode,
        sip_mode,
        debug_info,
        emit_sir,
        target_name,
    }))
}



pub fn run_cli(args: Vec<String>) -> anyhow::Result<()> {
    let config = match parse_args(args)? {
        Some(c) => c,
        None => return Ok(()),
    };

    let code = fs::read_to_string(&config.path).map_err(|e| {
        anyhow::anyhow!("[E001] Failed to read source file '{}': {}", config.path, e)
    })?;

    let processed = crate::preprocess(&code);
    let mut file: crate::grammar::SaltFile = syn::parse_str(&processed)?;

    let mut registry = crate::registry::Registry::new();
    let main_pkg = if let Some(pkg) = &file.package {
        pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(".")
    } else {
        "main".to_string()
    };
    registry.register(crate::registry::ModuleInfo::new(&main_pkg));

    let prelude_imports = [
        "use std::core::ptr::*;",
    ];
    for import_str in &prelude_imports {
        let processed = crate::preprocess(import_str);
        if let Ok(parsed) = syn::parse_str::<crate::grammar::SaltFile>(&processed) {
            file.imports.extend(parsed.imports);
        }
    }

    load_imports(&file, &mut registry);

    match crate::compile_ast(&mut file, config.release_mode, Some(&registry), config.skip_scan, config.disable_alias_scopes, config.no_verify, config.lib_mode, config.sip_mode, config.debug_info, &config.path) {
        Ok(mlir) => {
            let basename = std::path::Path::new(&config.path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");

            if config.emit_sir {
                cli_build::emit_sir_file(&file, basename, config.output_path.as_deref());
            }

            if config.binary_mode {
                cli_build::handle_binary_synthesis(&mlir, basename, &config);
            } else if config.object_mode {
                cli_build::handle_object_synthesis(&mlir, basename, &config);
            } else {
                let out_p = config.output_path.clone().unwrap_or_else(|| "out.mlir".to_string());
                fs::write(&out_p, &mlir).map_err(|e| anyhow::anyhow!("[E001] Failed to write MLIR: {}", e))?;
                eprintln!("✅ MLIR compiled successfully.");
                eprintln!("📄 Wrote MLIR to: {}", out_p);
            }
        }
        Err(e) => {
            eprintln!("[E003] Compilation failed:");
            eprintln!("{}", e);
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
                    eprintln!("[E008] Warning: Failed to parse imported file {}: {}", found_path, e);
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
                    eprintln!("[E008] Warning: Could not find imported file: {} (scanned parents)", original_parts.join("."));
                    break;
                }
            }
        }
    }
}

