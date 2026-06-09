import sys

with open('std/fs/fs.salt', 'r') as f:
    content = f.read()

content = content.replace("impl VfsConnection {", "pub fn VfsConnection_connect() -> VfsConnection {\n    return VfsConnection::connect();\n}\n\nimpl VfsConnection {")

with open('std/fs/fs.salt', 'w') as f:
    f.write(content)

with open('lettuce/aof.salt', 'r') as f:
    content = f.read()

content = content.replace("extern fn std__fs__fs__VfsConnection__connect() -> VfsConnection;", "")
content = content.replace("use std.fs.fs.{VfsConnection, FileHandle}", "use std.fs.fs.{VfsConnection, FileHandle, VfsConnection_connect}")
content = content.replace("let mut conn = std__fs__fs__VfsConnection__connect();", "let mut conn = VfsConnection_connect();")

with open('lettuce/aof.salt', 'w') as f:
    f.write(content)

