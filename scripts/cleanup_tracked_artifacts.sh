#!/usr/bin/env bash
# Salt V1 Pre-Launch: Remove tracked build artifacts from git index
# Files stay on disk but are no longer tracked (already in .gitignore)
set -euo pipefail

echo "=== Phase 1: Root-level intermediate build outputs ==="
git rm --cached -f --ignore-unmatch \
  .mlir \
  repro_crash.mlir \
  loop_verify.testmlir \
  struct_verify_backend.testmlir \
  test_alloca.testmlir \
  sieve_stdlib.mlir \
  sieve.mlir sieve.ll sieve.o

echo "=== Phase 2: KeuOS train artifacts ==="
git rm --cached -f --ignore-unmatch \
  keuos_train.mlir keuos_train.ll keuos_train.o \
  keuos_train_opt.mlir keuos_train_scf.mlir \
  keuos_train_unrolled.mlir keuos_train_vec.mlir \
  keuos_train_batch.mlir keuos_train_batch.ll keuos_train_batch.o \
  keuos_train_batch_opt.mlir keuos_train_batch_unrolled.mlir keuos_train_batch_vec.mlir \
  keuos_train_old.mlir keuos_train_old.ll keuos_train_old.o \
  keuos_train_old_opt.mlir keuos_train_old_scf.mlir \
  keuos_train_v4.mlir keuos_train_v4.ll keuos_train_v4.o \
  keuos_train_v4_opt.mlir keuos_train_v4_scf.mlir \
  keuos_train_v4_unrolled.mlir keuos_train_v4_vec.mlir \
  keuos_train_v5.mlir keuos_train_v5.ll keuos_train_v5.o \
  keuos_train_v5_opt.mlir keuos_train_v5_scf.mlir \
  keuos_train_v6.mlir keuos_train_v6.ll keuos_train_v6.o \
  keuos_train_v6_opt.mlir keuos_train_v6_scf.mlir

echo "=== Phase 3: Test intermediate outputs ==="
git rm --cached -f --ignore-unmatch \
  test_addr.mlir test_atomic.mlir test_atomic_align.mlir test_atomic_final.mlir \
  test_layout.mlir test_load.mlir test_load_int.mlir test_output.mlir \
  test_rdtsc.mlir test_rmw_types.mlir test_shim.mlir test_raw_vec_fix.mlir \
  test_arena_alloc.mlir test_arena_alloc.ll test_arena_alloc.o \
  test_arena_alloc_opt.mlir test_arena_alloc_scf.mlir \
  test_arena_poison.mlir test_arena_poison.ll test_arena_poison.o \
  test_arena_poison_opt.mlir test_arena_poison_scf.mlir \
  test_large_alloc.mlir test_large_alloc.ll test_large_alloc.o \
  test_large_alloc_opt.mlir test_large_alloc_scf.mlir \
  test_matmul_ffi.mlir test_matmul_ffi.ll test_matmul_ffi.o \
  test_matmul_ffi_opt.mlir test_matmul_ffi_scf.mlir \
  test_matmul_small.mlir test_matmul_small.ll test_matmul_small.o \
  test_matmul_small_opt.mlir test_matmul_small_scf.mlir \
  test_pointer_add.mlir test_pointer_add.ll test_pointer_add.o \
  test_pointer_add_opt.mlir test_pointer_add_scf.mlir \
  test_vector_ops.mlir test_vector_ops.ll test_vector_ops.o \
  test_vector_ops_opt.mlir test_vector_ops_scf.mlir

echo "=== Phase 4: Compiled binaries ==="
git rm --cached -f --ignore-unmatch \
  deep_recursion edge_cases for_loop_test_exe \
  sieve_c sieve_rs sieve_salt \
  keuos_train keuos_train_c keuos_train_old \
  test_arena_alloc_bin test_arena_poison_bin test_debug

echo "=== Phase 5: Object files ==="
git rm --cached -f --ignore-unmatch \
  common_bridge.o driver_v3.o ml_bridge.o runtime.o

echo "=== Phase 6: Logs & profiling ==="
git rm --cached -f --ignore-unmatch \
  build.log compiler_debug.log compiler_trace.log qemu.log \
  keuos_train_debug.log test_arena_alloc_debug.log \
  default.profraw salt-opt.profdata benchmark_results.json
# verification_run logs
git rm --cached -f --ignore-unmatch \
  verification_run.log verification_run_2.log verification_run_3.log \
  verification_run_4.log verification_run_5.log verification_run_6.log \
  verification_run_7.log verification_run_8.log verification_run_9.log \
  verification_run_10.log verification_run_11.log verification_run_12.log \
  verification_run_13.log verification_run_14.log verification_run_15.log \
  verification_run_16.log verification_run_17.log verification_run_18.log \
  verification_run_19.log verification_run_20.log verification_run_21.log \
  verification_run_22.log verification_run_23.log verification_run_24.log \
  verification_run_final.log

echo "=== Phase 7: Loose scratch files ==="
git rm --cached -f --ignore-unmatch \
  mini_driver.c mini_test.c sieve_check.c test_utils.c \
  test.salt test_ptr.salt test_raw_vec_fix.salt test_tensor_ops.salt \
  ipc_ping_pong.salt keuos_status.h pipeline.sh \
  fail_semantic_2.salt fail_semantic_4.salt fail_semantic_6.salt \
  fail_semantic_8.salt fail_semantic_472.salt fail_semantic_474.salt \
  fail_semantic_476.salt fail_semantic_478.salt fail_semantic_480.salt

echo "=== Phase 8: Directories ==="
git rm --cached -rf --ignore-unmatch \
  coverage_report/ coverage_artifacts/ keuos_rt/ qemu_build/

echo "=== Phase 9: salt-front loose files ==="
git rm --cached -f --ignore-unmatch \
  salt-front/contextual_trie_verification.salt \
  salt-front/nested_hydration.salt \
  salt-front/sieve.salt \
  salt-front/test_io.salt \
  salt-front/test_println.salt \
  salt-front/verification_generics.salt \
  salt-front/src/phase22_full.salt \
  salt-front/debug.log \
  salt-front/sieve_salt_bench.mlir \
  salt-front/test_combinators_opt.mlir \
  salt-front/test_combinators.mlir \
  salt-front/test_simple_opt.mlir \
  salt-front/test_simple.mlir

echo ""
echo "=== Done! ==="
echo "Removed files from git tracking. They still exist on disk."
echo "Run 'git status' to review, then 'git commit -m \"chore: remove tracked build artifacts\"'"
