import sys

with open('lettuce/tests/test_aof.salt', 'r') as f:
    content = f.read()

content = content.replace("use lettuce.aof", "use std.fs.fs.{VfsConnection_connect}\nuse lettuce.aof")

with open('lettuce/tests/test_aof.salt', 'w') as f:
    f.write(content)

