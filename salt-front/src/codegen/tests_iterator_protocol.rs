//! Iterator Protocol Tests (TDD)
//!
//! These tests verify that the compiler correctly lowers `for x in iter`
//! to a while-loop calling `.next()` with Option tag checking.

#[cfg(test)]
mod tests {
    /// Test 1: A struct with next() method should compile when used in a for loop
    #[test]
    fn test_iterator_for_loop_compiles() {
        let code = r#"
            package test.iter_basic;

            enum Option<T> {
                Some(T),
                None
            }

            struct Counter {
                current: i64,
                end: i64
            }

            impl Counter {
                fn new(start: i64, end: i64) -> Counter {
                    return Counter { current: start, end: end };
                }

                fn next(&mut self) -> Option<i64> {
                    if self.current < self.end {
                        let val = self.current;
                        self.current = self.current + 1;
                        return Option::Some(val);
                    }
                    return Option::None;
                }
            }

            fn main() -> i32 {
                let mut sum: i64 = 0;
                let iter = Counter::new(0, 5);
                for x in iter {
                    sum = sum + x;
                }
                return 0;
            }
        "#;
        let result = crate::compile(code, false, None, true);
        assert!(result.is_ok(),
            "Iterator for-loop should compile, got: {}",
            result.err().map(|e| format!("{}", e)).unwrap_or_default());
    }

    /// Test 2: Verify the emitted MLIR contains a method call to 'next'
    #[test]
    fn test_iterator_emits_next_call() {
        let code = r#"
            package test.iter_next;

            enum Option<T> {
                Some(T),
                None
            }

            struct Seq {
                current: i64,
                end: i64
            }

            impl Seq {
                fn new(end: i64) -> Seq {
                    return Seq { current: 0, end: end };
                }

                fn next(&mut self) -> Option<i64> {
                    if self.current < self.end {
                        let val = self.current;
                        self.current = self.current + 1;
                        return Option::Some(val);
                    }
                    return Option::None;
                }
            }

            fn main() -> i32 {
                let iter = Seq::new(3);
                for x in iter {
                    let y = x;
                }
                return 0;
            }
        "#;
        let result = crate::compile(code, false, None, true);
        assert!(result.is_ok(),
            "Iterator with next() should compile, got: {}",
            result.err().map(|e| format!("{}", e)).unwrap_or_default());

        // Verify MLIR output contains a call to the next method
        let mlir = result.unwrap();
        assert!(mlir.contains("next"),
            "MLIR should contain a call to 'next' method. Got:\n{}", mlir);
    }

    /// Test 3: Verify Option tag extraction (discriminant check) appears in MLIR
    #[test]
    fn test_iterator_emits_tag_check() {
        let code = r#"
            package test.iter_tag;

            enum Option<T> {
                Some(T),
                None
            }

            struct Nums {
                current: i64,
                end: i64
            }

            impl Nums {
                fn new(end: i64) -> Nums {
                    return Nums { current: 0, end: end };
                }

                fn next(&mut self) -> Option<i64> {
                    if self.current < self.end {
                        let val = self.current;
                        self.current = self.current + 1;
                        return Option::Some(val);
                    }
                    return Option::None;
                }
            }

            fn main() -> i32 {
                let iter = Nums::new(3);
                for x in iter {
                    let y = x;
                }
                return 0;
            }
        "#;
        let result = crate::compile(code, false, None, true);
        assert!(result.is_ok(),
            "Iterator with tag check should compile, got: {}",
            result.err().map(|e| format!("{}", e)).unwrap_or_default());

        let mlir = result.unwrap();
        // Should contain extractvalue for tag (index 0)
        assert!(mlir.contains("llvm.extractvalue") || mlir.contains("extractvalue"),
            "MLIR should contain extractvalue for Option tag. Got:\n{}", mlir);
        // Should contain a conditional branch (cf.cond_br) for Some/None check
        assert!(mlir.contains("cf.cond_br"),
            "MLIR should contain cf.cond_br for iterator loop. Got:\n{}", mlir);
    }

    /// Test 4: for x in iter should NOT conflict with existing 0..N range loops
    #[test]
    fn test_range_loop_still_works() {
        let code = r#"
            package test.range_still_works;
            fn main() -> i32 {
                let mut sum: i64 = 0;
                for i in 0..10 {
                    sum = sum + i;
                }
                return 0;
            }
        "#;
        let result = crate::compile(code, false, None, true);
        assert!(result.is_ok(),
            "Existing 0..N range loop should still compile, got: {}",
            result.err().map(|e| format!("{}", e)).unwrap_or_default());
    }

    /// Test 5: Empty iterator (start == end) should compile and not loop
    #[test]
    fn test_empty_iterator() {
        let code = r#"
            package test.iter_empty;

            enum Option<T> {
                Some(T),
                None
            }

            struct EmptyIter {
                current: i64,
                end: i64
            }

            impl EmptyIter {
                fn new() -> EmptyIter {
                    return EmptyIter { current: 5, end: 5 };
                }

                fn next(&mut self) -> Option<i64> {
                    if self.current < self.end {
                        let val = self.current;
                        self.current = self.current + 1;
                        return Option::Some(val);
                    }
                    return Option::None;
                }
            }

            fn main() -> i32 {
                let iter = EmptyIter::new();
                for x in iter {
                    return 1;
                }
                return 0;
            }
        "#;
        let result = crate::compile(code, false, None, true);
        assert!(result.is_ok(),
            "Empty iterator should compile, got: {}",
            result.err().map(|e| format!("{}", e)).unwrap_or_default());
    }
}
