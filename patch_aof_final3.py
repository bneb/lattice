import sys

with open('lettuce/aof.salt', 'r') as f:
    content = f.read()

content = content.replace("let res = conn.open", "let res = conn_ptr.open")

with open('lettuce/aof.salt', 'w') as f:
    f.write(content)

