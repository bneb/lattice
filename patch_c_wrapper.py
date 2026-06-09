import sys

with open('lettuce/tests/dummy_ebr.c', 'a') as f:
    f.write("\nvoid* VfsConnection_connect_ptr() {\n    extern void* std__fs__fs__VfsConnection_connect_ptr();\n    return std__fs__fs__VfsConnection_connect_ptr();\n}\n")

with open('lettuce/aof.salt', 'r') as f:
    content = f.read()

content = content.replace("use std.fs.fs.{VfsConnection, FileHandle, VfsConnection_connect_ptr}", "use std.fs.fs.{VfsConnection, FileHandle}")
content = content.replace("let conn_ptr = std.fs.fs.VfsConnection_connect_ptr();", "let conn_ptr = VfsConnection_connect_ptr();")

# Add extern fn VfsConnection_connect_ptr() -> Ptr<VfsConnection>;
content = content.replace("extern fn malloc(size: i64) -> Ptr<u8>;", "extern fn malloc(size: i64) -> Ptr<u8>;\nextern fn VfsConnection_connect_ptr() -> Ptr<VfsConnection>;")

with open('lettuce/aof.salt', 'w') as f:
    f.write(content)

