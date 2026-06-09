import sys

with open('lettuce/aof.salt', 'r') as f:
    content = f.read()

content = content.replace("let mut conn = VfsConnection::connect();", "let conn = VfsConnection::connect();")
content = content.replace("let heap_conn = malloc(32) as Ptr<VfsConnection>;\n        heap_conn.write(conn);\n        ctx.write(AofContext { conn_ptr: heap_conn as u64, fd: fd });", "ctx.write(AofContext { conn_ptr: conn as u64, fd: fd });")

with open('lettuce/aof.salt', 'w') as f:
    f.write(content)

