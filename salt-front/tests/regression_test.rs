// ============================================================================
// Regression Test Suite
// Entry point for codegen regression tests guarding against fixed bugs
//
// This file includes tests for:
// - Enum type resolution with package prefix stripping
// - String literal escaping and deduplication
// - Aggregate equality (enum comparison) with proper name resolution
// - Generic receiver specialization (Sieve benchmark linker fix - Jan 2026)
// - Inception Guard (Verified Metal Single Indirection Property - Jan 2026)
// - MLIR Header Finalization (Verified Metal Fixed-Point Buffering - Jan 2026)
// ============================================================================

// Include codegen subdirectory test modules as submodules
#[path = "codegen/enum_type_resolution_test.rs"]
mod enum_type_resolution_test;

#[path = "codegen/string_literal_test.rs"]
mod string_literal_test;

#[path = "codegen/aggregate_eq_test.rs"]
mod aggregate_eq_test;

#[path = "codegen/generic_receiver_specialization_test.rs"]
mod generic_receiver_specialization_test;

#[path = "codegen/inception_guard_test.rs"]
mod inception_guard_test;

#[path = "codegen/mlir_finalization_test.rs"]
mod mlir_finalization_test;

#[path = "codegen/identity_routing_test.rs"]
mod identity_routing_test;

#[path = "codegen/hex_prefix_test.rs"]
mod hex_prefix_test;

#[path = "codegen/keuos_authority_test.rs"]
mod keuos_authority_test;

#[path = "codegen/hashmap_eq_dispatch_test.rs"]
mod hashmap_eq_dispatch_test;

#[path = "codegen/hashmap_codegen_test.rs"]
mod hashmap_codegen_test;

#[path = "codegen/ptr_struct_field_test.rs"]
mod ptr_struct_field_test;

#[path = "codegen/result_monomorphization_test.rs"]
mod result_monomorphization_test;

#[path = "codegen/struct_generic_inference_test.rs"]
mod struct_generic_inference_test;

#[path = "codegen/mut_param_loop_test.rs"]
mod mut_param_loop_test;
