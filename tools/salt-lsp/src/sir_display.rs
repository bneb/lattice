//! SIR Display Formatting — hover text and type rendering
//!
//! Extracted from sir_index.rs to keep files under the project's 500-line limit.

use crate::sir_index::{
    SirFunction, SirStruct, SirType,
};

/// Format a SirType as a human-readable Salt type string.
pub fn format_type(ty: &SirType) -> String {
    match ty {
        SirType::I32 => "i32".to_string(),
        SirType::I64 => "i64".to_string(),
        SirType::U32 => "u32".to_string(),
        SirType::U64 => "u64".to_string(),
        SirType::F64 => "f64".to_string(),
        SirType::Bool => "bool".to_string(),
        SirType::Void => "void".to_string(),
        SirType::Ptr(inner) => format!("Ptr<{}>", format_type(inner)),
        SirType::Struct(name) => name.clone(),
        SirType::Array(inner, size) => format!("[{}; {}]", format_type(inner), size),
    }
}

/// Format a function signature for hover display.
pub fn format_function_hover(func: &SirFunction) -> String {
    let mut md = String::new();

    md.push_str("```salt\n");
    if func.is_pub {
        md.push_str("pub ");
    }
    md.push_str(&format!("fn {}(", func.name));
    let param_strs: Vec<String> = func.params
        .iter()
        .map(|p| format!("{}: {}", p.name, format_type(&p.ty)))
        .collect();
    md.push_str(&param_strs.join(", "));
    md.push_str(&format!(") -> {}\n", format_type(&func.return_type)));
    md.push_str("```\n");

    if !func.contracts.is_empty() {
        md.push_str("---\n**Formal Contracts:**\n\n");
        for contract in &func.contracts {
            let status_icon = if contract.z3_verified {
                "✅ *(Verified)*"
            } else {
                "⚠️ *(Runtime Assertion)*"
            };
            md.push_str(&format!("* `{}`: `{}` {}\n",
                contract.kind, contract.expression, status_icon));
        }
    }

    if !func.attributes.is_empty() {
        md.push_str("\n---\n**Attributes:** ");
        let attrs: Vec<String> = func.attributes.iter()
            .map(|a| format!("`{}`", a)).collect();
        md.push_str(&attrs.join(", "));
        md.push('\n');
    }

    md
}

/// Format a struct for hover display.
pub fn format_struct_hover(s: &SirStruct) -> String {
    let mut md = String::new();

    md.push_str("```salt\n");
    md.push_str(&format!("struct {} {{\n", s.name));
    for field in &s.fields {
        md.push_str(&format!("    {}: {},\n", field.name, format_type(&field.ty)));
    }
    md.push_str("}\n```\n");

    if !s.attributes.is_empty() {
        md.push_str("---\n**Attributes:** ");
        let attrs: Vec<String> = s.attributes.iter()
            .map(|a| format!("`{}`", a)).collect();
        md.push_str(&attrs.join(", "));
        md.push('\n');
    }

    md
}
