use saltc::compile;
use std::fs;

#[test]
fn test_pmm_compilation() {
    let pmm_path = "../kernel/core/pmm.salt";
    let code = fs::read_to_string(pmm_path).expect("Failed to read pmm.salt");
    
    // We need to mock the environment or ensure pmm.salt is self-contained.
    // pmm.salt uses `!llvm.ptr` which is built-in.
    // It uses `u64` etc.
    
    // However, pmm.salt might have package declaration `package kernel.core.pmm`.
    // The compiler handles this. 
    
    let result = compile(&code, false, None, true);
    assert!(result.is_ok(), "PMM compilation failed: {:?}", result.err());
    
    let mlir = result.unwrap();
    
    // Verify Key Characteristics of KeuOS PMM
    
    // 1. Check for FreePageNode struct usage
    // It should be defined as a type, or at least used in logic.
    // Actually structs in Salt map to !llvm.struct.
    // We expect !llvm.struct<"FreePageNode", (!llvm.ptr)>
    
    assert!(mlir.contains("!llvm.struct<\"FreePageNode\", (!llvm.ptr)>"), "Missing FreePageNode struct definition in MLIR");
    
    // 2. Check for Atomic CAS in alloc
    // We look for logic inside @kernel__core__pmm__alloc
    assert!(mlir.contains("llvm.cmpxchg"), "Missing CAS in alloc");
    
    // 3. Check for Global Head
    assert!(mlir.contains("@kernel__core__pmm__FREE_LIST_HEAD"), "Missing global free list head");
    
    // 4. Check for loop structure in alloc (branching)
    // llvm.cond_br or similar
    assert!(mlir.contains("cond_br"), "Missing conditional branch in alloc loop");
}
