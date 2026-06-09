import sys

with open('lettuce/aof.salt', 'r') as f:
    content = f.read()

content = content.replace("let conn_ptr = VfsConnection::connect();", "let mut conn = VfsConnection::connect();")
content = content.replace("let res = conn_ptr.open(\"lettuce.aof\\0\" as &u8);", "let res = conn.open(\"lettuce.aof\\0\" as &u8);")
content = content.replace("ctx.write(AofContext { conn_ptr: conn_ptr as u64, fd: fd });", "let heap_conn = malloc(32) as Ptr<VfsConnection>;\n        heap_conn.write(conn);\n        ctx.write(AofContext { conn_ptr: heap_conn as u64, fd: fd });")

with open('lettuce/aof.salt', 'w') as f:
    f.write(content)

