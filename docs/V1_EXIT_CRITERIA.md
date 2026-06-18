# Salt + KeuOS v1.0.0 — Exit Criteria

**Status:** Proposed
**Target:** Q4 2026

v1.0.0 is the first stable release. After this milestone, the syscall ABI, language syntax, and public APIs are governed by stability guarantees. This document defines "done."

---

## Compiler (salt-front v1.0.0)

### Language Stability
- [ ] All syntax features documented in `docs/SPEC.md` and implemented consistently
- [ ] Generics: 6-phase inference pipeline has no known soundness bugs
- [ ] Pattern matching: match exhaustiveness checking passes on all valid patterns
- [ ] Traits: `@derive` supports all four traits on generic structs
- [ ] No breaking syntax changes without an edition mechanism

### Verification
- [ ] Z3 SAT/UNSAT inversion bug is fixed (Phase 1)
- [ ] `requires` verification: zero false positives on the benchmark suite
- [ ] `ensures` verification: postcondition tests pass for pure functions
- [ ] ArenaVerifier: no known escape-analysis false negatives
- [ ] Z3 timeout threshold documented and tunable

### Code Quality
- [ ] `cargo clippy -- -D warnings` passes with zero warnings
- [ ] `cargo test`: all 1,318+ `#[test]` functions pass
- [ ] `unsafe` blocks: every occurrence has a documented justification
- [ ] Test-to-source ratio ≥ 20% (currently ~15%)

### Performance
- [ ] All 28 benchmarks within 10% of C (`clang -O3`)
- [ ] No benchmark regressions >5% from v0.9.2

---

## Kernel (KeuOS v1.0.0)

### Architecture
- [ ] NetD runs in Ring 3 over zero-trap SPSC rings (not as kernel thread)
- [ ] TCP stack: connect, send, recv, close functional over VirtIO
- [ ] EXT2 filesystem: read support verified against test images

### Syscall ABI (Frozen)
- [ ] All syscall numbers documented and frozen in `docs/abi/KEUOS_ABI_STABLE.md`
- [ ] Syscall struct layouts are backward-compatible
- [ ] `sys_ipc_reg_send` (14) and `sys_ipc_await` (15) implemented (currently ENOSYS)

### SMP
- [ ] 16-core SMP: all cores boot and participate in work-stealing
- [ ] Chase-Lev memory ordering hardened to acquire/release pairs
- [ ] Cross-core TLB shootdowns implemented (Phase 2)

### Security
- [ ] SPSC ring capacity/tail clamping validated by arbiter
- [ ] KASLR implemented
- [ ] SMAP/SMEP enforced
- [ ] Page table leak on process exit fixed
- [ ] Proof hint collision resistance formally analyzed

### Testing
- [ ] All kernel TDD gates pass GREEN in QEMU (KVM and TCG)
- [ ] Preemptive test layers 1+ un-skipped and passing
- [ ] Ring 3 E2E test: SYSCALL gate passes on KVM

---

## Standard Library (v1.0.0)

- [ ] All 70+ modules have doc comments on public functions
- [ ] `docs/stdlib/` API reference covers every public type and function
- [ ] No undocumented `unsafe` in stdlib
- [ ] `HashMap`, `Vec`, `String`, `File`, `TcpStream`: fuzz-tested

---

## Tooling (v1.0.0)

### LSP (v0.4.0+)
- [ ] Semantic tokens, references, document symbols: shipped and tested
- [ ] Z3 counterexample hover: "Z3 says: counterexample x=15" on squiggly lines
- [ ] Code actions: "Add requires clause" and "Wrap in @trusted" shipped

### Package Manager (v0.3.0+)
- [ ] Version resolution (PubGrub) implemented
- [ ] Package registry protocol defined (at minimum: Git-based)
- [ ] `sp build` invokes salt-front directly (no shell script dependency)

### CI
- [ ] macOS build + test in CI
- [ ] Kernel boot smoke test in CI (QEMU)
- [ ] Benchmark regression tracking (flag >5% regressions)
- [ ] Code coverage reporting (codecov or similar)

---

## Documentation

- [ ] "Salt by Example" tutorial: 8 chapters complete
- [ ] Standard library API reference: all modules documented
- [ ] Architecture Decision Records: 15+ ADRs
- [ ] Changelog: v0.9.2 → v1.0.0 migration guide
- [ ] `CONTRIBUTING.md` with working contributor ladder

---

## Non-Goals for v1.0.0

These are explicitly deferred:
- RISC-V port (aarch64 must be bootable first)
- Self-hosting compiler (Salt compiler written in Salt)
- WebGPU support in Basalt
- Dynamic service discovery for daemons
- Userspace device drivers (beyond VirtIO)

---

## Sign-Off

v1.0.0 ships when:
1. All checkboxes above are checked
2. The benchmark suite shows zero regressions from v0.9.2
3. A clean `make setup` → `make test` → `make run-qemu` works on macOS and Linux
4. One production-shaped application (Lettuce or a TLS proxy) runs with Z3-verified safety
5. The syscall ABI freeze has been in effect for ≥2 weeks with no violations
