// =============================================================================
// TDD Tests: SIR Translation Boundary — Structural Verification
// =============================================================================
//
// Tests the SIR type system and emitter in isolation (pure Rust, no Salt
// compilation needed). Validates:
//
//   Layer 1: SIR types construct correctly
//   Layer 2: SIR emitter produces valid instructions
//   Layer 3: While loop with invariant preserves Z3 data
//   Layer 4: SIR module serializes to valid JSON
//   Layer 5: Function round-trip (construct → serialize → inspect)
//
// =============================================================================

#[cfg(test)]
mod tests {
    use crate::codegen::sir::types::*;
    use crate::codegen::sir::sir_emit::*;

    // =========================================================================
    // LAYER 1: Basic Type Construction
    // =========================================================================

    #[test]
    fn test_sir_module_new() {
        let module = SirModule::new("test_module");
        assert_eq!(module.name, "test_module");
        assert_eq!(module.version, SIR_VERSION);
        assert!(module.functions.is_empty());
        assert!(module.structs.is_empty());
    }

    #[test]
    fn test_sir_block_new() {
        let mut block = SirBlock::new("entry");
        assert_eq!(block.label, "entry");
        assert!(block.instructions.is_empty());

        block.push(emit_sir_return(Some(SirValue::IntLiteral(0))));
        assert_eq!(block.instructions.len(), 1);
    }

    #[test]
    fn test_sir_version_is_1() {
        assert_eq!(SIR_VERSION, 1, "SIR version must be 1 for initial release");
    }

    // =========================================================================
    // LAYER 2: Instruction Emission
    // =========================================================================

    #[test]
    fn test_emit_sir_return_with_value() {
        let inst = emit_sir_return(Some(SirValue::IntLiteral(42)));
        match inst {
            SirInstruction::Return { value: Some(SirValue::IntLiteral(42)) } => {},
            _ => panic!("Expected Return with IntLiteral(42), got: {:?}", inst),
        }
    }

    #[test]
    fn test_emit_sir_call() {
        let inst = emit_sir_call(
            Some("result"),
            "siphash_2_4",
            vec![SirValue::Register("k0".into()), SirValue::Register("k1".into())],
        );
        match inst {
            SirInstruction::Call { target, callee, args } => {
                assert_eq!(target, Some("result".to_string()));
                assert_eq!(callee, "siphash_2_4");
                assert_eq!(args.len(), 2);
            },
            _ => panic!("Expected Call, got: {:?}", inst),
        }
    }

    #[test]
    fn test_emit_sir_binop() {
        let inst = emit_sir_binop(
            "sum",
            "+",
            SirValue::Register("a".into()),
            SirValue::Register("b".into()),
        );
        match inst {
            SirInstruction::BinaryOp { target, op, .. } => {
                assert_eq!(target, "sum");
                assert_eq!(op, "+");
            },
            _ => panic!("Expected BinaryOp, got: {:?}", inst),
        }
    }

    #[test]
    fn test_emit_sir_assign() {
        let inst = emit_sir_assign("x", SirValue::IntLiteral(100), SirType::I64);
        match inst {
            SirInstruction::Assign { target, value, ty } => {
                assert_eq!(target, "x");
                assert_eq!(value, SirValue::IntLiteral(100));
                assert_eq!(ty, SirType::I64);
            },
            _ => panic!("Expected Assign, got: {:?}", inst),
        }
    }

    // =========================================================================
    // LAYER 3: While Loop with Z3 Invariant
    // =========================================================================

    #[test]
    fn test_emit_sir_while_with_invariant() {
        let mut body = SirBlock::new("while.body");
        body.push(emit_sir_binop("i", "+", SirValue::Register("i".into()), SirValue::IntLiteral(1)));

        let inst = emit_sir_while(
            "cond_i_lt_10",
            Some("i >= 0"),
            vec![body],
        );

        match inst {
            SirInstruction::While { condition_reg, verified_invariant, body } => {
                assert_eq!(condition_reg, "cond_i_lt_10");
                assert_eq!(verified_invariant, Some("i >= 0".to_string()));
                assert_eq!(body.len(), 1);
                assert_eq!(body[0].label, "while.body");
                assert_eq!(body[0].instructions.len(), 1);
            },
            _ => panic!("Expected While, got: {:?}", inst),
        }
    }

    #[test]
    fn test_emit_sir_while_without_invariant() {
        let inst = emit_sir_while("cond", None, vec![]);
        match inst {
            SirInstruction::While { verified_invariant, .. } => {
                assert_eq!(verified_invariant, None);
            },
            _ => panic!("Expected While, got: {:?}", inst),
        }
    }

    // =========================================================================
    // LAYER 4: JSON Serialization
    // =========================================================================

    #[test]
    fn test_sir_module_to_json_contains_version() {
        let module = SirModule::new("test");
        let json = module.to_json();

        assert!(json.contains("\"version\": 1"), "JSON must contain version 1. Got:\n{}", json);
        assert!(json.contains("\"name\": \"test\""), "JSON must contain module name. Got:\n{}", json);
    }

    #[test]
    fn test_sir_module_to_json_with_function() {
        let func = emit_sir_function(
            "main",
            vec![],
            SirType::I32,
            vec![],
            vec![SirBlock::new("entry")],
            false,
            vec![],
        );

        let mut module = SirModule::new("main");
        module.functions.push(func);

        let json = module.to_json();
        assert!(json.contains("\"name\": \"main\""), "JSON must contain function name");
        assert!(json.contains("\"blocks\": 1"), "JSON must show 1 block");
    }

    // =========================================================================
    // LAYER 5: Full Function Round-Trip
    // =========================================================================

    #[test]
    fn test_sir_steal_function_structure() {
        // Build a simplified SIR representation of steal()
        let mut entry = SirBlock::new("entry");
        entry.push(emit_sir_assign("t", SirValue::Register("q.top".into()), SirType::I64));
        entry.push(emit_sir_assign("b", SirValue::Register("q.bottom".into()), SirType::I64));
        entry.push(emit_sir_binop("size", "-", SirValue::Register("b".into()), SirValue::Register("t".into())));
        entry.push(emit_sir_compare("cond", "<=", SirValue::Register("size".into()), SirValue::IntLiteral(0)));

        let mut empty_block = SirBlock::new("empty");
        empty_block.push(emit_sir_return(Some(SirValue::Null)));

        let mut steal_block = SirBlock::new("steal");
        steal_block.push(emit_sir_binop("index", "&", SirValue::Register("t".into()), SirValue::Register("mask".into())));
        steal_block.push(emit_sir_atomic_cas(
            "won",
            SirValue::Register("q.top".into()),
            SirValue::Register("t".into()),
            SirValue::Register("t_plus_1".into()),
            "Acquire",
        ));
        steal_block.push(emit_sir_return(Some(SirValue::Register("task".into()))));

        entry.push(emit_sir_if("cond", vec![empty_block], vec![steal_block]));

        let func = emit_sir_function(
            "steal",
            vec![
                SirParam { name: "q".into(), ty: SirType::Ptr(Box::new(SirType::Struct("WorkDeque".into()))) },
            ],
            SirType::Ptr(Box::new(SirType::Void)),
            vec![
                SirContract {
                    kind: "requires".into(),
                    expression: "q != null".into(),
                    z3_verified: true,
                },
                SirContract {
                    kind: "requires".into(),
                    expression: "q.mask > 0".into(),
                    z3_verified: true,
                },
            ],
            vec![entry],
            true,
            vec!["no_mangle".into()],
        );

        assert_eq!(func.name, "steal");
        assert_eq!(func.params.len(), 1);
        assert_eq!(func.contracts.len(), 2);
        assert!(func.contracts[0].z3_verified);
        assert_eq!(func.body.len(), 1);
        assert!(func.is_pub);

        // Verify JSON output
        let mut module = SirModule::new("kernel.sched.chase_lev");
        module.functions.push(func);
        let json = module.to_json();
        assert!(json.contains("steal"), "JSON must contain function name 'steal'");
        assert!(json.contains("\"contracts\": 2"), "JSON must show 2 contracts");
    }

    // =========================================================================
    // LAYER 6: Atomic Operations
    // =========================================================================

    #[test]
    fn test_emit_sir_atomic_cas() {
        let inst = emit_sir_atomic_cas(
            "result",
            SirValue::Register("addr".into()),
            SirValue::IntLiteral(0),
            SirValue::IntLiteral(1),
            "SeqCst",
        );
        match inst {
            SirInstruction::AtomicCas { target, ordering, .. } => {
                assert_eq!(target, "result");
                assert_eq!(ordering, "SeqCst");
            },
            _ => panic!("Expected AtomicCas, got: {:?}", inst),
        }
    }
}
