# Compiler Audit Report: `salt-front/src/codegen/`

This report details three critical security vulnerabilities and soundness holes found during a manual audit of the Salt compiler's code generation and verification passes.

## 1. Unsound Precondition Verification in `@requires` (Logic Error)
**File:** `salt-front/src/codegen/verification/mod.rs`
**Line Numbers:** ~181-190

**Description:**
The `VerificationEngine::verify` function attempts to formally verify `@requires` preconditions at call sites using Z3. However, it contains a fundamental logic error in how it interprets SMT solver results. 
The compiler asserts the substituted requirement (`solver.assert(&z3_req_subst);`) and passes the check if Z3 returns `SAT`:
```rust
match solver.check() {
    crate::z3_shim::SatResult::Sat => {
        // Requirement CAN be satisfied → PASS
        *ctx.elided_checks += 1;
    }
    // ...
```

**Impact:**
`SAT` merely proves that *there exists* at least one possible combination of symbolic inputs that satisfies the requirement; it does not prove that the requirement holds for *all* possible values in the caller's context. Because of this, an attacker can pass entirely unconstrained symbolic variables (which could be out-of-bounds or malicious) to a function. As long as the requirement is not a logical contradiction (e.g., `0 > 1`), Z3 will find a valid assignment, return `SAT`, and the compiler will silently elide the runtime bounds check.

**Remediation:**
To prove a precondition, the verifier must assert the *negation* of the precondition (`solver.assert(&z3_req_subst.not())`) and check for `UNSAT`. `UNSAT` indicates that a violation is mathematically impossible.

---

## 2. Unchecked Pointer Indexing (Bounds Checking Unsoundness)
**Files:** 
- `salt-front/src/codegen/expr/memory.rs` (Lines 289-300 in `emit_index`)
- `salt-front/src/codegen/expr/mod.rs` (Lines 596-610 in `emit_lvalue`)

**Description:**
While the compiler contains a dedicated `ptr_bounds_verifier.rs` module intended to use Z3 for proving the safety of pointer indexing, these verification functions (`verify_ptr_index` and `verify_ptr_offset`) are entirely unreferenced outside of their own unit tests.
In `memory.rs` and `mod.rs`, the code generation for `Ptr<T>` indexing (`ptr[i]`) lowers directly to an unchecked MLIR `getelementptr` instruction:
```rust
let res = format!("%ptr_idx_{}", ctx.next_id());
let elem_mlir = element.to_mlir_storage_type(ctx)?;

// LOWERING: Becomes a direct LLVM GEP + LOAD
ctx.emit_gep(out, &res, &ptr_for_gep, &idx_final, &elem_mlir);
```

**Impact:**
Because raw pointers (`Ptr<T>`) and array references (`&[T; N]`) lack both Z3 bounds check elision and runtime bounds checks, any indexing operation is inherently unsafe. Attackers can trivially achieve arbitrary memory read/write (buffer overflows) by providing out-of-bounds indices, completely bypassing the intended memory safety guarantees of the language.

**Remediation:**
Integrate `ptr_bounds_verifier.rs` into the `Type::Pointer` matching blocks in `emit_index` and `emit_lvalue`. If the Z3 proof returns `Unsafe` or `Unknown`, the compiler must enforce memory safety by emitting a mandatory runtime bounds check.

---

## 3. Inconsistent Generic Unification (Monomorphization Type Confusion)
**Files:** 
- `salt-front/src/codegen/generic_resolver.rs` (Lines 413-421 in `unify_types`)
- `salt-front/src/codegen/expr/mod.rs` (Lines 1177-1183 in `unify_types_recursive`)

**Description:**
During generic type inference, the compiler structurally unifies argument types with function signatures to resolve generic parameters. The unification logic binds a generic placeholder `T` to a concrete type the first time it is encountered:
```rust
(Type::Generic(name), _) => {
    if !map.contains_key(name) {
        map.insert(name.clone(), concrete.clone());
    }
}
```
Crucially, if the same generic parameter appears multiple times in a signature (e.g., `fn swap<T>(a: T, b: T)`), the compiler silently skips subsequent unifications because the key is already in the map. It never validates that subsequent arguments match the initially bound type.

**Impact:**
This enables critical type confusion vulnerabilities during code generation. If a user calls `swap(1, "secret_string_ptr")`, the compiler maps `T` to `Int` based on the first argument and ignores the mismatched second argument. The function is specialized as `swap(Int, Int)`. During MLIR emission, the compiler will forcefully treat the string pointer as a raw integer (or vice versa), breaking type safety and potentially allowing attackers to leak pointers or forge object addresses.

**Remediation:**
The `unify_types` functions must enforce consistency. If `map.contains_key(name)` evaluates to true, the compiler must verify that the previously mapped type is structurally equivalent to the new `concrete` type. If they do not match, it must throw a hard type-mismatch compilation error.