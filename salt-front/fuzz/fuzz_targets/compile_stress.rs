#![no_main]
use libfuzzer_sys::fuzz_target;
use saltc::fuzz_ast::FuzzSaltFile;
use saltc::codegen::emit_mlir;

fuzz_target!(|fuzz_file: FuzzSaltFile| {
    let salt_file = fuzz_file.to_salt();
    // We bypass the parser and comptime passes for now to stress codegen
    // In a real scenario we might want to run comptime passes too
    let _ = emit_mlir(&salt_file, false);
});
