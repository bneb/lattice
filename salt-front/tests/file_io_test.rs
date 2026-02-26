// Unit tests for std.io.file module
// Tests cover: File struct, Ptr returns, mmap codegen

use salt_front::codegen::emit_mlir;
use salt_front::grammar::SaltFile;

// =============================================================================
// File Struct Parsing Tests
// =============================================================================

#[test]
fn test_file_struct_parses() {
    let src = r#"
        package test::file_struct;
        
        pub struct File {
            fd: i32,
            path: u64,
        }
        
        fn is_valid(fd: i32) -> bool {
            return fd >= 0;
        }
        
        fn main() -> i32 {
            let f = File { fd: 3, path: 0 };
            if is_valid(f.fd) {
                return 0;
            }
            return 1;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse File struct");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    assert!(res.is_ok(), "File struct codegen failed: {:?}", res.err());
}

#[test]
fn test_file_open_close_signature() {
    let src = r#"
        package test::file_open;
        
        pub struct File {
            fd: i32,
            path: u64,
        }
        
        extern fn sys_open(path: &u8, flags: i32, mode: i32) -> i32;
        extern fn sys_close(fd: i32) -> i32;
        
        impl File {
            pub fn open(path: &u8, flags: i32) -> File {
                let fd = sys_open(path, flags, 420);
                return File { fd: fd, path: 0 };
            }
            
            pub fn close(&mut self) -> i32 {
                let result = sys_close(self.fd);
                self.fd = -1;
                return result;
            }
        }
        
        fn main() -> i32 {
            return 0;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    assert!(res.is_ok(), "File open/close codegen failed: {:?}", res.err());
}

#[test]
fn test_file_read_write_syscalls() {
    let src = r#"
        package test::file_rw;
        
        pub struct File {
            fd: i32,
        }
        
        extern fn sys_read(fd: i32, buf: &mut u8, count: u64) -> i64;
        extern fn sys_write(fd: i32, buf: &u8, count: u64) -> i64;
        
        impl File {
            pub fn read(&self, buf: &mut u8, count: u64) -> i64 {
                return sys_read(self.fd, buf, count);
            }
            
            pub fn write(&self, buf: &u8, count: u64) -> i64 {
                return sys_write(self.fd, buf, count);
            }
        }
        
        fn main() -> i32 {
            return 0;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    assert!(res.is_ok(), "File read/write codegen failed: {:?}", res.err());
}

// =============================================================================
// Ptr<T> Return Type Tests (Unified Pointer Model)
// =============================================================================

#[test]
fn test_native_ptr_struct() {
    // Tests that Ptr<T> works as a first-class primitive type
    let src = r#"
        package test::native_ptr;
        extern fn malloc(size: u64) -> Ptr<u8>;
        extern fn free(ptr: Ptr<u8>);
        
        fn main() -> i32 {
            let p: Ptr<u8> = malloc(1024);
            // Ptr<T> supports direct null check via is_null() or comparison
            free(p);
            return 0;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse Ptr<u8>");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    assert!(res.is_ok(), "Ptr<u8> codegen failed: {:?}", res.err());
}

#[test]
fn test_mmap_returns_native_ptr() {
    // Tests mmap returning Ptr<u8> - the unified pointer primitive
    let src = r#"
        package test::mmap;
        
        pub struct File {
            fd: i32,
        }
        
        extern fn sys_mmap(addr: u64, len: u64, prot: i32, flags: i32, fd: i32, offset: i64) -> Ptr<u8>;
        
        fn mmap(file_fd: i32, len: u64) -> Ptr<u8> {
            let prot: i32 = 3;
            let flags: i32 = 1;
            return sys_mmap(0, len, prot, flags, file_fd, 0);
        }
        
        fn main() -> i32 {
            let f = File { fd: 5 };
            let p: Ptr<u8> = mmap(f.fd, 4096);
            return 0;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse mmap");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    assert!(res.is_ok(), "mmap returning Ptr<u8> failed: {:?}", res.err());
}

// =============================================================================
// Convenience Function Tests
// =============================================================================

#[test]
fn test_read_all_convenience() {
    let src = r#"
        package test::read_all;
        
        pub struct File {
            fd: i32,
        }
        
        extern fn sys_open(path: &u8, flags: i32, mode: i32) -> i32;
        extern fn sys_close(fd: i32) -> i32;
        extern fn sys_read(fd: i32, buf: &mut u8, count: u64) -> i64;
        
        const O_RDONLY: i32 = 0;
        
        impl File {
            pub fn open(path: &u8, flags: i32) -> File {
                let fd = sys_open(path, flags, 420);
                return File { fd: fd };
            }
            
            pub fn close(&mut self) -> i32 {
                return sys_close(self.fd);
            }
            
            pub fn read(&self, buf: &mut u8, count: u64) -> i64 {
                return sys_read(self.fd, buf, count);
            }
            
            pub fn is_valid(&self) -> bool {
                return self.fd >= 0;
            }
        }
        
        pub fn read_all(path: &u8, buf: &mut u8, max_size: u64) -> i64 {
            let mut f = File::open(path, O_RDONLY);
            if !f.is_valid() {
                return -1;
            }
            let n = f.read(buf, max_size);
            f.close();
            return n;
        }
        
        fn main() -> i32 {
            return 0;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse read_all");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    assert!(res.is_ok(), "read_all convenience function failed: {:?}", res.err());
}

#[test]  
fn test_file_mlir_emits_correct_syscall_types() {
    let src = r#"
        package test::syscall_types;
        extern fn malloc(size: usize) -> !llvm.ptr;
        extern fn free(ptr: Ptr<u8>);
        fn main() -> i32 {
            let buf = malloc(64);
            free(buf);
            return 0;
        }
    "#;
    let res = salt_front::compile(src, false, None, true, false);
    assert!(res.is_ok(), "Syscall types failed: {:?}", res.err());
}
