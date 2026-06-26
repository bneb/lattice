//! Method Receiver Tests: Self argument must appear in MLIR func.call operands.
//!
//! Bug scenario: When a module has both a free function `fn mark() -> i64` 
//! and an impl method `fn mark(self) -> i64`, resolve_method may return
//! the free function definition (no `self` arg), causing `is_static_method=true`
//! and the receiver value to be dropped from the emitted `func.call` operands.
//!
//! The MLIR then has e.g. `func.call @Foo__mark() : (!struct_Foo) -> i64`
//! — 0 operands but 1 expected — which fails mlir-opt validation.

#[cfg(test)]
mod tests {
    /// Helper: compile Salt source and return MLIR string.
    fn compile_to_mlir(source: &str) -> String {
        crate::compile(source, false, None, true)
            .unwrap_or_else(|e| panic!("Compile failed: {}", e))
    }

    /// Extract the operand string from a func.call line containing the given identifier.
    /// Returns the content between the parentheses after @name(...).
    fn extract_call_operands(mlir: &str, call_name: &str) -> String {
        let search = format!("{}(", call_name);
        for line in mlir.lines() {
            if line.contains("func.call") && line.contains(&search) && !line.contains("func.func") {
                if let Some(idx) = line.find(&search) {
                    let after_name = &line[idx + search.len()..];
                    if let Some(paren_end) = after_name.find(')') {
                        return after_name[..paren_end].to_string();
                    }
                }
            }
        }
        panic!("No func.call containing '{}' found in MLIR:\n{}", call_name, mlir);
    }

    // =========================================================================
    // Test 1: Instance method call emits receiver operand (no name collision)
    // =========================================================================

    #[test]
    fn test_method_call_emits_receiver() {
        let mlir = compile_to_mlir(r#"
            package test::method_receiver

            pub struct Counter { val: i64 }

            impl Counter {
                pub fn get(self) -> i64 { return self.val; }
            }

            fn main() -> i32 {
                let c = Counter { val: 42 };
                let v = c.get();
                return v as i32;
            }
        "#);

        let operands = extract_call_operands(&mlir, "Counter__get");
        assert!(
            !operands.trim().is_empty(),
            "Instance method Counter::get must have receiver operand.\nOperands: '{}'\nMLIR:\n{}",
            operands, mlir
        );
    }

    // =========================================================================
    // Test 2: Method shadowed by free function — receiver must still be emitted
    // This is the EXACT bug from arena.salt: pub fn mark() AND impl Arena { pub fn mark(self) }
    // =========================================================================

    #[test]
    fn test_method_with_same_name_as_free_fn_emits_receiver() {
        let mlir = compile_to_mlir(r#"
            package test::shadow_method

            extern fn do_mark() -> i64;

            pub fn mark() -> i64 {
                return do_mark();
            }

            pub struct Arena { cap: i64 }

            impl Arena {
                pub fn mark(self) -> i64 { return do_mark(); }
            }

            fn main() -> i32 {
                let a = Arena { cap: 1024 };
                let m = a.mark();
                return m as i32;
            }
        "#);

        let operands = extract_call_operands(&mlir, "Arena__mark");
        assert!(
            !operands.trim().is_empty(),
            "Arena::mark() must emit receiver even when free function 'mark' exists.\nOperands: '{}'\nMLIR:\n{}",
            operands, mlir
        );
    }

    // =========================================================================
    // Test 3: Method with args, shadowed by free function — all operands present
    // Mirrors arena.salt: pub fn reset_to(m: i64) AND impl Arena { pub fn reset_to(self, m: i64) }
    // =========================================================================

    #[test]
    fn test_method_with_args_shadowed_by_free_fn() {
        let mlir = compile_to_mlir(r#"
            package test::shadow_method_args

            extern fn do_reset(m: i64);

            pub fn reset_to(m: i64) {
                do_reset(m);
            }

            pub struct Arena { cap: i64 }

            impl Arena {
                pub fn reset_to(self, m: i64) { do_reset(m); }
            }

            fn main() -> i32 {
                let a = Arena { cap: 1024 };
                a.reset_to(42);
                return 0;
            }
        "#);

        let operands = extract_call_operands(&mlir, "Arena__reset_to");
        let operand_count = if operands.trim().is_empty() { 0 } else { operands.split(',').count() };
        assert_eq!(
            operand_count, 2,
            "Arena::reset_to(self, m) must emit 2 operands (self + m), got {}.\nOperands: '{}'\nMLIR:\n{}",
            operand_count, operands, mlir
        );
    }
}
