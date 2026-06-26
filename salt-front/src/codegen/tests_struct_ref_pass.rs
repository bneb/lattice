#[cfg(test)]
mod tests {

    fn compile_program(code: &str) -> Result<String, String> {
        crate::compile(code, false, None, true)
            .map_err(|e| format!("{}", e))
    }

    #[test]
    fn test_nested_struct_pass_by_value() {
        let code = r#"
            package test.struct_pass;
            
            struct Complex { r: f32, i: f32 }
            struct Config { dim: i64 }
            struct TransformerWeights {
                w1: Ptr<f32>,
                seq_len: i64,
                arr: [Complex; 128]
            }
            struct Engine {
                config: Config,
                weights: TransformerWeights
            }
            
            fn forward(cfg: Config, w: TransformerWeights) { }
            
            extern fn malloc(size: i64) -> Ptr<u8>;
            extern fn free(ptr: Ptr<u8>);
            
            fn main() -> i32 {
                let engine = malloc(128) as Ptr<Engine>;
                
                // Test 1: Direct function argument passing
                forward(engine.config, engine.weights);
                
                // Test 2: Local variable assignment
                let w = engine.weights;
                let c = engine.config;
                forward(c, w);
                
                free(engine as Ptr<u8>);
                return 0;
            }
        "#;
        
        // This should fail with "Numeric promotion not supported"
        // until we fix `type_bridge.rs`.
        let result = compile_program(code);
        
        assert!(result.is_ok(), "Failed to pass struct field by value: {}", result.err().unwrap_or_default());
    }
}
