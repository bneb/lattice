//! [FACET L1] TDD tests for Metal Shading Language (MSL) codegen.
//!
//! Tests verify that `@shader` Salt functions compile correctly to MSL text
//! and that the host glue LLVM IR is properly generated.

mod tests {
    use crate::grammar::SaltFile;
    use crate::codegen::context::CodegenContext;
    use crate::grammar::attr::{extract_shader_kind, extract_workgroup_size, Attribute};

    // =========================================================================
    // Section 1: Attribute detection
    // =========================================================================

    #[test]
    fn test_shader_attribute_detected() {
        // Parse @shader(compute) attribute
        let attr: Attribute = syn::parse_str("@ shader ( compute )").unwrap();
        assert_eq!(attr.name.to_string(), "shader");
        assert_eq!(attr.args[0].to_string(), "compute");
    }

    #[test]
    fn test_shader_kind_extraction_compute() {
        let attr: Attribute = syn::parse_str("@ shader ( compute )").unwrap();
        let kind = extract_shader_kind(&[attr]);
        assert_eq!(kind, Some("compute".to_string()));
    }

    #[test]
    fn test_shader_kind_extraction_vertex() {
        let attr: Attribute = syn::parse_str("@ shader ( vertex )").unwrap();
        let kind = extract_shader_kind(&[attr]);
        assert_eq!(kind, Some("vertex".to_string()));
    }

    #[test]
    fn test_shader_kind_default_compute() {
        let attr: Attribute = syn::parse_str("@ shader").unwrap();
        let kind = extract_shader_kind(&[attr]);
        assert_eq!(kind, Some("compute".to_string()));
    }

    #[test]
    fn test_shader_no_attribute() {
        let kind = extract_shader_kind(&[]);
        assert_eq!(kind, None);
    }

    #[test]
    fn test_shader_workgroup_size_default() {
        let attr: Attribute = syn::parse_str("@ shader ( compute )").unwrap();
        let size = extract_workgroup_size(&[attr]);
        assert_eq!(size, 64);
    }

    // =========================================================================
    // Section 2: Type mapping (Salt → MSL)
    // =========================================================================

    #[test]
    fn test_shader_type_mapping_i32() {
        use crate::grammar::SynType;
        use crate::codegen::shader::salt_type_to_msl;
        let ty: SynType = syn::parse_str("i32").unwrap();
        assert_eq!(salt_type_to_msl(&ty).unwrap(), "int");
    }

    #[test]
    fn test_shader_type_mapping_f32() {
        use crate::grammar::SynType;
        use crate::codegen::shader::salt_type_to_msl;
        let ty: SynType = syn::parse_str("f32").unwrap();
        assert_eq!(salt_type_to_msl(&ty).unwrap(), "float");
    }

    #[test]
    fn test_shader_type_mapping_u32() {
        use crate::grammar::SynType;
        use crate::codegen::shader::salt_type_to_msl;
        let ty: SynType = syn::parse_str("u32").unwrap();
        assert_eq!(salt_type_to_msl(&ty).unwrap(), "uint");
    }

    #[test]
    fn test_shader_type_mapping_u8() {
        use crate::grammar::SynType;
        use crate::codegen::shader::salt_type_to_msl;
        let ty: SynType = syn::parse_str("u8").unwrap();
        assert_eq!(salt_type_to_msl(&ty).unwrap(), "uchar");
    }

    #[test]
    fn test_shader_type_mapping_ptr_f32() {
        use crate::grammar::SynType;
        use crate::codegen::shader::salt_type_to_msl;
        let ty: SynType = syn::parse_str("Ptr<f32>").unwrap();
        assert_eq!(salt_type_to_msl(&ty).unwrap(), "device float*");
    }

    // =========================================================================
    // Section 3: MSL parameter generation
    // =========================================================================

    #[test]
    fn test_shader_buffer_bindings() {
        use crate::grammar::SynType;
        use crate::codegen::shader::salt_type_to_msl_param;
        let ty: SynType = syn::parse_str("Ptr<f32>").unwrap();
        let param = salt_type_to_msl_param(&ty, 0).unwrap();
        assert!(param.contains("device float*"), "Should have device float*, got: {}", param);
        assert!(param.contains("[[buffer(0)]]"), "Should have buffer binding, got: {}", param);
    }

    #[test]
    fn test_shader_scalar_binding() {
        use crate::grammar::SynType;
        use crate::codegen::shader::salt_type_to_msl_param;
        let ty: SynType = syn::parse_str("i32").unwrap();
        let param = salt_type_to_msl_param(&ty, 2).unwrap();
        assert!(param.contains("constant int&"), "Scalar should be constant ref, got: {}", param);
        assert!(param.contains("[[buffer(2)]]"), "Should have buffer(2), got: {}", param);
    }

    // =========================================================================
    // Section 4: Expression translation
    // =========================================================================

    #[test]
    fn test_shader_arithmetic() {
        use crate::codegen::shader::expr_to_msl;
        let expr: syn::Expr = syn::parse_str("a + b * c").unwrap();
        let msl = expr_to_msl(&expr);
        assert!(msl.contains("+"), "Should contain +, got: {}", msl);
        assert!(msl.contains("*"), "Should contain *, got: {}", msl);
    }

    #[test]
    fn test_shader_comparison() {
        use crate::codegen::shader::expr_to_msl;
        let expr: syn::Expr = syn::parse_str("id < len").unwrap();
        let msl = expr_to_msl(&expr);
        assert!(msl.contains("<"), "Should contain <, got: {}", msl);
        assert!(msl.contains("id"), "Should contain id, got: {}", msl);
        assert!(msl.contains("len"), "Should contain len, got: {}", msl);
    }

    #[test]
    fn test_shader_thread_id() {
        use crate::codegen::shader::expr_to_msl;
        let expr: syn::Expr = syn::parse_str("thread_id()").unwrap();
        let msl = expr_to_msl(&expr);
        assert_eq!(msl, "tid", "thread_id() should map to tid, got: {}", msl);
    }

    #[test]
    fn test_shader_method_read_at() {
        use crate::codegen::shader::expr_to_msl;
        let expr: syn::Expr = syn::parse_str("a.read_at(id)").unwrap();
        let msl = expr_to_msl(&expr);
        assert!(msl.contains("a[id]") || msl.contains("a [id]"), 
            "read_at should map to array index, got: {}", msl);
    }

    #[test]
    fn test_shader_method_write_at() {
        use crate::codegen::shader::expr_to_msl;
        let expr: syn::Expr = syn::parse_str("out.write_at(id, val)").unwrap();
        let msl = expr_to_msl(&expr);
        assert!(msl.contains("out[id]") || msl.contains("out [id]"), 
            "write_at should map to array index assignment, got: {}", msl);
        assert!(msl.contains("= val"), "Should assign val, got: {}", msl);
    }

    // =========================================================================
    // Section 5: Full MSL generation
    // =========================================================================

    #[test]
    fn test_shader_emits_msl_kernel() {
        use crate::codegen::shader::generate_msl;
        // Parse a minimal shader function
        let file: SaltFile = syn::parse_str(r#"
            @shader(compute)
            fn add_one(data: Ptr<f32>, len: i32) {
                let id = thread_id();
                if id < len {
                    data.write_at(id, data.read_at(id) + 1.0);
                }
            }
            fn main() -> i32 { return 0; }
        "#).unwrap();

        // Extract the shader function
        let shader_fn = file.items.iter().find_map(|item| {
            if let crate::grammar::Item::Fn(f) = item {
                if f.name == "add_one" { return Some(f); }
            }
            None
        }).expect("Should find add_one function");

        let msl = generate_msl(shader_fn, "compute", 64).unwrap();
        assert!(msl.contains("kernel void add_one"), 
            "Should contain kernel declaration, got:\n{}", msl);
        assert!(msl.contains("#include <metal_stdlib>"), 
            "Should include metal header, got:\n{}", msl);
        assert!(msl.contains("using namespace metal"), 
            "Should use metal namespace, got:\n{}", msl);
        assert!(msl.contains("thread_position_in_grid"), 
            "Should have thread position param, got:\n{}", msl);
        assert!(msl.contains("[[buffer("), 
            "Should have buffer bindings, got:\n{}", msl);
    }

    // =========================================================================
    // Section 6: Host glue LLVM IR
    // =========================================================================

    #[test]
    fn test_shader_host_glue() {
        use crate::codegen::shader::emit_shader_fn;
        let file: SaltFile = syn::parse_str(r#"
            @shader(compute)
            fn vector_add(a: Ptr<f32>, b: Ptr<f32>, out: Ptr<f32>, len: i32) {
                let id = thread_id();
                if id < len {
                    out.write_at(id, a.read_at(id) + b.read_at(id));
                }
            }
            fn main() -> i32 { return 0; }
        "#).unwrap();

        let shader_fn = file.items.iter().find_map(|item| {
            if let crate::grammar::Item::Fn(f) = item {
                if f.name == "vector_add" { return Some(f); }
            }
            None
        }).expect("Should find vector_add function");

        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        
        let glue = ctx.with_lowering_ctx(|lctx| emit_shader_fn(lctx, shader_fn)).unwrap();
        
        // Should emit global MSL string constant
        assert!(glue.contains("llvm.mlir.global internal constant @__shader_msl_vector_add"), 
            "Should contain global MSL string, got:\n{}", glue);
        // Should emit accessor function
        assert!(glue.contains("func.func @get_shader_msl_vector_add"), 
            "Should contain accessor function, got:\n{}", glue);
        // Should contain kernel void in the embedded string
        assert!(glue.contains("kernel void vector_add"), 
            "Embedded MSL should contain kernel declaration, got:\n{}", glue);
    }
}
