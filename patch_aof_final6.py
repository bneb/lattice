import sys

# 1. Fix salt-front/std/fs/fs.salt to be package std.fs.fs again
with open('salt-front/std/fs/fs.salt', 'r') as f:
    content = f.read()
content = content.replace("package std.fs\n", "package std.fs.fs\n")
with open('salt-front/std/fs/fs.salt', 'w') as f:
    f.write(content)

# 2. Fix lettuce/aof.salt to use mut conn and heap allocation for AofContext
with open('lettuce/aof.salt', 'r') as f:
    content = f.read()

content = content.replace("let conn = VfsConnection::connect();", "let mut conn = VfsConnection::connect();")
content = content.replace("ctx.write(AofContext { conn_ptr: conn as u64, fd: fd });", "let heap_conn = malloc(32) as Ptr<VfsConnection>;\n        heap_conn.write(conn);\n        ctx.write(AofContext { conn_ptr: heap_conn as u64, fd: fd });")

with open('lettuce/aof.salt', 'w') as f:
    f.write(content)

