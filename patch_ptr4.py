import sys

with open('lettuce/aof.salt', 'r') as f:
    content = f.read()

content = content.replace("let conn_ptr = VfsConnection_connect_ptr();", "let conn_ptr = std.fs.fs.VfsConnection_connect_ptr();")

with open('lettuce/aof.salt', 'w') as f:
    f.write(content)

