# TS-03: Software Memory Tagging (SMT) Design

## Objective
Implement dynamic temporal safety as a fallback when static Z3-based verification yields `SatResult::Unknown` or when the user explicitly forces it via `@dynamic_check`. This relies on storing "Epoch IDs" in the unused top 16 bits of a 64-bit pointer and validating them against a runtime shadow store or allocation header on dereference.

## Core Mechanisms

### 1. Pointer Tagging (Tag Assignment)
- **Tag Generation:** When memory is allocated (via `malloc` or arena), an epoch ID (e.g., a 16-bit pseudo-random value or an incrementing generation counter) is generated.
- **Pointer Injection:** The 16-bit epoch ID is bitwise-OR'd into the top 16 bits (bits 48-63) of the returned pointer.
- **Header Storage:** The epoch ID is simultaneously stored in a dedicated shadow map or a hidden allocation header directly preceding the allocated memory.

### 2. Pointer Masking (Dereference)
- Hardware on x86_64 expects canonical addresses. Pointers must be stripped of their top 16 bits before issuing actual memory loads/stores.
- **Operation:** `raw_ptr = tagged_ptr & 0x0000FFFFFFFFFFFF`.
- *Note: On ARM64 with TBI (Top Byte Ignore) or MTE, hardware masking is native. For generic LLVM/MLIR, we must emit masking instructions manually.*

### 3. Dynamic Checking
When evaluating paths that cannot be statically proven safe (`PointerState::Optional` with no Z3 proof):
- Emit MLIR instructions to extract the tag from the pointer: `ptr_tag = tagged_ptr >> 48`.
- Emit MLIR to retrieve the canonical pointer: `raw_ptr = tagged_ptr & 0x0000FFFFFFFFFFFF`.
- Emit MLIR to load the expected tag from the allocation's metadata (e.g., `*(raw_ptr - 2)` assuming a 16-bit tag header).
- Emit MLIR to trap/panic if `ptr_tag != expected_tag`.

### 4. Tag Invalidation (Deallocation)
- When `free` is called, the expected tag in the metadata is set to an invalid state (e.g., `0x0000` or simply incremented). 
- Any subsequent access with the old tagged pointer will naturally fail the dynamic check.

### 5. Compiler Integration (`@dynamic_check`)
- Add parsing support for `@dynamic_check` attribute on functions or blocks.
- When `VerificationEngine::verify_ensures` or `check_read` encounters an `Optional` pointer:
  - If a static proof fails (timeout or unprovable due to loops/aliasing), it typically emits a warning or error.
  - Now, we defer to the **SMT pass**. The compiler inserts the check-and-mask MLIR block instead of failing compilation.

## Implementation Steps
1. **IR Extensions:** Implement `llvm.call @salt_verify_epoch(ptr)` MLIR emission on unprovable pointer dereferences.
2. **Metadata Layout:** **[FINAL PIVOT: Shadow Map]** Both Inline Headers (due to Interior Pointers) and Fat Pointers (due to FFI compatibility — C functions return naked pointers, so we cannot reconstruct a Fat Pointer's tag data) are rejected. We will use a **Global Shadow Map**. `salt_verify_epoch(ptr)` will right-shift the pointer by 3 (8-byte granularity) and load the 1-byte epoch tag from a pre-allocated shadow memory region. This preserves 100% C-ABI compatibility and eliminates bitmasking overhead on valid paths.
3. **AST & Parsing:** Support `@dynamic_check`.
4. **Verification Fallback:** Modify `PointerStateTracker::check_read` to allow `Optional` and `Uninitialized` checks to remain active statically, except `Optional` is allowed if dynamic checking is enabled.
