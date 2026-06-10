//! TDD Tests for Ticket 3: Kill Switch Removal & @kernel_unsafe
//!
//! The global --no-verify flag is a "kill switch" that disables ALL Z3
//! verification. This ticket extends unsafe block permission to kernel.*
//! packages (in addition to std.*) and renames --no-verify.

mod tests {
    use crate::grammar::SaltFile;
    use crate::codegen::context::CodegenContext;

    fn try_compile(source: &str) -> Result<String, String> {
        let file: SaltFile = syn::parse_str(source)
            .map_err(|e| format!("Parse error: {}", e))?;
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        ctx.drive_codegen()
    }

    // =========================================================================
    // LAYER 1: unsafe blocks in kernel.* packages
    // =========================================================================

    /// RED: Stmt::Unsafe only allows "std" packages. Kernel must also be allowed.
    #[test]
    fn test_kernel_code_allows_unsafe_blocks() {
        let result = try_compile(r#"
            package kernel.core.test_unsafe

            fn test_raw_cast() -> i64 {
                let addr: u64 = 0x1000;
                unsafe {
                    let ptr = addr as &i64;
                    let val = *ptr;
                    return val;
                }
            }
            fn main() -> i32 {
                let _ = test_raw_cast();
                return 0;
            }
        "#);

        assert!(
            result.is_ok(),
            "Kernel packages must be allowed to use unsafe blocks. Error: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_kernel_subpackage_allows_unsafe() {
        let result = try_compile(r#"
            package kernel.mem.vmm

            fn map_page(phys: u64) -> i64 {
                unsafe {
                    let ptr = phys as &i64;
                    return *ptr;
                }
            }
            fn main() -> i32 {
                let _ = map_page(0);
                return 0;
            }
        "#);

        assert!(
            result.is_ok(),
            "Kernel subpackages must be allowed to use unsafe blocks. Error: {:?}",
            result.err()
        );
    }

    // =========================================================================
    // LAYER 2: User code still rejects unsafe
    // =========================================================================

    #[test]
    fn test_user_code_rejects_unsafe() {
        let result = try_compile(r#"
            package myapp.core.logic

            fn bad_cast(addr: u64) -> i64 {
                unsafe {
                    let ptr = addr as &i64;
                    return *ptr;
                }
            }
            fn main() -> i32 {
                let _ = bad_cast(0);
                return 0;
            }
        "#);

        assert!(
            result.is_err(),
            "User code must NOT be allowed to use unsafe blocks"
        );
    }

    // =========================================================================
    // LAYER 3: unsafe scoping reverts
    // =========================================================================

    #[test]
    fn test_unsafe_scoping_reverts_after_block() {
        let result = try_compile(r#"
            package kernel.core.test_scope

            fn test_scoping() -> i32 {
                let addr: u64 = 0x1000;
                unsafe {
                    let ptr = addr as &i64;
                }
                let x: i64 = 42;
                return 0;
            }
            fn main() -> i32 {
                let _ = test_scoping();
                return 0;
            }
        "#);

        assert!(
            result.is_ok(),
            "Unsafe scoping must revert after block. Error: {:?}",
            result.err()
        );
    }

    // =========================================================================
    // LAYER 4: std.* still works (regression)
    // =========================================================================

    #[test]
    fn test_std_code_allows_unsafe_regression() {
        let result = try_compile(r#"
            package std.core.mem

            fn raw_read(addr: u64) -> i64 {
                unsafe {
                    let ptr = addr as &i64;
                    return *ptr;
                }
            }
            fn main() -> i32 {
                let _ = raw_read(0);
                return 0;
            }
        "#);

        assert!(
            result.is_ok(),
            "std.* packages must still be allowed to use unsafe blocks. Error: {:?}",
            result.err()
        );
    }
}
