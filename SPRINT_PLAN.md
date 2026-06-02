# KeuOS & Salt - Sprint Plan

This sprint plan is derived from the comprehensive codebase audit and prioritizes stabilization, safety, and core features.

## Sprint 1: Compiler & Stdlib Stabilization
**Goal:** Eradicate internal compiler errors (ICEs), implement missing core memory primitives, and add FFI bindings.

- [ ] **SALT-01**: Implement `SovereignArena` and `DmaArena` in `salt/std/mem/` to fulfill standard library requirements.
- [ ] **SALT-02**: Add `#[export]` ABI bindings to the Salt Compiler to allow WASM to preserve entry points and remove `basalt` DCE hacks.
- [ ] **SALT-03**: Complete RAII drop emission for `Owned` types in `salt-front/src/codegen/stmt.rs` to fix memory leaks.
- [ ] **SALT-04**: Replace `.unwrap()` usage in `salt-front/src/codegen/expr/` and `hir/typeck.rs` with graceful `Result<T, Diagnostic>` reporting.
- [ ] **SALT-05**: Convert all deprecated `import` keywords to `use` within `salt/std/`.

## Sprint 2: Safety & Runtime Overhaul
**Goal:** Eliminate unbounded pointer arithmetic and fix executor deadlocks.

- [x] **KEU-01**: Refactor `lettuce/src/server.salt` to replace manual `memcpy` sliding windows with bounds-checked slices (Completed via `std.collections.string_map` and RESP refactor).
- [ ] **KEU-02**: Strip unbounded array loops and struct offsets from `basalt/src/main.salt`.
- [ ] **KEU-03**: Fix the Chase-Lev deque race condition in `salt/std/async/executor.salt` (prevent non-atomic remote pushes to `bottom`).
- [ ] **KEU-04**: Inject `intrinsics::m4_sev()` wake sequence across the asynchronous executor state transitions to resolve `WFE` deadlocks.
- [ ] **KEU-05**: Implement strict `requires` / `ensures` bounds-checking contracts on `salt/std/net/buffer.salt` methods to unblock Z3 verification.
- [ ] **KEU-06**: Add missing packet length checks (`BUF_SIZE - VIRTIO_NET_HDR_SIZE`) to the VirtIO Network driver TX pathway.

## Sprint 3: Testing & Resilience
**Goal:** Modernize ecosystem testing, remove shell script orchestrators, and cover edge cases.

- [ ] **ECO-01**: Deprecate `sleep 1` shell scripting in `tools/cloud/bench_run.sh` in favor of deterministic IPC/process synchronization.
- [ ] **ECO-02**: Implement a chaos testing suite for `lettuce` specifically targeting socket exhaustion and connection resets.
- [ ] **ECO-03**: Add explicit unit tests for pipeline fallback flushes in `lettuce`.
- [ ] **ECO-04**: Implement quantization regression testing and tokenizer fuzzing for `basalt`.
- [ ] **ECO-05**: Port the remaining yield-cost analysis AST walker into the compiler (`codegen/passes/yield_injection.rs`).
