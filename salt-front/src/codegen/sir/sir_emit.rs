// =============================================================================
// SIR Emitter — Lowering Salt AST to SIR
// =============================================================================
//
// Translates Salt AST nodes into SIR instructions. This is a "shadow" emitter
// that runs alongside the existing MLIR codegen, producing a parallel SIR
// representation that can be serialized and later translated back to MLIR
// by the sir-to-llvm binary.
//
// The emitter handles:
//   - Function declarations (params, return type, contracts)
//   - Control flow (if/else, while loops with invariants)
//   - Expressions (binary ops, calls, comparisons)
//   - Atomic operations (CAS, load, store)
// =============================================================================

use super::types::*;

/// Emit a SIR function from its name, params, return type, and body blocks.
pub fn emit_sir_function(
    name: &str,
    params: Vec<SirParam>,
    return_type: SirType,
    contracts: Vec<SirContract>,
    body: Vec<SirBlock>,
    is_pub: bool,
    attributes: Vec<String>,
) -> SirFunction {
    SirFunction {
        name: name.to_string(),
        params,
        return_type,
        contracts,
        body,
        is_pub,
        attributes,
        location: None,
    }
}

/// Emit a SIR while loop instruction.
pub fn emit_sir_while(
    condition_reg: &str,
    invariant: Option<&str>,
    body_blocks: Vec<SirBlock>,
) -> SirInstruction {
    SirInstruction::While {
        condition_reg: condition_reg.to_string(),
        verified_invariant: invariant.map(|s| s.to_string()),
        body: body_blocks,
    }
}

/// Emit a SIR if/else instruction.
pub fn emit_sir_if(
    condition_reg: &str,
    then_blocks: Vec<SirBlock>,
    else_blocks: Vec<SirBlock>,
) -> SirInstruction {
    SirInstruction::If {
        condition_reg: condition_reg.to_string(),
        then_blocks,
        else_blocks,
    }
}

/// Emit a SIR function call.
pub fn emit_sir_call(
    target: Option<&str>,
    callee: &str,
    args: Vec<SirValue>,
) -> SirInstruction {
    SirInstruction::Call {
        target: target.map(|s| s.to_string()),
        callee: callee.to_string(),
        args,
    }
}

/// Emit a SIR return.
pub fn emit_sir_return(value: Option<SirValue>) -> SirInstruction {
    SirInstruction::Return { value }
}

/// Emit a SIR binary operation.
pub fn emit_sir_binop(
    target: &str,
    op: &str,
    lhs: SirValue,
    rhs: SirValue,
) -> SirInstruction {
    SirInstruction::BinaryOp {
        target: target.to_string(),
        op: op.to_string(),
        lhs,
        rhs,
    }
}

/// Emit a SIR assignment.
pub fn emit_sir_assign(target: &str, value: SirValue, ty: SirType) -> SirInstruction {
    SirInstruction::Assign {
        target: target.to_string(),
        value,
        ty,
    }
}

/// Emit a SIR comparison.
pub fn emit_sir_compare(
    target: &str,
    op: &str,
    lhs: SirValue,
    rhs: SirValue,
) -> SirInstruction {
    SirInstruction::Compare {
        target: target.to_string(),
        op: op.to_string(),
        lhs,
        rhs,
    }
}

/// Emit a SIR CAS instruction.
pub fn emit_sir_atomic_cas(
    target: &str,
    addr: SirValue,
    expected: SirValue,
    desired: SirValue,
    ordering: &str,
) -> SirInstruction {
    SirInstruction::AtomicCas {
        target: target.to_string(),
        addr,
        expected,
        desired,
        ordering: ordering.to_string(),
    }
}

/// Construct a SIR module from a collection of functions and structs.
pub fn build_sir_module(
    name: &str,
    structs: Vec<SirStruct>,
    functions: Vec<SirFunction>,
) -> SirModule {
    SirModule {
        name: name.to_string(),
        version: SIR_VERSION,
        structs,
        functions,
    }
}

// =============================================================================
// AST Extraction — Walk Salt AST and produce SIR
// =============================================================================

use crate::grammar::{SaltFile, Item, SaltFn, StructDef, SynType};

/// Convert a Salt SynType to a SIR SirType.
fn syntype_to_sirtype(ty: &SynType) -> SirType {
    match ty {
        SynType::Path(path) => {
            let name = path.to_string();
            match name.as_str() {
                "i32" => SirType::I32,
                "i64" => SirType::I64,
                "u32" => SirType::U32,
                "u64" => SirType::U64,
                "f64" => SirType::F64,
                "bool" => SirType::Bool,
                _ => SirType::Struct(name),
            }
        }
        SynType::Pointer(inner) => SirType::Ptr(Box::new(syntype_to_sirtype(inner))),
        SynType::Reference(inner, _) => SirType::Ptr(Box::new(syntype_to_sirtype(inner))),
        SynType::Array(inner, _len_expr) => {
            // We can't easily evaluate the length expression at SIR time,
            // so we use 0 as a sentinel for "expression-length array".
            SirType::Array(Box::new(syntype_to_sirtype(inner)), 0)
        }
        _ => SirType::Void,
    }
}

/// Convert a syn::Expr to a string representation for contract clauses.
fn expr_to_string(expr: &syn::Expr) -> String {
    quote::quote!(#expr).to_string()
}

/// Extract a location from a syn::Ident's span.
fn ident_to_location(ident: &syn::Ident) -> Option<SirLocation> {
    let span = ident.span();
    let start = span.start();
    let end = span.end();
    // proc-macro2 with span-locations returns line=0,col=0 when locations
    // are not available (e.g., in some test contexts). Only emit a location
    // if we have real coordinates.
    if start.line == 0 && start.column == 0 {
        return None;
    }
    Some(SirLocation {
        line: start.line,
        column: start.column,
        end_line: end.line,
        end_column: end.column,
    })
}

/// Extract a SIR struct from a Salt StructDef.
fn extract_sir_struct(s: &StructDef) -> SirStruct {
    let fields = s.fields.iter().map(|f| {
        SirParam {
            name: f.name.to_string(),
            ty: syntype_to_sirtype(&f.ty),
        }
    }).collect();

    let attributes = s.attributes.iter().map(|a| a.name.to_string()).collect();
    let location = ident_to_location(&s.name);

    SirStruct {
        name: s.name.to_string(),
        fields,
        attributes,
        location,
    }
}

/// Extract a SIR function from a Salt SaltFn.
fn extract_sir_function(f: &SaltFn) -> SirFunction {
    let params = f.args.iter().filter_map(|arg| {
        arg.ty.as_ref().map(|ty| SirParam {
            name: arg.name.to_string(),
            ty: syntype_to_sirtype(ty),
        })
    }).collect();

    let return_type = f.ret_type.as_ref()
        .map(syntype_to_sirtype)
        .unwrap_or(SirType::Void);

    let mut contracts = Vec::new();

    for req in &f.requires {
        contracts.push(SirContract {
            kind: "requires".to_string(),
            expression: expr_to_string(req),
            z3_verified: false,  // Verification status not tracked at AST level
        });
    }

    for ens in &f.ensures {
        contracts.push(SirContract {
            kind: "ensures".to_string(),
            expression: expr_to_string(ens),
            z3_verified: false,
        });
    }

    let attributes = f.attributes.iter().map(|a| a.name.to_string()).collect();
    let location = ident_to_location(&f.name);

    // We emit an empty body for now — full body lowering is future work.
    // The critical contract and signature metadata is preserved.
    SirFunction {
        name: f.name.to_string(),
        params,
        return_type,
        contracts,
        body: vec![],
        is_pub: f.is_pub,
        attributes,
        location,
    }
}

/// Walk the parsed SaltFile AST and produce a SirModule.
/// Extracts all top-level structs and functions, preserving
/// contracts, attributes, and type information.
pub fn extract_sir_from_ast(file: &SaltFile, module_name: &str) -> SirModule {
    let mut structs = Vec::new();
    let mut functions = Vec::new();

    for item in &file.items {
        match item {
            Item::Struct(s) => {
                structs.push(extract_sir_struct(s));
            }
            Item::Fn(f) => {
                functions.push(extract_sir_function(f));
            }
            Item::Impl(impl_item) => {
                // Extract methods from impl blocks
                match impl_item {
                    crate::grammar::SaltImpl::Methods { methods, .. } => {
                        for m in methods {
                            functions.push(extract_sir_function(m));
                        }
                    }
                    crate::grammar::SaltImpl::Trait { methods, .. } => {
                        for m in methods {
                            functions.push(extract_sir_function(m));
                        }
                    }
                    _ => {}
                }
            }
            _ => {} // Skip globals, consts, enums, externs, concepts, traits for now
        }
    }

    build_sir_module(module_name, structs, functions)
}
