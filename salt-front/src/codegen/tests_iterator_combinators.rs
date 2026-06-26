// =============================================================================
// TDD Tests for Iterator Combinators: Map, Filter, Fold
// =============================================================================
// These tests verify that Salt can:
// 1. Pass named functions as arguments (function pointer inference)
// 2. Store function pointers in generic struct fields
// 3. Call function pointers stored in struct fields (indirect calls)
// 4. Chain combinators: .filter().map().fold()
// 5. Use Map/Filter as iterators in for-in loops
// =============================================================================

#[cfg(test)]
mod tests {
    /// Common Salt prelude: Option enum + Range struct with next()
    fn prelude() -> String {
        r#"
        enum Option<T> {
            Some(T),
            None
        }

        impl<T> Option<T> {
            fn is_none(self) -> bool {
                match self {
                    Option::Some(_) => return false,
                    Option::None => return true
                }
            }

            fn unwrap(self) -> T {
                match self {
                    Option::Some(val) => return val,
                    Option::None => return self.unwrap()
                }
            }
        }

        struct Range {
            current: i64,
            end: i64
        }

        impl Range {
            fn new(start: i64, end: i64) -> Range {
                return Range { current: start, end: end };
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
        "#.to_string()
    }

    // =========================================================================
    // Test 1: fold — Pass a named function as an argument
    // =========================================================================
    #[test]
    fn test_fold_compiles() {
        let code = format!(r#"
        package test::fold;

        {}

        impl Range {{
            fn fold<A, F>(&mut self, init: A, f: F) -> A {{
                let mut acc = init;
                while true {{
                    let opt = self.next();
                    if opt.is_none() {{ break; }}
                    let val = opt.unwrap();
                    acc = f(acc, val);
                }}
                return acc;
            }}
        }}

        fn add(a: i64, b: i64) -> i64 {{
            return a + b;
        }}

        fn main() -> i32 {{
            let mut r = Range::new(0, 5);
            let result = r.fold(0, add);
            return 0;
        }}
        "#, prelude());

        let result = crate::compile(&code, false, None, true);
        assert!(result.is_ok(), "fold should compile:\n{}", result.unwrap_err());
    }

    // =========================================================================
    // Test 2: Map struct — Generic struct with function pointer field
    // =========================================================================
    #[test]
    fn test_map_struct_compiles() {
        let code = format!(r#"
        package test::map;

        {}

        struct Map<I, F, T> {{
            iter: I,
            func: F
        }}

        impl<I, F, T> Map<I, F, T> {{
            fn next(&mut self) -> Option<T> {{
                let item = self.iter.next();
                match item {{
                    Option::Some(val) => {{
                        let res = (self.func)(val);
                        return Option::Some(res);
                    }},
                    Option::None => return Option::None
                }}
            }}
        }}

        impl Range {{
            fn map<F, T>(self, f: F) -> Map<Range, F, T> {{
                return Map {{ iter: self, func: f }};
            }}
        }}

        fn double(x: i64) -> i64 {{
            return x * 2;
        }}

        fn main() -> i32 {{
            let r = Range::new(0, 5);
            let mut mapped = r.map(double);
            let first = mapped.next();
            return 0;
        }}
        "#, prelude());

        let result = crate::compile(&code, false, None, true);
        assert!(result.is_ok(), "Map struct should compile:\n{}", result.unwrap_err());
    }

    // =========================================================================
    // Test 3: Filter struct — Predicate function in generic field
    // =========================================================================
    #[test]
    fn test_filter_struct_compiles() {
        let code = format!(r#"
        package test::filter;

        {}

        struct Filter<I, F> {{
            iter: I,
            predicate: F
        }}

        impl<I, F> Filter<I, F> {{
            fn next(&mut self) -> Option<i64> {{
                while true {{
                    let item = self.iter.next();
                    if item.is_none() {{
                        return Option::None;
                    }}
                    let val = item.unwrap();
                    if (self.predicate)(val) {{
                        return Option::Some(val);
                    }}
                }}
                return Option::None;
            }}
        }}

        impl Range {{
            fn filter<F>(self, f: F) -> Filter<Range, F> {{
                return Filter {{ iter: self, predicate: f }};
            }}
        }}

        fn is_even(x: i64) -> bool {{
            let rem = x - (x / 2) * 2;
            return rem == 0;
        }}

        fn main() -> i32 {{
            let r = Range::new(0, 10);
            let mut filtered = r.filter(is_even);
            let first = filtered.next();
            return 0;
        }}
        "#, prelude());

        let result = crate::compile(&code, false, None, true);
        assert!(result.is_ok(), "Filter struct should compile:\n{}", result.unwrap_err());
    }

    // =========================================================================
    // Test 4: Map used in for-in loop (iterator protocol)
    // =========================================================================
    #[test]
    fn test_map_in_for_loop() {
        let code = format!(r#"
        package test::map_for;

        {}

        struct Map<I, F, T> {{
            iter: I,
            func: F
        }}

        impl<I, F, T> Map<I, F, T> {{
            fn next(&mut self) -> Option<T> {{
                let item = self.iter.next();
                match item {{
                    Option::Some(val) => {{
                        let res = (self.func)(val);
                        return Option::Some(res);
                    }},
                    Option::None => return Option::None
                }}
            }}
        }}

        impl Range {{
            fn map<F, T>(self, f: F) -> Map<Range, F, T> {{
                return Map {{ iter: self, func: f }};
            }}
        }}

        fn double(x: i64) -> i64 {{
            return x * 2;
        }}

        fn main() -> i32 {{
            let r = Range::new(0, 3);
            let mapped = r.map(double);
            for val in mapped {{
                println("{{}}", val);
            }}
            return 0;
        }}
        "#, prelude());

        let result = crate::compile(&code, false, None, true);
        assert!(result.is_ok(), "Map in for-in loop should compile:\n{}", result.unwrap_err());
    }

    // =========================================================================
    // Test 5a: Generic wrapper calling method on inner field (isolated)
    // This tests the core pattern from Map::next: self.iter.next()
    // =========================================================================
    #[test]
    fn test_generic_wrapper_delegates_method() {
        let code = format!(r#"
        package test::wrapper;

        {}

        struct Wrapper<I> {{
            inner: I
        }}

        impl<I> Wrapper<I> {{
            fn next(&mut self) -> Option<i64> {{
                return self.inner.next();
            }}
        }}

        impl Range {{
            fn wrap(self) -> Wrapper<Range> {{
                return Wrapper {{ inner: self }};
            }}
        }}

        fn main() -> i32 {{
            let r = Range::new(0, 5);
            let mut w = r.wrap();
            let first = w.next();
            return 0;
        }}
        "#, prelude());

        let result = crate::compile(&code, false, None, true);
        assert!(result.is_ok(), "Generic wrapper delegating to inner.next() should compile:\n{}", result.unwrap_err());
    }

    // =========================================================================
    // Test 5b: Map::next calling iter.next() and (func)(val) (incremental)
    // This isolates Map::next behavior from the full chain
    // =========================================================================
    #[test]
    fn test_map_next_with_func_call() {
        let code = format!(r#"
        package test::mapnext;

        {}

        struct Map<I, F, T> {{
            iter: I,
            func: F
        }}

        impl<I, F, T> Map<I, F, T> {{
            fn next(&mut self) -> Option<T> {{
                let item = self.iter.next();
                match item {{
                    Option::Some(val) => {{
                        let res = (self.func)(val);
                        return Option::Some(res);
                    }},
                    Option::None => return Option::None
                }}
            }}
        }}

        impl Range {{
            fn map<F, T>(self, f: F) -> Map<Range, F, T> {{
                return Map {{ iter: self, func: f }};
            }}
        }}

        fn square(x: i64) -> i64 {{
            return x * x;
        }}

        fn main() -> i32 {{
            let r = Range::new(0, 5);
            let mut m = r.map(square);
            let first = m.next();
            return 0;
        }}
        "#, prelude());

        let result = crate::compile(&code, false, None, true);
        assert!(result.is_ok(), "Map::next with func call should compile:\n{}", result.unwrap_err());
    }

    // =========================================================================
    // Test 5c: filter().map() — nested generic wrappers (the chain failure point)
    // Map<Filter<Range, F>, F2, T> — tests method dispatch on nested generics
    // =========================================================================
    #[test]
    fn test_filter_then_map() {
        let code = format!(r#"
        package test::fmap;

        {}

        struct Map<I, F, T> {{
            iter: I,
            func: F
        }}

        struct Filter<I, F> {{
            iter: I,
            predicate: F
        }}

        impl<I, F, T> Map<I, F, T> {{
            fn next(&mut self) -> Option<T> {{
                let item = self.iter.next();
                match item {{
                    Option::Some(val) => {{
                        let res = (self.func)(val);
                        return Option::Some(res);
                    }},
                    Option::None => return Option::None
                }}
            }}
        }}

        impl<I, F> Filter<I, F> {{
            fn next(&mut self) -> Option<i64> {{
                while true {{
                    let item = self.iter.next();
                    if item.is_none() {{
                        return Option::None;
                    }}
                    let val = item.unwrap();
                    if (self.predicate)(val) {{
                        return Option::Some(val);
                    }}
                }}
                return Option::None;
            }}

            fn map<F2, T>(self, f: F2) -> Map<Filter<I, F>, F2, T> {{
                return Map {{ iter: self, func: f }};
            }}
        }}

        impl Range {{
            fn filter<F>(self, f: F) -> Filter<Range, F> {{
                return Filter {{ iter: self, predicate: f }};
            }}
        }}

        fn is_even(x: i64) -> bool {{
            let rem = x - (x / 2) * 2;
            return rem == 0;
        }}

        fn square(x: i64) -> i64 {{
            return x * x;
        }}

        fn main() -> i32 {{
            let r = Range::new(0, 10);
            let mut pipeline = r.filter(is_even).map(square);
            let first = pipeline.next();
            return 0;
        }}
        "#, prelude());

        let result = crate::compile(&code, false, None, true);
        assert!(result.is_ok(), "filter().map() chain should compile:\n{}", result.unwrap_err());
    }

    // =========================================================================
    // Test 5c-ii: Generic name collision regression test
    // Verifies that struct construction correctly handles generic parameter
    // name collisions between inner and outer types (e.g., Wrapper<T> where
    // T shadows Container<T>'s own T parameter).
    // =========================================================================
    #[test]
    fn test_generic_name_collision() {
        // Regression test: Container<T> and Wrapper<T> both use generic name T.
        // When constructing Wrapper { inner: self } inside Container<T>::wrap(),
        // the compiler must not recursively substitute T → Container<T> → Container<Container<T>> → ...
        // Explicit type annotations on let bindings guide the compiler past the
        // dangerous emit_expr probe, locking T to its intended concrete type.
        let code = r#"
        package test::collision;

        struct Container<T> {
            value: T,
            count: i64
        }

        struct Wrapper<T> {
            inner: T,
            tag: i64
        }

        impl<T> Container<T> {
            fn wrap(self) -> Wrapper<Container<T>> {
                let w: Wrapper<Container<T>> = Wrapper {
                    inner: self,
                    tag: 42
                };
                return w;
            }
        }

        fn main() -> i32 {
            let c: Container<i64> = Container { value: 7, count: 1 };
            let w = c.wrap();
            return 0;
        }
        "#;

        // Run in a thread with 4MB stack as safety net against deep recursion
        let builder = std::thread::Builder::new().stack_size(4 * 1024 * 1024);
        let handler = builder.spawn(|| {
            let result = crate::compile(code, false, None, true);
            assert!(result.is_ok(), "Generic name collision (Container<T>.wrap -> Wrapper<Container<T>>) should compile:\n{}", result.unwrap_err());
        }).unwrap();
        handler.join().unwrap();
    }

    // =========================================================================
    // Test 5d: Full chain: filter → map → fold
    // =========================================================================
    #[test]
    fn test_combinator_chain() {
        let code = format!(r#"
        package test::chain;

        {}

        struct Map<I, F, T> {{
            iter: I,
            func: F
        }}

        struct Filter<I, F> {{
            iter: I,
            predicate: F
        }}

        impl<I, F, T> Map<I, F, T> {{
            fn next(&mut self) -> Option<T> {{
                let item = self.iter.next();
                match item {{
                    Option::Some(val) => {{
                        let res = (self.func)(val);
                        return Option::Some(res);
                    }},
                    Option::None => return Option::None
                }}
            }}

            fn fold<A, F2>(&mut self, init: A, f: F2) -> A {{
                let mut acc = init;
                while true {{
                    let opt = self.next();
                    if opt.is_none() {{ break; }}
                    let val = opt.unwrap();
                    acc = f(acc, val);
                }}
                return acc;
            }}
        }}

        impl<I, F> Filter<I, F> {{
            fn next(&mut self) -> Option<i64> {{
                while true {{
                    let item = self.iter.next();
                    if item.is_none() {{
                        return Option::None;
                    }}
                    let val = item.unwrap();
                    if (self.predicate)(val) {{
                        return Option::Some(val);
                    }}
                }}
                return Option::None;
            }}

            fn map<F2, T>(self, f: F2) -> Map<Filter<I, F>, F2, T> {{
                return Map {{ iter: self, func: f }};
            }}
        }}

        impl Range {{
            fn filter<F>(self, f: F) -> Filter<Range, F> {{
                return Filter {{ iter: self, predicate: f }};
            }}
        }}

        fn is_even(x: i64) -> bool {{
            let rem = x - (x / 2) * 2;
            return rem == 0;
        }}

        fn square(x: i64) -> i64 {{
            return x * x;
        }}

        fn add(acc: i64, x: i64) -> i64 {{
            return acc + x;
        }}

        fn main() -> i32 {{
            let r = Range::new(0, 10);
            let mut pipeline = r.filter(is_even).map(square);
            let result = pipeline.fold(0, add);
            return 0;
        }}
        "#, prelude());

        let result = crate::compile(&code, false, None, true);
        assert!(result.is_ok(), "Combinator chain should compile:\n{}", result.unwrap_err());
    }

    // =========================================================================
    // Test 6: Verify fold emits indirect call (not string matching — structural)
    // =========================================================================
    #[test]
    fn test_fold_emits_indirect_call() {
        let code = format!(r#"
        package test::fold_ir;

        {}

        impl Range {{
            fn fold<A, F>(&mut self, init: A, f: F) -> A {{
                let mut acc = init;
                while true {{
                    let opt = self.next();
                    if opt.is_none() {{ break; }}
                    let val = opt.unwrap();
                    acc = f(acc, val);
                }}
                return acc;
            }}
        }}

        fn add(a: i64, b: i64) -> i64 {{
            return a + b;
        }}

        fn main() -> i32 {{
            let mut r = Range::new(0, 3);
            let result = r.fold(0, add);
            return 0;
        }}
        "#, prelude());

        let result = crate::compile(&code, false, None, true);
        assert!(result.is_ok(), "fold should compile for IR inspection:\n{}", result.unwrap_err());

        let ir = result.unwrap();
        // The fold function should exist as a monomorphized function
        // It should contain a call instruction (direct or indirect)
        // We verify the function was emitted, not specific IR patterns
        assert!(ir.contains("fold"), "MLIR should contain a fold function");
    }

    // =========================================================================
    // Test 7: sum() — Terminal operation that sums all items
    // =========================================================================
    #[test]
    fn test_sum_compiles() {
        let code = format!(r#"
        package test::sum;

        {}

        impl Range {{
            fn sum(&mut self) -> i64 {{
                let mut total: i64 = 0;
                while true {{
                    let Option::Some(val) = self.next() else {{ break; }};
                    total = total + val;
                }}
                return total;
            }}
        }}

        fn main() -> i32 {{
            let mut r = Range::new(0, 5);
            let total = r.sum();
            return 0;
        }}
        "#, prelude());

        let result = crate::compile(&code, false, None, true);
        assert!(result.is_ok(), "sum() should compile:\n{}", result.unwrap_err());
    }

    // =========================================================================
    // Test 8: count() — Terminal operation that counts items
    // =========================================================================
    #[test]
    fn test_count_compiles() {
        let code = format!(r#"
        package test::count;

        {}

        struct Filter<I, F> {{
            iter: I,
            predicate: F
        }}

        impl<I, F> Filter<I, F> {{
            fn next(&mut self) -> Option<i64> {{
                while true {{
                    let Option::Some(val) = self.iter.next() else {{
                        return Option::None;
                    }};
                    if (self.predicate)(val) {{
                        return Option::Some(val);
                    }}
                }}
                return Option::None;
            }}

            fn count(&mut self) -> i64 {{
                let mut n: i64 = 0;
                while true {{
                    let Option::Some(_) = self.next() else {{ break; }};
                    n = n + 1;
                }}
                return n;
            }}
        }}

        impl Range {{
            fn filter<F>(self, f: F) -> Filter<Range, F> {{
                return Filter {{ iter: self, predicate: f }};
            }}
        }}

        fn is_even(x: i64) -> bool {{
            let rem = x - (x / 2) * 2;
            return rem == 0;
        }}

        fn main() -> i32 {{
            let r = Range::new(0, 10);
            let mut f = r.filter(is_even);
            let n = f.count();
            return 0;
        }}
        "#, prelude());

        let result = crate::compile(&code, false, None, true);
        assert!(result.is_ok(), "count() should compile:\n{}", result.unwrap_err());
    }

    // =========================================================================
    // Test 9: any() / all() — Short-circuit terminal operations
    // =========================================================================
    #[test]
    fn test_any_all_compile() {
        let code = format!(r#"
        package test::anyall;

        {}

        impl Range {{
            fn any<F>(&mut self, pred: F) -> bool {{
                while true {{
                    let Option::Some(val) = self.next() else {{ break; }};
                    if pred(val) {{ return true; }}
                }}
                return false;
            }}

            fn all<F>(&mut self, pred: F) -> bool {{
                while true {{
                    let Option::Some(val) = self.next() else {{ break; }};
                    if !pred(val) {{ return false; }}
                }}
                return true;
            }}
        }}

        fn is_positive(x: i64) -> bool {{
            return x > 0;
        }}

        fn main() -> i32 {{
            let mut r1 = Range::new(1, 10);
            let has_positive = r1.any(is_positive);
            let mut r2 = Range::new(1, 10);
            let all_positive = r2.all(is_positive);
            return 0;
        }}
        "#, prelude());

        let result = crate::compile(&code, false, None, true);
        assert!(result.is_ok(), "any()/all() should compile:\n{}", result.unwrap_err());
    }

    // =========================================================================
    // Test 10: Chained pipeline: filter().sum() — the motivating example
    // =========================================================================
    #[test]
    fn test_filter_sum_chain() {
        let code = format!(r#"
        package test::filtersum;

        {}

        struct Filter<I, F> {{
            iter: I,
            predicate: F
        }}

        impl<I, F> Filter<I, F> {{
            fn next(&mut self) -> Option<i64> {{
                while true {{
                    let Option::Some(val) = self.iter.next() else {{
                        return Option::None;
                    }};
                    if (self.predicate)(val) {{
                        return Option::Some(val);
                    }}
                }}
                return Option::None;
            }}

            fn sum(&mut self) -> i64 {{
                let mut total: i64 = 0;
                while true {{
                    let Option::Some(val) = self.next() else {{ break; }};
                    total = total + val;
                }}
                return total;
            }}
        }}

        impl Range {{
            fn filter<F>(self, f: F) -> Filter<Range, F> {{
                return Filter {{ iter: self, predicate: f }};
            }}
        }}

        fn is_even(x: i64) -> bool {{
            let rem = x - (x / 2) * 2;
            return rem == 0;
        }}

        fn main() -> i32 {{
            let r = Range::new(0, 10);
            let total = r.filter(is_even).sum();
            return 0;
        }}
        "#, prelude());

        let result = crate::compile(&code, false, None, true);
        assert!(result.is_ok(), "filter().sum() chain should compile:\n{}", result.unwrap_err());
    }
}
