// ============================================================================
// Hex Prefix Expansion Tests
// Tests for hex"..." → Vec::<u8>::from_array([...]) transformation
// ============================================================================

#[cfg(test)]
mod tests {
    use salt_front::codegen::context::CodegenContext;
    use salt_front::grammar::SaltFile;

    macro_rules! with_ctx {
        ($name:ident, $block:block) => {
            let file: SaltFile = syn::parse_str("fn main() {}").expect("valid salt file");
            let z3_cfg = z3::Config::new();
            let z3_ctx = z3::Context::new(&z3_cfg);
            #[allow(unused_mut)]
            let mut $name = CodegenContext::new(&file, false, None, &z3_ctx);
            $block
        };
    }

    #[test]
    fn test_hex_expand_basic() {
        with_ctx!(ctx, {
            // Basic hex conversion: DEADBEEF
            let code = ctx.native_hex_expand("DEADBEEF");
            assert!(code.contains("Vec::<u8>::from_array"), "Should generate Vec constructor, got: {}", code);
            assert!(code.contains("0xDE"), "Should have 0xDE byte");
            assert!(code.contains("0xAD"), "Should have 0xAD byte");
            assert!(code.contains("0xBE"), "Should have 0xBE byte");
            assert!(code.contains("0xEF"), "Should have 0xEF byte");
        });
    }

    #[test]
    fn test_hex_expand_ascii() {
        with_ctx!(ctx, {
            // ASCII "ABC" = 0x41, 0x42, 0x43
            let code = ctx.native_hex_expand("414243");
            assert!(code.contains("0x41"), "Should have 0x41 ('A')");
            assert!(code.contains("0x42"), "Should have 0x42 ('B')");
            assert!(code.contains("0x43"), "Should have 0x43 ('C')");
        });
    }

    #[test]
    fn test_hex_expand_with_spaces() {
        with_ctx!(ctx, {
            // Whitespace separators are allowed
            let code = ctx.native_hex_expand("DE AD BE EF");
            assert!(code.contains("0xDE"), "Should handle spaces, got: {}", code);
            assert!(code.contains("0xAD"), "Should handle spaces");
            assert!(code.contains("from_array"), "Should be Vec constructor");
        });
    }

    #[test]
    fn test_hex_expand_lowercase() {
        with_ctx!(ctx, {
            // Lowercase should work and normalize to uppercase
            let code = ctx.native_hex_expand("deadbeef");
            assert!(code.contains("0xDE"), "Should handle lowercase and uppercase output, got: {}", code);
        });
    }

    #[test]
    fn test_hex_expand_empty() {
        with_ctx!(ctx, {
            // Empty hex string
            let code = ctx.native_hex_expand("");
            assert!(code.contains("Vec::<u8>::new()"), "Empty hex should create empty Vec, got: {}", code);
        });
    }
    
    #[test]
    fn test_hex_expand_odd_length_error() {
        with_ctx!(ctx, {
            // Odd length hex should return empty Vec (error case)
            let code = ctx.native_hex_expand("ABC"); // 3 chars = odd
            assert!(code.contains("new()"), "Odd length should return empty Vec, got: {}", code);
        });
    }
}
