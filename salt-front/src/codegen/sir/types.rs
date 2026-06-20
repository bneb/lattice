// =============================================================================
// SIR Types — Salt Intermediate Representation
// =============================================================================
//
// SIR is a versioned, serializable representation of the Salt program that
// decouples the Salt compiler from MLIR/LLVM dialect deprecation. It captures
// the semantics of Salt programs at a level above MLIR ops, preserving:
//
//   - Z3-verified invariants and contracts
//   - Function signatures with requires/ensures
//   - Control flow (while, if, for, match)
//   - Type information sufficient for re-lowering
//
// This is the "LLVM-independent" checkpoint: if MLIR dialects change, we
// only need to update the SIR-to-MLIR translator, not the Salt compiler.
// =============================================================================

/// SIR version for forward compatibility.
pub const SIR_VERSION: u32 = 1;

/// Source location for a SIR definition (for Go-to-Definition).
#[derive(Debug, Clone, PartialEq)]
pub struct SirLocation {
    /// 1-indexed line number where the definition starts.
    pub line: usize,
    /// 0-indexed column number where the definition starts.
    pub column: usize,
    /// 1-indexed line number where the definition ends.
    pub end_line: usize,
    /// 0-indexed column number where the definition ends.
    pub end_column: usize,
}

/// A SIR value — the operand of instructions.
#[derive(Debug, Clone, PartialEq)]
pub enum SirValue {
    IntLiteral(i64),
    FloatLiteral(f64),
    BoolLiteral(bool),
    StringLiteral(String),
    Register(String),
    Null,
}

/// A SIR type — simplified type system for serialization.
#[derive(Debug, Clone, PartialEq)]
pub enum SirType {
    I32,
    I64,
    U32,
    U64,
    F64,
    Bool,
    Ptr(Box<SirType>),
    Void,
    Struct(String),
    Array(Box<SirType>, usize),
}

/// A SIR instruction — one operation in a block.
#[derive(Debug, Clone, PartialEq)]
pub enum SirInstruction {
    Assign {
        target: String,
        value: SirValue,
        ty: SirType,
    },
    BinaryOp {
        target: String,
        op: String,
        lhs: SirValue,
        rhs: SirValue,
    },
    Call {
        target: Option<String>,
        callee: String,
        args: Vec<SirValue>,
    },
    Return {
        value: Option<SirValue>,
    },
    While {
        condition_reg: String,
        verified_invariant: Option<String>,
        body: Vec<SirBlock>,
    },
    If {
        condition_reg: String,
        then_blocks: Vec<SirBlock>,
        else_blocks: Vec<SirBlock>,
    },
    Compare {
        target: String,
        op: String,
        lhs: SirValue,
        rhs: SirValue,
    },
    AtomicCas {
        target: String,
        addr: SirValue,
        expected: SirValue,
        desired: SirValue,
        ordering: String,
    },
    AtomicLoad {
        target: String,
        addr: SirValue,
        ordering: String,
    },
    AtomicStore {
        addr: SirValue,
        value: SirValue,
        ordering: String,
    },
}

/// A labeled basic block containing a sequence of instructions.
#[derive(Debug, Clone, PartialEq)]
pub struct SirBlock {
    pub label: String,
    pub instructions: Vec<SirInstruction>,
}

/// A SIR function parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct SirParam {
    pub name: String,
    pub ty: SirType,
}

/// A SIR function contract (requires/ensures).
#[derive(Debug, Clone, PartialEq)]
pub struct SirContract {
    pub kind: String,
    pub expression: String,
    pub z3_verified: bool,
}

/// A SIR function.
#[derive(Debug, Clone, PartialEq)]
pub struct SirFunction {
    pub name: String,
    pub params: Vec<SirParam>,
    pub return_type: SirType,
    pub contracts: Vec<SirContract>,
    pub body: Vec<SirBlock>,
    pub is_pub: bool,
    pub attributes: Vec<String>,
    /// Source location of the function definition.
    pub location: Option<SirLocation>,
}

/// A SIR struct definition.
#[derive(Debug, Clone, PartialEq)]
pub struct SirStruct {
    pub name: String,
    pub fields: Vec<SirParam>,
    pub attributes: Vec<String>,
    /// Source location of the struct definition.
    pub location: Option<SirLocation>,
}

/// A SIR module — the top-level compilation unit.
#[derive(Debug, Clone, PartialEq)]
pub struct SirModule {
    pub name: String,
    pub version: u32,
    pub structs: Vec<SirStruct>,
    pub functions: Vec<SirFunction>,
}

impl SirModule {
    pub fn new(name: &str) -> Self {
        SirModule {
            name: name.to_string(),
            version: SIR_VERSION,
            structs: Vec::new(),
            functions: Vec::new(),
        }
    }

    /// Serialize the module to a JSON string (manual — no serde dependency).
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        out.push_str("{\n");
        out.push_str(&format!("  \"name\": \"{}\",\n", self.name));
        out.push_str(&format!("  \"version\": {},\n", self.version));

        // Structs
        out.push_str("  \"structs\": [\n");
        for (i, s) in self.structs.iter().enumerate() {
            out.push_str(&format!("    {{\"name\": \"{}\", \"fields\": [", s.name));
            for (j, f) in s.fields.iter().enumerate() {
                out.push_str(&format!("{{\"name\": \"{}\", \"ty\": \"{:?}\"}}", f.name, f.ty));
                if j + 1 < s.fields.len() { out.push_str(", "); }
            }
            out.push_str("]}");
            if i + 1 < self.structs.len() { out.push(','); }
            out.push('\n');
        }
        out.push_str("  ],\n");

        // Functions
        out.push_str("  \"functions\": [\n");
        for (i, f) in self.functions.iter().enumerate() {
            out.push_str(&format!("    {{\"name\": \"{}\", \"params\": {}, \"return_type\": \"{:?}\", \"is_pub\": {}, \"blocks\": {}, \"contracts\": {}}}",
                f.name,
                f.params.len(),
                f.return_type,
                f.is_pub,
                f.body.len(),
                f.contracts.len(),
            ));
            if i + 1 < self.functions.len() { out.push(','); }
            out.push('\n');
        }
        out.push_str("  ]\n");
        out.push_str("}\n");
        out
    }
}

impl SirBlock {
    pub fn new(label: &str) -> Self {
        SirBlock {
            label: label.to_string(),
            instructions: Vec::new(),
        }
    }

    pub fn push(&mut self, inst: SirInstruction) {
        self.instructions.push(inst);
    }
}
