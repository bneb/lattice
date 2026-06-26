// =============================================================================
// TDD: v_load / v_store / v_fma / v_hsum SIMD intrinsics
// =============================================================================
// Tests that the MLIR codegen emits correct vector<4xf32> operations
// for the WASM SIMD v128 pipeline.

#[cfg(test)]
mod tests {

    fn compile_program(code: &str) -> Result<String, String> {
        crate::compile(code, false, None, true)
            .map_err(|e| format!("{}", e))
    }

    #[test]
    fn test_v_load_emits_vector_load() {
        let code = r#"
            package test
            
            extern fn malloc(size: i64) -> Ptr<u8>;
            extern fn free(ptr: Ptr<u8>);

            fn main() -> i32 {
                let buf = malloc(64) as Ptr<f32>;
                let vec: Vector4f32 = v_load(buf, 0);
                let sum: f32 = v_hsum(vec);
                free(buf as Ptr<u8>);
                return 0;
            }
        "#;
        let result = compile_program(code);
        assert!(result.is_ok(), "v_load compilation failed: {}", result.err().unwrap_or_default());
        let mlir = result.unwrap();
        assert!(
            mlir.contains("vector<4xf32>"),
            "v_load must emit vector<4xf32> ops, got:\n{}", mlir
        );
    }

    #[test]
    fn test_v_fma_emits_vector_fma() {
        let code = r#"
            package test
            
            extern fn malloc(size: i64) -> Ptr<u8>;
            extern fn free(ptr: Ptr<u8>);

            fn main() -> i32 {
                let buf = malloc(64) as Ptr<f32>;
                let a: Vector4f32 = v_load(buf, 0);
                let b: Vector4f32 = v_load(buf, 4);
                let zero: Vector4f32 = v_broadcast(0.0f32);
                let result: Vector4f32 = v_fma(zero, a, b);
                let s: f32 = v_hsum(result);
                free(buf as Ptr<u8>);
                return 0;
            }
        "#;
        let result = compile_program(code);
        assert!(result.is_ok(), "v_fma compilation failed: {}", result.err().unwrap_or_default());
        let mlir = result.unwrap();
        assert!(
            mlir.contains("vector.fma"),
            "v_fma must emit vector.fma, got:\n{}", mlir
        );
    }

    #[test]
    fn test_v_store_emits_vector_store() {
        let code = r#"
            package test
            
            extern fn malloc(size: i64) -> Ptr<u8>;
            extern fn free(ptr: Ptr<u8>);

            fn main() -> i32 {
                let buf = malloc(64) as Ptr<f32>;
                let vec: Vector4f32 = v_load(buf, 0);
                v_store(buf, 4, vec);
                free(buf as Ptr<u8>);
                return 0;
            }
        "#;
        let result = compile_program(code);
        assert!(result.is_ok(), "v_store compilation failed: {}", result.err().unwrap_or_default());
        let mlir = result.unwrap();
        assert!(
            mlir.contains("llvm.store") && mlir.contains("vector<4xf32>"),
            "v_store must emit llvm.store of vector<4xf32>, got:\n{}", mlir
        );
    }
}
