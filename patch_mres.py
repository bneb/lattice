import re

with open('salt-front/src/codegen/expr/method_resolution.rs', 'r') as f:
    content = f.read()

# We want to fix the `if is_aggregate` block:
#         let is_aggregate = is_aggregate_type(&ty);
#         if is_aggregate {
#             (addr, Type::Reference(Box::new(ty), false))
#         } else {
#             let val = format!("%mres_load_{}", ctx.next_id());
#             let mlir_ty = ty.to_mlir_storage_type(ctx)?;
#             ctx.emit_load(out, &val, &addr, &mlir_ty);
#             (val, ty)
#         }

# Change to:
#         let is_aggregate = is_aggregate_type(&ty);
#         let is_ref_ssa = matches!(ty, Type::Reference(..)) && _kind == LValueKind::SSA;
#         if is_aggregate {
#             (addr, Type::Reference(Box::new(ty), false))
#         } else if is_ref_ssa {
#             (addr, ty)
#         } else {
#             ...

replacement = """
        let is_aggregate = is_aggregate_type(&ty);
        let is_ref_ssa = matches!(ty, Type::Reference(_, _)) && matches!(_kind, LValueKind::SSA);
        if is_aggregate {
            (addr, Type::Reference(Box::new(ty), false))
        } else if is_ref_ssa {
            (addr, ty)
        } else {
            let val = format!("%mres_load_{}", ctx.next_id());
"""

content = content.replace(
'''
        let is_aggregate = is_aggregate_type(&ty);
        if is_aggregate {
            (addr, Type::Reference(Box::new(ty), false))
        } else {
            let val = format!("%mres_load_{}", ctx.next_id());
''', replacement)

# We need to make sure I am replacing the original block because I modified it earlier to add println!
content = re.sub(
r'''        let is_aggregate = is_aggregate_type\(&ty\);.*?if is_aggregate \{
            \(addr, Type::Reference\(Box::new\(ty\), false\)\)
        \} else \{
            let val = format!\("%mres_load_\{\}", ctx\.next_id\(\)\);''',
replacement, content, flags=re.DOTALL)

with open('salt-front/src/codegen/expr/method_resolution.rs', 'w') as f:
    f.write(content)

