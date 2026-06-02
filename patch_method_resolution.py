import sys

file_path = "salt-front/src/codegen/expr/method_resolution.rs"
with open(file_path, "r") as f:
    content = f.read()

content = content.replace("matches!(self_arg_ty, Type::Reference(_, _) | Type::Pointer { .. })", "matches!(self_arg_ty, Type::Reference(_, _))")

with open(file_path, "w") as f:
    f.write(content)
