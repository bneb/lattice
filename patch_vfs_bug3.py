import sys

with open('lettuce/aof.salt', 'r') as f:
    content = f.read()

content = content.replace("let mut conn = VfsConnection_connect();", "let mut conn = std.fs.fs.VfsConnection_connect();")

with open('lettuce/aof.salt', 'w') as f:
    f.write(content)

