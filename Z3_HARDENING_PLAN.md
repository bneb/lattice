# Z3 Hardening Plan — earning the "verified/proven" claims

Measured with `saltc v1.2.0` (Z3 on). "Deferred to runtime" = the compiler
could not prove the check statically, so it emits a **runtime** check and
exits 0. A headline that says *proven/verified* implies **static** proof, so a
deferred check does not back that claim. The strong claim is earned only when
the claimed surface has **0 deferred** checks and is verified on the compiled
path (not `--lib`, which skips contract proving) under CI.

## Empirical finding — the pipeline DOES support these verifications

The Z3 pipeline handles exactly these verification types. salt's own contract
suite proves **39/39** of them statically: while-loop array bounds via
`invariant`, call-site `requires`/`forall` preconditions, sort invariants,
postconditions, the matmul operator. The capability is real and CI-enforced.

Two corrections after measuring:

1. **Not a timeout.** The 100ms Z3 watchdog (`verification/mod.rs:273,566`,
   `context/raii.rs:114`) is *not* why basalt/facet defer — rebuilding saltc
   with a 5000ms budget gave **identical** numbers (basalt 10/24, facet 0/1).
   These deferrals are structural, not slow.
2. **`--lib` mis-measures.** A `requires` is discharged *at call sites*; in
   library mode there are none, so preconditions defer by construction. With
   real callers (`make test`) basalt proves 10/24, not 0.

Earning the strong claims is **code-level work** (types + invariants + caller
guarantees), not a compiler timeout knob.


## Current enforcement reality

| Repo | Headline Z3 claim | What CI now enforces | Static-proof status |
|------|-------------------|----------------------|---------------------|
| salt | "Z3-powered compile-time verification" | `cargo test` + 39/39 z3-contract tests | **Enforced** |
| keuos | "Z3-verified safety invariants" | `sp check` → all contracts verified | **Enforced (static)** |
| lettuce | "Z3-proven bounds on every buffer access" | `make test` verifies resp/store/aof contracts | **Mostly enforced** — audit "every access" |
| basalt | "Z3-verified compute kernels" | `make test`: 10/24 proven, 14 deferred | **Partial** (rest runtime-checked) |
| facet | "Z3-verified bounds on every pixel write" | `set_pixel` precondition deferred (0/1 proven) | **Runtime-checked, not static** |

## Cross-cutting changes (compiler + CI) — do these first

1. **Add a `--deny-deferred` (a.k.a. `--require-all-proven`) flag to saltc.**
   Turns any "N deferred to runtime" into a hard error. Without it, CI cannot
   distinguish *statically proven* from *runtime-deferred* — both exit 0 today.
   Small/moderate compiler change; it is the keystone that makes every strong
   claim mechanically checkable.
2. **Verify on the real compiled surface, not `--lib`.** `--lib` skips contract
   proving. CI for a "verified" claim must compile the actual entry (or the
   contract-bearing modules as entries) with Z3 on.
3. **Use the release compiler in CI.** Release saltc refuses
   `--danger-no-verify` (`E007`), so verification cannot be silently disabled.
   basalt/lettuce CI currently build *debug* saltc, where the flag works.
4. **Interim gate:** until (1) lands, assert on the `Z3: X/Y ... N deferred`
   summary line in CI (`N` must be 0) for the claimed surface.

## Per-claim work

### facet — "Z3-verified bounds on every pixel write"
- `set_pixel` (raster.salt:224) carries `requires(0<=x<width && 0<=y<height)`,
  proven at the definition but **deferred at call sites** (fill / scanline).
- Add loop invariants in `fill_with_rule` / scanline fill tying loop indices to
  canvas bounds (`invariant 0 <= y && y < canvas.height`; prove x-span ⊆
  `[0,width)`), or funnel every write through a helper that carries the proven
  invariant.
- Then verify the compositor entry (not `--lib`) with `--deny-deferred`.

### basalt — "Z3-verified compute kernels"
- `rmsnorm` / `softmax` / `mat_mul` index raw `Ptr<f32>` inside `unsafe {}`
  blocks, which emit **no bounds checks at all** — the only contract is
  `requires size>0` at entry. So "Z3-verified compute kernels" today means
  *entry preconditions*, not memory-safe loops. To verify the loop accesses,
  give the kernels **length-carrying types** (sized slices `&[f32]` with a
  provable `.length()`) instead of raw pointers, then add loop invariants so
  `x[i]`/`out[i]` bounds are provable.
- The 14 deferrals under `make test` are the `requires size>0` / `m,n,d>0`
  preconditions where the driver (`main.salt`) can't guarantee the value, plus
  float postconditions (`expf`/`sqrtf`) that Z3 returns Unknown on. Add
  validate-and-early-return at the driver boundary (`if cfg.vocab_size <= 0 {
  return err }`) so the precondition is established — this also fixes the
  `main.salt:74 could not prove (> vocab 0)` build failure that forces
  `--danger-no-verify`. Scope numeric-float claims to bounds/memory-safety.

### lettuce — "Z3-proven bounds on every buffer access"
- Contracts on `resp`/`store`/`aof` **do** verify and are now in CI via
  `make test`. To back "every buffer access": audit RESParser/store for any
  deferred bound, compile those modules as verified entries (not `--lib`) with
  `--deny-deferred`.
- Fix the misleading `e2e_test_coverage` check in `tests/test_verified_http.sh`
  (reports "0/5 operations covered" yet PASS) — make it assert real coverage or
  remove it.

### keuos — "Z3-verified safety invariants" (already enforced)
- `sp check` verifies all contracts. Strengthen by expanding `requires` on
  unsafe memory ops and adding `--deny-deferred`. Kernel boot/QEMU tests remain
  out of ubuntu CI (need LLVM21 + QEMU) — keep that caveat explicit.

### salt — the verifier itself (already enforced)
- 39/39 contract tests pass. Raise salt-front line coverage from **63.6%**
  toward the >95% goal; 0%-covered non-test files: `grammar/expr_utils.rs`,
  `interpreter_helpers.rs`, `codegen/types/zero_attr.rs`.

## Prototype findings (facet) — the idiomatic refactor is blocked by the verifier, not the app code

I tried to take facet's `set_pixel` bound from deferred → proven by moving
`Canvas.pixels` to the idiomatic length-carrying `Slice<u8>` (whose `at`/`set`
carry `requires index < self.len`). Minimal probes with `saltc v1.2.0` hit two
verifier walls — one confirmed fundamental:

1. **`.len()` is opaque through construction (confirmed, blocking).**
   `Slice::new(p, 100).len() == 100` **cannot be proven** —
   `VERIFICATION ERROR: could not prove '(= (method_len buf) 100)'`. The verifier
   models a slice's length as an uninterpreted function and does not connect it
   to the constructor's field. This breaks *every* `requires ... < buf.len()`
   check at a call site — i.e. the entire premise of `Slice.at/set` "verified
   access." This is a salt-front verifier gap, not a facet problem.
2. **Flat 2D→1D addressing is nonlinear.** `y*stride + x*4 < stride*height`
   defers (Z3 Unknown) even with every relationship supplied and a 5s budget —
   products of variables. Random-access `set_pixel(x,y)` is inherently
   nonlinear; only sequential/cursor writes stay linear.

**Conclusion:** an idiomatic rewrite alone does **not** earn facet's claim today.
It requires compiler/verifier work first, in this order:
- **Keystone: teach the verifier to propagate `Slice` length through
  construction** (model `Slice::new`'s `len` into `.len()`/`self.len`). Without
  this, no slice-based bound is dischargeable across a call boundary, and the
  whole `Slice` contract design is inert at call sites.
- **Sequential writes** (`clear`, scanline fill) can then be proven with a
  **linear flat cursor** + loop invariant (`off < len`, `off += 4`) — no
  nonlinearity. This is the real, provable win.
- **Random-access `set_pixel(x,y)`** needs nonlinear proof hints/lemmas or stays
  runtime-checked. **mmap'd model weights** (basalt) stay validated-then-trusted.

Net: the honest claim wording is *"Z3-verified bounds on sequential/Salt-managed
buffer writes; random-access and external buffers runtime-checked/validated."*
Nothing was committed to facet — the refactor doesn't achieve static proof until
the verifier's slice-length modeling lands.

## Sequencing
- **Days:** driver input-guards (basalt), loop invariants (facet), e2e assert
  fix (lettuce).
- **Keystone:** `--deny-deferred` compiler flag — unlocks real CI enforcement.
- **Hardest:** nonlinear index proofs (basalt matmul) — proof hints or affine
  restructuring.

---

## Implementation Record — 2026-07-12: Loop Call Precondition Proving

### Changes Made (salt-front/)

Three interlocking fixes enable the verifier to statically discharge callee
`requires` preconditions inside while loops:

#### A. Loop assumption channel (`loop_assumptions` in EmissionState)

**Files:** `codegen/phases/emission.rs`, `codegen/stmt/while_stmt.rs`,
`codegen/verification/mod.rs`

Before this change, `VerificationEngine::verify()` created a fresh Z3 solver
and only loaded `caller_preconditions` + `path_conditions`. The while-loop's
invariant and guard, which were asserted into `ctx.z3_solver` in
`setup_while_loop_inductive_step`, were invisible to the fresh solver.

Added `loop_assumptions: Vec<syn::Expr>` to `EmissionState`. In
`emit_while_stmt`, push the invariant exprs + loop guard before emitting the
body, pop after. In `verify()`, assert `loop_assumptions` into the fresh
solver alongside `path_conditions`.

Soundness: invariants are already proven at loop entry by
`prove_while_loop_base_case`; the guard holds in the body by construction.
Asserting them is sound.

#### B. Method call precondition verification

**File:** `codegen/expr/method_resolution.rs`

Method calls (`buf.set(off, v)`) were emitted via `emit_resolved_method_call`
directly as `func.call @mangled` WITHOUT checking preconditions. Regular
function calls went through `emit_function_args` → `verify()`, but method
calls bypassed this entirely. All `Slice.set`/`Slice.at` requires were
silently skipped.

Added `VerificationEngine::verify()` call in `emit_resolved_method_call`
before emitting the `func.call`, passing the receiver + args as `arg_exprs`
and the function signature params.

#### C. `.len()` method ↔ field unification

**File:** `codegen/expr/memory.rs`

`buf.len()` (method call) lowered to `method_len(buf_z3)` while `self.len`
(field access) lowered to `field_len(self_z3)`. These are different
uninterpreted functions in Z3, so a loop guard `off < buf.len()` could never
discharge a callee's `index < self.len`.

Changed zero-argument method calls in `translate_to_z3` and
`translate_bool_to_z3` to use `field_` prefix instead of `method_`, so both
paths produce the same Z3 function symbol.

#### D. Slice construction length tracking

**Files:** `codegen/phases/emission.rs`, `codegen/stmt/mod.rs`,
`codegen/verification/mod.rs`, `codegen/verification/fold_constants.rs`

`Slice::new(ptr, 100).len() == 100` could not be proven because the
constructor's length argument was not propagated to Z3's `field_len`.
Added `known_slice_lengths: HashMap<String, i64>` to `EmissionState`,
populated at `let`-binding time when the init is a `Slice::new` call or
`Slice { data, len }` struct literal with a literal integer length. Extended
`verify()`'s known_lengths builder to check `known_slice_lengths`. Added
`resolve_fields` to the constant folder so `self.len` (field access, not
method call) in requires clauses also resolves to the known length.

#### E. Refactoring

**File:** `codegen/stmt/while_stmt.rs` — minor edit (loop assumption push/pop
added inline). `has_monotonic_increment` nesting reduced. Function is still
over 32 lines but only marginally — deferred to a dedicated refactor session.

### Test Results

- `cargo test --release`: all pass (0 failures)
- `cargo clippy -- -D warnings`: clean
- `tests/z3_contracts/run_tests.sh`: 43/43 pass (39 original + 4 new RED probes)
- 4 new probes added to CI:
  - `test_slice_cursor_proved`: sequential slice writes inside while loop
  - `test_loop_call_precond_proved`: call precondition proved via invariant+guard
  - `test_slice_len_construction_proved`: Slice::new length propagates to .len()
  - `test_slice_cursor_rejected`: OOB slice access correctly rejected

### What's Proven vs Deferred

The `set()` preconditions inside while loops are now statically proven (2/2
for the slice cursor test). Remaining "deferred" checks are from separate
verification subsystems (pointer bounds on unsafe bodies, postcondition
checks) and are unchanged by this work.

### Remaining: Sub-problem C — symbolic construction length

Only concrete literal lengths (e.g., `Slice::new(p, 100)`) are tracked.
Symbolic lengths (e.g., `stride * height` in facet) would require
expression-level tracking (`HashMap<String, syn::Expr>`) instead of `i64`.
However, the sequential-cursor pattern works with symbolic lengths too:
`while off + 4 <= pixels.len()` directly uses the uninterpreted
`field_len(pixels)` in the guard, and Z3's linear arithmetic proves
`off + 3 < field_len` from `off + 4 <= field_len`. No concrete length
needed for linear cursor bounds.

### Facet application — DONE

**File:** `facet/raster/raster.salt`, `facet/README.md`

`clear()` rewritten from nested 2D loop to linear flat cursor with
`Slice<u8>` wrapper + while-loop invariant. Z3 statically proves all
8 per-byte `set()` bounds (8/11 checks proven, 72%, up from 0/1, 0%).

Required turbofish `Slice::<u8>::new(...)` to resolve generic inference
when `std.core.slice` is imported alongside the raster module's existing
types. `set_pixel(x,y)` and `blend_pixel` stay on raw `Ptr<u8>` writes
for performance; their 2D→1D nonlinear bounds remain runtime-checked.

README updated to accurately distinguish statically-proven sequential
writes from runtime-checked random-access writes.
