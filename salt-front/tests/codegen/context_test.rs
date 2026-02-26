
#[cfg(test)]
mod tests {
    use salt_front::codegen::context::{StringInterner, CodegenContext};
    use salt_front::grammar::SaltFile;
    use salt_front::types::Type;
    use std::rc::Rc;
    // use std::collections::{BTreeMap, HashMap}; // unused

    #[test]
    fn test_string_interner() {
        let mut interner = StringInterner::new();
        let s1 = interner.intern("hello");
        let s2 = interner.intern("hello");
        let s3 = interner.intern("world");

        assert_eq!(s1, s2);
        assert!(Rc::ptr_eq(&s1, &s2)); // Must be same pointer
        assert_ne!(s1, s3);
        assert_eq!(s1.as_ref(), "hello");
    }

    #[test]
    fn test_physical_index_layout() {
        // Mock a Context to test `get_physical_index`
        // We don't need a real file for this specific method if we populate registry manually?
        // Actually CodegenContext::new requires basic args.
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        // Define a struct: [u8, i32, u8]
        // Alignment: i32 needs 4-byte align.
        // Offset 0: u8 (size 1)
        // Padding: 3 bytes (to align 4)
        // Offset 4: i32 (size 4)
        // Offset 8: u8 (size 1)
        // Total size: 9 -> padded to 12 (align 4)
        
        // Physical Indices:
        // 0: u8
        // 1: pad (hidden? No, physical index includes padding?)
        // Wait, `get_physical_index` calculates index in LLVM struct?
        // LLVM struct usually is packed or explicit padding.
        // Salt generates explicit padding fields.
        // So:
        // Field 0 (u8) -> Index 0
        // Padding [u8; 3] -> Index 1
        // Field 1 (i32) -> Index 2
        // Field 2 (u8) -> Index 3
        // Padding [u8; 3] (end) -> Not a field index, but part of struct size.
        
        let fields = vec![Type::U8, Type::I32, Type::U8];
        
        // Verify layouts of primitives first
        assert_eq!(ctx.size_of(&Type::U8), 1);
        assert_eq!(ctx.align_of(&Type::I32), 4);

        // logical 0 -> physical 0
        assert_eq!(ctx.get_physical_index(&fields, 0), 0);
        // logical 1 -> physical 2 (skip padding at 1)
        assert_eq!(ctx.get_physical_index(&fields, 1), 2);
        // logical 2 -> physical 3
        assert_eq!(ctx.get_physical_index(&fields, 2), 3);
    }
    
    #[test]
    fn test_layout_cache() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        
        let ty = Type::Array(Box::new(Type::I64), 10);
        let (s1, a1) = ctx.get_layout(&ty);
        let (s2, _a2) = ctx.get_layout(&ty);
        
        assert_eq!(s1, 80);
        assert_eq!(a1, 8);
        assert_eq!(s1, s2);
        
        // Ensure cache is populated (internal inspection not easy without RefCell access or mock)
        // But functionally verified by consistent results.
    }

    #[test]
    fn test_scan_local_definitions_imports_coverage() {
        let code = r#"
            use std.io;
            use kernel.core as k;
            fn main() {}
        "#;
        
        let file: SaltFile = syn::parse_str(code).unwrap();
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        
        assert_eq!(ctx.imports.borrow().len(), 0);
        ctx.scan_defs_from_file(&file);
        
        let imports = ctx.imports.borrow();
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].name[0].to_string(), "std");
        assert_eq!(imports[0].name[1].to_string(), "io");
        assert_eq!(imports[1].alias.as_ref().unwrap().to_string(), "k");
    }
}
