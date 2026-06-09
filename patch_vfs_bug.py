import sys

with open('lettuce/aof.salt', 'r') as f:
    content = f.read()

content = content.replace("extern fn printf(fmt: Ptr<u8>, arg: i64);", "extern fn printf(fmt: Ptr<u8>, arg: i64);\nextern fn std__fs__fs__VfsConnection__connect() -> VfsConnection;")
content = content.replace("let mut conn = VfsConnection::connect();", "let mut conn = std__fs__fs__VfsConnection__connect();")

with open('lettuce/aof.salt', 'w') as f:
    f.write(content)

