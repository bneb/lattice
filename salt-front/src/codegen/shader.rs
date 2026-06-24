//! Metal Shading Language (MSL) codegen for @shader functions.
//!
//! Salt functions annotated with `@shader(compute)` are compiled to MSL text
//! instead of LLVM IR. The MSL source is embedded as a global string constant
//! in the LLVM module. At runtime, the host loads this string via the Metal API
//! (`MTLDevice.makeLibrary(source:)`).
//!
//! ## Supported Salt → MSL translation subset
//!
//! | Salt                      | MSL                                        |
//! |---------------------------|--------------------------------------------|
//! | `i32`, `u32`              | `int`, `uint`                              |
//! | `f32`                     | `float`                                    |
//! | `Ptr<f32>` param          | `device float* [[buffer(N)]]`              |
//! | `+`, `-`, `*`, `/`        | same                                       |
//! | `if`/`else`, `while`      | same                                       |
//! | `thread_id()` intrinsic   | `thread_position_in_grid`                  |

use crate::grammar::{SaltFn, Stmt, SaltBlock, SaltIf, SaltElse, SaltWhile, SynType};
use crate::grammar::attr::{extract_shader_kind, extract_workgroup_size};
use crate::codegen::context::LoweringContext;

/// Result from shader compilation: (msl_source, host_glue_mlir)
pub struct ShaderOutput {
    /// Complete MSL kernel text
    pub msl_source: String,
    /// LLVM IR that stores MSL as a global string constant + accessor function
    pub host_glue: String,
    /// Shader entry point name in MSL
    pub entry_name: String,
}

/// Main entry point: emit a @shader function as MSL text + LLVM host glue
pub fn emit_shader_fn(_ctx: &mut LoweringContext, func: &SaltFn) -> Result<String, String> {
    let kind = extract_shader_kind(&func.attributes)
        .ok_or_else(|| "emit_shader_fn called on non-shader function".to_string())?;
    let workgroup_size = extract_workgroup_size(&func.attributes);
    let fn_name = func.name.to_string();

    // 1. Generate MSL source
    let msl = generate_msl(func, &kind, workgroup_size)?;

    // 2. Emit LLVM IR: store MSL as global string constant + thin accessor
    let escaped_msl = msl
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('"', "\\\"")
        .replace('\0', "\\00");
    let msl_len = msl.len() + 1; // +1 for null terminator
    let global_name = format!("__shader_msl_{}", fn_name);
    let accessor_name = format!("get_shader_msl_{}", fn_name);

    let mut out = String::new();

    // Global constant holding the MSL source text
    out.push_str(&format!(
        "  llvm.mlir.global internal constant @{}(\"{}\\00\") {{addr_space = 0 : i32}} : !llvm.array<{} x i8>\n",
        global_name, escaped_msl, msl_len
    ));

    // Accessor function: returns pointer to the MSL string
    out.push_str(&format!(
        "  func.func @{}() -> !llvm.ptr {{\n",
        accessor_name
    ));
    out.push_str(&format!(
        "    %addr = llvm.mlir.addressof @{} : !llvm.ptr\n",
        global_name
    ));
    out.push_str("    return %addr : !llvm.ptr\n");
    out.push_str("  }\n");

    Ok(out)
}

/// Generate complete MSL source text from a Salt function
pub fn generate_msl(func: &SaltFn, kind: &str, _workgroup_size: u32) -> Result<String, String> {
    let fn_name = func.name.to_string();
    let mut msl = String::new();

    // MSL header
    msl.push_str("#include <metal_stdlib>\n");
    msl.push_str("using namespace metal;\n\n");

    // Function signature
    let msl_qualifier = match kind {
        "compute" => "kernel",
        "vertex" => "vertex",
        "fragment" => "fragment",
        _ => return Err(format!("Unknown shader kind: {}", kind)),
    };

    msl.push_str(&format!("{} void {}(\n", msl_qualifier, fn_name));

    // Emit parameters with buffer bindings + thread_id
    let mut buffer_idx = 0u32;
    let args: Vec<_> = func.args.iter().collect();
    for (i, arg) in args.iter().enumerate() {
        let arg_name = arg.name.to_string();
        if let Some(ref ty) = arg.ty {
            let msl_ty = salt_type_to_msl_param(ty, buffer_idx)?;
            msl.push_str(&format!("    {}", msl_ty.replace("$NAME", &arg_name)));
            buffer_idx += 1;
        }
        if i < args.len() - 1 {
            msl.push(',');
        }
        msl.push('\n');
    }

    // Add thread position parameter for compute shaders
    if kind == "compute" {
        if !args.is_empty() {
            // Replace last \n with ,\n
            if msl.ends_with('\n') {
                msl.pop();
                if !msl.ends_with(',') {
                    msl.push(',');
                }
                msl.push('\n');
            }
        }
        msl.push_str("    uint tid [[thread_position_in_grid]]\n");
    }

    msl.push_str(") {\n");

    // Emit body
    let body_msl = emit_msl_block(&func.body, 1)?;
    msl.push_str(&body_msl);

    msl.push_str("}\n");

    Ok(msl)
}

/// Map a Salt SynType to an MSL parameter declaration with buffer binding
/// Returns a template string with $NAME placeholder for the parameter name
pub fn salt_type_to_msl_param(ty: &SynType, buffer_idx: u32) -> Result<String, String> {
    match ty {
        SynType::Pointer(inner) => {
            let inner_msl = salt_type_to_msl(inner)?;
            Ok(format!("device {}* $NAME [[buffer({})]]", inner_msl, buffer_idx))
        },
        SynType::Path(path) => {
            let name = path.segments.last()
                .map(|s| s.ident.to_string())
                .unwrap_or_default();
            match name.as_str() {
                "i32" => Ok(format!("constant int& $NAME [[buffer({})]]", buffer_idx)),
                "u32" => Ok(format!("constant uint& $NAME [[buffer({})]]", buffer_idx)),
                "f32" => Ok(format!("constant float& $NAME [[buffer({})]]", buffer_idx)),
                "i64" => Ok(format!("constant long& $NAME [[buffer({})]]", buffer_idx)),
                _ => Ok(format!("constant int& $NAME [[buffer({})]]", buffer_idx)),
            }
        },
        _ => Ok(format!("constant int& $NAME [[buffer({})]]", buffer_idx)),
    }
}

/// Map a Salt SynType to an MSL type name (for inner types)
pub fn salt_type_to_msl(ty: &SynType) -> Result<String, String> {
    match ty {
        SynType::Path(path) => {
            let name = path.segments.last()
                .map(|s| s.ident.to_string())
                .unwrap_or_default();
            match name.as_str() {
                "i32" => Ok("int".to_string()),
                "u32" => Ok("uint".to_string()),
                "i64" => Ok("long".to_string()),
                "u64" => Ok("ulong".to_string()),
                "f32" => Ok("float".to_string()),
                "f64" => Ok("double".to_string()),
                "u8" => Ok("uchar".to_string()),
                "i8" => Ok("char".to_string()),
                "bool" => Ok("bool".to_string()),
                other => Ok(other.to_string()),
            }
        },
        SynType::Pointer(inner) => {
            let inner_msl = salt_type_to_msl(inner)?;
            Ok(format!("device {}*", inner_msl))
        },
        _ => Ok("int".to_string()), // fallback
    }
}

/// Emit an MSL block (list of statements) with given indentation level
fn emit_msl_block(block: &SaltBlock, indent: usize) -> Result<String, String> {
    let mut out = String::new();
    for stmt in &block.stmts {
        out.push_str(&emit_msl_stmt(stmt, indent)?);
    }
    Ok(out)
}

/// Emit a single Salt statement as MSL
fn emit_msl_stmt(stmt: &Stmt, indent: usize) -> Result<String, String> {
    let pad = "    ".repeat(indent);
    match stmt {
        Stmt::Syn(syn_stmt) => {
            // Handle syn::Stmt variants (let bindings, expressions)
            match syn_stmt {
                syn::Stmt::Local(local) => {
                    let pat = quote::quote!(#local.pat).to_string();
                    // Clean up pattern: remove `mut ` prefix for MSL
                    let clean_pat = pat.replace("mut ", "");
                    if let Some(init) = &local.init {
                        let expr = &init.expr;
                        let expr_str = expr_to_msl(expr);
                        Ok(format!("{}auto {} = {};\n", pad, clean_pat.trim(), expr_str))
                    } else {
                        Ok(format!("{}int {};\n", pad, clean_pat.trim()))
                    }
                },
                syn::Stmt::Expr(expr, semi) => {
                    let expr_str = expr_to_msl(expr);
                    if semi.is_some() {
                        Ok(format!("{}{};\n", pad, expr_str))
                    } else {
                        Ok(format!("{}{}\n", pad, expr_str))
                    }
                },
                _ => Ok(String::new()),
            }
        },
        Stmt::Return(Some(expr)) => {
            let expr_str = expr_to_msl(expr);
            Ok(format!("{}return {};\n", pad, expr_str))
        },
        Stmt::Return(None) => {
            Ok(format!("{}return;\n", pad))
        },
        Stmt::If(salt_if) => emit_msl_if(salt_if, indent),
        Stmt::While(salt_while) => emit_msl_while(salt_while, indent),
        Stmt::Expr(expr, has_semi) => {
            let expr_str = expr_to_msl(expr);
            if *has_semi {
                Ok(format!("{}{};\n", pad, expr_str))
            } else {
                Ok(format!("{}{}\n", pad, expr_str))
            }
        },
        Stmt::Break => Ok(format!("{}break;\n", pad)),
        Stmt::Continue => Ok(format!("{}continue;\n", pad)),
        _ => Ok(format!("{}// unsupported statement\n", pad)),
    }
}

/// Emit a Salt if statement as MSL
fn emit_msl_if(salt_if: &SaltIf, indent: usize) -> Result<String, String> {
    let pad = "    ".repeat(indent);
    let cond = expr_to_msl(&salt_if.cond);
    let mut out = format!("{}if ({}) {{\n", pad, cond);
    out.push_str(&emit_msl_block(&salt_if.then_branch, indent + 1)?);
    
    if let Some(ref else_branch) = salt_if.else_branch {
        match else_branch.as_ref() {
            SaltElse::Block(block) => {
                out.push_str(&format!("{}}} else {{\n", pad));
                out.push_str(&emit_msl_block(block, indent + 1)?);
            },
            SaltElse::If(nested_if) => {
                out.push_str(&format!("{}}} else ", pad));
                out.push_str(&emit_msl_if(nested_if, indent)?);
                return Ok(out); // nested if already closes
            },
        }
    }
    
    out.push_str(&format!("{}}}\n", pad));
    Ok(out)
}

/// Emit a Salt while loop as MSL
fn emit_msl_while(salt_while: &SaltWhile, indent: usize) -> Result<String, String> {
    let pad = "    ".repeat(indent);
    let cond = expr_to_msl(&salt_while.cond);
    let mut out = format!("{}while ({}) {{\n", pad, cond);
    out.push_str(&emit_msl_block(&salt_while.body, indent + 1)?);
    out.push_str(&format!("{}}}\n", pad));
    Ok(out)
}

/// Convert a syn::Expr to MSL text
/// This is a simplified translation that handles:
/// - Binary ops (arithmetic, comparisons)
/// - Function calls (including thread_id() → tid)
/// - Field access (for buffer indexing)
/// - Literals
/// - Variable references
pub fn expr_to_msl(expr: &syn::Expr) -> String {
    match expr {
        syn::Expr::Binary(bin) => {
            let left = expr_to_msl(&bin.left);
            let right = expr_to_msl(&bin.right);
            let op = quote::quote!(#bin.op).to_string();
            format!("{} {} {}", left, op.trim(), right)
        },
        syn::Expr::Lit(lit) => quote::quote!(#lit).to_string(),
        syn::Expr::Path(path) => {
            let segs: Vec<String> = path.path.segments.iter().map(|s| s.ident.to_string()).collect();
            let name = segs.join("::");
            match name.as_str() {
                "thread_id" => "tid".to_string(),
                other => other.to_string(),
            }
        },
        syn::Expr::Call(call) => expr_to_msl_call(call),
        syn::Expr::MethodCall(mc) => expr_to_msl_method_call(mc),
        syn::Expr::Index(idx) => format!("{}[{}]", expr_to_msl(&idx.expr), expr_to_msl(&idx.index)),
        syn::Expr::Paren(p) => format!("({})", expr_to_msl(&p.expr)),
        syn::Expr::Unary(u) => format!("{}{}", quote::quote!(#u.op).to_string().trim(), expr_to_msl(&u.expr)),
        syn::Expr::Assign(assign) => format!("{} = {}", expr_to_msl(&assign.left), expr_to_msl(&assign.right)),
        syn::Expr::Field(field) => format!("{}.{}", expr_to_msl(&field.base), quote::quote!(#field.member)),
        syn::Expr::Cast(cast) => expr_to_msl_cast(cast),
        syn::Expr::Block(block) => {
            let stmts: Vec<String> = block.block.stmts.iter().map(|s| quote::quote!(#s).to_string()).collect();
            stmts.join("; ")
        },
        _ => quote::quote!(#expr).to_string(),
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_salt_type_to_msl_i32() {
        let ty: SynType = syn::parse_str("i32").expect("hardcoded i32 type string is valid SynType");
        assert_eq!(salt_type_to_msl(&ty).expect("salt_type_to_msl succeeds for i32"), "int");
    }

    #[test]
    fn test_salt_type_to_msl_f32() {
        let ty: SynType = syn::parse_str("f32").expect("hardcoded f32 type string is valid SynType");
        assert_eq!(salt_type_to_msl(&ty).expect("salt_type_to_msl succeeds for f32"), "float");
    }

    #[test]
    fn test_salt_type_to_msl_u32() {
        let ty: SynType = syn::parse_str("u32").expect("hardcoded u32 type string is valid SynType");
        assert_eq!(salt_type_to_msl(&ty).expect("salt_type_to_msl succeeds for u32"), "uint");
    }
}


fn expr_to_msl_call(call: &syn::ExprCall) -> String {
    let func_name = expr_to_msl(&call.func);
    if func_name == "tid" || func_name == "thread_id" {
        return "tid".to_string();
    }
    let args: Vec<String> = call.args.iter().map(expr_to_msl).collect();
    format!("{}({})", func_name, args.join(", "))
}

fn expr_to_msl_method_call(mc: &syn::ExprMethodCall) -> String {
    let receiver = expr_to_msl(&mc.receiver);
    let method = mc.method.to_string();
    let args: Vec<String> = mc.args.iter().map(expr_to_msl).collect();
    match method.as_str() {
        "read_at" => format!("{}[{}]", receiver, args.join(", ")),
        "write_at" => format!("{}[{}] = {}", receiver, 
            args.first().unwrap_or(&String::new()),
            args.get(1).unwrap_or(&String::new())),
        "offset" => format!("({} + {})", receiver, args.join(", ")),
        "read" => format!("*{}", receiver),
        "write" => format!("*{} = {}", receiver, args.join(", ")),
        _ => format!("{}.{}({})", receiver, method, args.join(", ")),
    }
}

fn expr_to_msl_cast(cast: &syn::ExprCast) -> String {
    let inner = expr_to_msl(&cast.expr);
    let ty = quote::quote!(#cast.ty).to_string();
    let msl_ty = match ty.trim() {
        "i32" => "int",
        "u32" => "uint",
        "f32" => "float",
        "i64" => "long",
        other => other,
    };
    format!("(({}){})", msl_ty, inner)
}
