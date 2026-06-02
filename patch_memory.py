import sys
import re

file_path = "salt-front/src/codegen/intrinsics/memory.rs"
with open(file_path, "r") as f:
    content = f.read()

# We need to replace:
#             let struct_ty = ptr_ty.to_mlir_storage_type(ctx)?;
#             
#             let raw_ptr = if struct_ty == "!llvm.ptr" {
#                 ptr.clone()
#             } else {
#                 let val_i64 = if struct_ty == "i64" {
#                     ptr.clone()
#                 } else {

replacement = """            let struct_ty = ptr_ty.to_mlir_storage_type(ctx)?;
            
            let is_aggregate = matches!(ptr_ty, Type::Struct(_) | Type::Concrete(_, _) | Type::Array(_, _, _));
            let loaded_ptr = if is_aggregate {
                let load_val = format!("%loaded_struct_{}", ctx.next_id());
                out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\\n", load_val, ptr, struct_ty));
                load_val
            } else {
                ptr.clone()
            };
            
            let raw_ptr = if struct_ty == "!llvm.ptr" {
                loaded_ptr
            } else {
                let val_i64 = if struct_ty == "i64" {
                    loaded_ptr
                } else {"""

content = content.replace("""            let struct_ty = ptr_ty.to_mlir_storage_type(ctx)?;
            
            let raw_ptr = if struct_ty == "!llvm.ptr" {
                ptr.clone()
            } else {
                let val_i64 = if struct_ty == "i64" {
                    ptr.clone()
                } else {""", replacement)

# Do the same for ptr_read
content = content.replace("""            let struct_ty = ptr_ty.to_mlir_storage_type(ctx)?;
            
            let raw_ptr = if struct_ty == "!llvm.ptr" {
                 ptr.clone()
            } else {
                let val_i64 = if struct_ty == "i64" {
                    ptr.clone()
                } else {""", """            let struct_ty = ptr_ty.to_mlir_storage_type(ctx)?;
            
            let is_aggregate = matches!(ptr_ty, Type::Struct(_) | Type::Concrete(_, _) | Type::Array(_, _, _));
            let loaded_ptr = if is_aggregate {
                let load_val = format!("%loaded_struct_{}", ctx.next_id());
                out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\\n", load_val, ptr, struct_ty));
                load_val
            } else {
                ptr.clone()
            };
            
            let raw_ptr = if struct_ty == "!llvm.ptr" {
                 loaded_ptr
            } else {
                let val_i64 = if struct_ty == "i64" {
                    loaded_ptr
                } else {""")

# Do the same for ptr_write
content = content.replace("""            let struct_ty = ptr_ty.to_mlir_storage_type(ctx)?;
            
            let raw_ptr = if struct_ty == "!llvm.ptr" {
                 ptr.clone()
            } else {
                let val_i64 = if struct_ty == "i64" {
                    ptr.clone()
                } else {""", """            let struct_ty = ptr_ty.to_mlir_storage_type(ctx)?;
            
            let is_aggregate = matches!(ptr_ty, Type::Struct(_) | Type::Concrete(_, _) | Type::Array(_, _, _));
            let loaded_ptr = if is_aggregate {
                let load_val = format!("%loaded_struct_{}", ctx.next_id());
                out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\\n", load_val, ptr, struct_ty));
                load_val
            } else {
                ptr.clone()
            };
            
            let raw_ptr = if struct_ty == "!llvm.ptr" {
                 loaded_ptr
            } else {
                let val_i64 = if struct_ty == "i64" {
                    loaded_ptr
                } else {""")


with open(file_path, "w") as f:
    f.write(content)

