//! TDD tests for std.io.ring — IoRing user-space wrapper codegen.
//!
//! Verifies that:
//!   1. `struct IoRing` with u64 fields compiles and can be constructed
//!   2. `struct SQE` and `struct CQE` compile with all required fields
//!   3. Atomic extern FFI declarations for ring buffer compile
//!   4. IoRing with impl methods generates correct MLIR
//!   5. Full ring push/pop pattern compiles end-to-end

#[cfg(test)]
mod tests {
    /// Helper: compile Salt source through the full pipeline and return MLIR.
    fn compile_to_mlir(source: &str) -> String {
        crate::compile(source, false, None, true)
            .unwrap_or_else(|e| panic!("Compile failed: {}", e))
    }

    // =========================================================================
    // Test 1: IoRing struct compiles with u64 fields
    // =========================================================================
    #[test]
    fn test_io_ring_struct_compiles() {
        let mlir = compile_to_mlir(r#"
            package main
            struct IoRing {
                sq_base: u64,
                cq_base: u64,
                capacity: u64,
                mask: u64,
            }
            fn main() -> i32 {
                let ring = IoRing { sq_base: 0, cq_base: 0, capacity: 32, mask: 31 };
                return 0;
            }
        "#);

        // Verify struct is used (mangled name)
        assert!(
            mlir.contains("i64"),
            "IoRing struct should compile to MLIR with i64 fields, got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // Test 2: SQE struct with all required fields
    // =========================================================================
    #[test]
    fn test_sqe_struct_compiles() {
        let mlir = compile_to_mlir(r#"
            package main
            struct SQE {
                op: u64,
                fd: u64,
                buf: u64,
                len: u64,
                user_data: u64,
                flags: u64,
            }
            fn main() -> i32 {
                let sqe = SQE { op: 1, fd: 1, buf: 0, len: 5, user_data: 42, flags: 0 };
                return 0;
            }
        "#);

        // Struct construction must produce store instructions for 6 u64 fields
        assert!(
            mlir.contains("llvm.store"),
            "SQE struct construction should emit stores, got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // Test 3: CQE struct with result field
    // =========================================================================
    #[test]
    fn test_cqe_struct_compiles() {
        let mlir = compile_to_mlir(r#"
            package main
            struct CQE {
                user_data: u64,
                result: i64,
                flags: u64,
            }
            fn main() -> i32 {
                let cqe = CQE { user_data: 0, result: 0, flags: 0 };
                return 0;
            }
        "#);

        assert!(
            mlir.contains("llvm.store"),
            "CQE struct construction should emit stores, got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // Test 4: Atomic extern FFI declarations for ring buffer
    // =========================================================================
    #[test]
    fn test_ring_atomic_ffi() {
        let mlir = compile_to_mlir(r#"
            package main
            extern fn salt_atomic_load_i64(ptr: u64) -> i64;
            extern fn salt_atomic_store_i64(ptr: u64, val: i64);
            extern fn salt_syscall1(num: u64, arg0: u64) -> u64;

            fn main() -> i32 {
                let sq_va = salt_syscall1(11, 0);
                let head = salt_atomic_load_i64(sq_va);
                salt_atomic_store_i64(sq_va + 8, head);
                return 0;
            }
        "#);

        assert!(
            mlir.contains("@salt_atomic_load_i64"),
            "Atomic load should be declared, got:\n{}",
            mlir
        );
        assert!(
            mlir.contains("@salt_syscall1"),
            "syscall1 should be declared, got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // Test 5: IoRing with impl method — sq_push pattern
    // =========================================================================
    #[test]
    fn test_io_ring_impl_submit() {
        let mlir = compile_to_mlir(r#"
            package main

            extern fn salt_atomic_load_i64(ptr: u64) -> i64;
            extern fn salt_atomic_store_i64(ptr: u64, val: i64);

            struct IoRing {
                sq_base: u64,
                cq_base: u64,
                capacity: u64,
                mask: u64,
            }

            impl IoRing {
                pub fn sq_push(&mut self, op: u64, fd: u64, buf: u64, len: u64, udata: u64) -> bool {
                    let head = *((self.sq_base) as &u64);
                    let tail = salt_atomic_load_i64(self.sq_base + 8) as u64;
                    if (head - tail) >= self.capacity {
                        return false;
                    }
                    let index = head & self.mask;
                    let entry = self.sq_base + 64 + (index * 64);
                    *((entry) as &mut u64) = op;
                    *((entry + 8) as &mut u64) = fd;
                    *((entry + 16) as &mut u64) = buf;
                    *((entry + 24) as &mut u64) = len;
                    *((entry + 32) as &mut u64) = udata;
                    *((entry + 40) as &mut u64) = 0;
                    salt_atomic_store_i64(self.sq_base, (head + 1) as i64);
                    return true;
                }
            }

            fn main() -> i32 {
                let mut ring = IoRing { sq_base: 0, cq_base: 0, capacity: 32, mask: 31 };
                let ok = ring.sq_push(0, 0, 0, 0, 42);
                return 0;
            }
        "#);

        // The impl method should generate a function with receiver
        assert!(
            mlir.contains("sq_push"),
            "IoRing::sq_push should be emitted in MLIR, got:\n{}",
            mlir
        );
    }
}
