import sys

with open('lettuce/aof.salt', 'r') as f:
    content = f.read()

content = content.replace("let res = VfsConnection::open(conn, \"lettuce.aof\\0\" as &u8);", "let res = conn.open(\"lettuce.aof\\0\" as &u8);")

with open('lettuce/aof.salt', 'w') as f:
    f.write(content)

