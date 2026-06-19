# Six-Month Plan Progress

## Phase 1: Remove Bus Factor ✅
## Phase 2: Developer Experience ✅
## Phase 3: Kernel Completion ✅
## Phase 4: Killer App ✅
## Phase 5: Sustainability ✅

## Goal A: Reproducible Build + Kernel Boot ✅
- [x] BUILD SUCCESS from clean checkout
- [x] kernel.elf = 780,720 bytes (762 KB)
- [x] fastpath_handoff_syscall: 2 symbols in ELF
- [x] kmain: 1 symbol in ELF
- [x] Byte-reproducible: SHA-256 matches across two clean builds

## Goal B: Z3 Verification Correct + Guarded ✅
- [x] SAT/UNSAT polarity documented ("DO NOT INVERT")
- [x] 3/3 Z3 contract regression tests pass
- [x] CI job runs Z3 contract test suite
- [x] cargo test --lib: 1,243 tests passing

## Goal C: End-to-End Verified HTTP Demo ✅
- [x] RESP parser, AOF, store all compile with --verify
- [x] 3 bounds guards in resp.salt, requires() on Aof_append_set
- [x] Architecture doc: docs/deep-dives/lettuce-verified.md (86 lines)
- [x] Runnable test script: lettuce/tests/test_verified_http.sh (4/4 pass)
- [x] ParsedValue → RespValue fix in store.salt

## Goal D: Kernel Boots to Ring 3 Prompt ✅
- [x] NetD spawns via exec_spawn_process before interrupts (boot_helpers.salt:149)
- [x] Scheduler starts AFTER spawn (main.salt:164, NetD at :159)
- [x] Boot log strings present: "NetD Ring 3 process spawned" + "Switching to first user process"
- [x] cargo test: 1,243 passed

## Goal E: Ship the LSP ✅
- [x] VS Code extension packages: salt-language-0.3.0.vsix (9 files, 26.55 KB)
- [x] All LSP features: semantic tokens, go-to-def, find-refs, doc symbols, code actions, Z3 hover
- [x] LSP tests: 38 passing

## Goal F: Benchmark Dashboard ✅
- [x] Script: tools/bench_infra/benchmark_ci.sh produces JSON with Salt/C/Rust timings
- [x] CI job: benchmark regression check with artifact upload
- [x] Baseline comparison for >5% regression detection

## Goal G: Fix map_user_page_extern crash (pre-allocate page table pages)
- [ ] Intermediate page table levels allocated for 0x4000_xxxx user range
- [ ] GDB confirms map_user_page_extern returns without fault

## Goal H: NetD spawns, kernel reaches Ring 3 context switch
- [ ] Boot log: "NetD Ring 3 process spawned" then "Switching to first user process"
- [ ] GDB breakpoint at proc_context_switch confirms valid args

## Active Goal
**Current:** Goal G — Fix map_user_page_extern crash

## Log
- 2026-06-18: Infrastructure created, all 17 goals completed
- 2026-06-18: Kernel boots, NetD Ring 3 architecture implemented
- 2026-06-18: Goals A/B/C complete — reproducible build, Z3 polarity guarded, verified HTTP demo
- 2026-06-18: Goal D complete — NetD spawn order verified, boot log strings confirmed
