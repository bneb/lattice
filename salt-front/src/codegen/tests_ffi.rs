#[cfg(test)]
mod tests {
    use crate::grammar::SaltFile;
    use crate::codegen::context::CodegenContext;

    fn compile_lib_to_mlir(source: &str) -> String {
        let file: SaltFile = syn::parse_str(source).unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        ctx.lib_mode = true;
        ctx.scan_defs_from_file(&file, true).unwrap();
        ctx.drive_codegen().unwrap()
    }

    #[test]
    fn test_export_emits_c_interface() {
        let code = r#"
            @export
            fn public_ffi_function(x: i32) -> i32 {
                return x + 1;
            }
        "#;
        let mlir = compile_lib_to_mlir(code);
        
        assert!(mlir.contains("func.func public @public_ffi_function"));
        assert!(mlir.contains("llvm.emit_c_interface"));
    }

    #[test]
    #[should_panic(expected = "which is not FFI-safe")]
    fn test_export_rejects_string_arg() {
        let code = r#"
            @export
            fn bad_export(s: String) {
                println(s);
            }
        "#;
        compile_lib_to_mlir(code);
    }

    #[test]
    #[should_panic(expected = "which is not FFI-safe")]
    fn test_extern_rejects_string_ret() {
        let code = r#"
            extern fn get_string() -> String;
        "#;
        compile_lib_to_mlir(code);
    }

    #[test]
    #[should_panic(expected = "which is not FFI-safe")]
    fn test_extern_rejects_string_callback() {
        let code = r#"
            extern fn register_callback(cb: fn(String) -> i32);
        "#;
        compile_lib_to_mlir(code);
    }
}
