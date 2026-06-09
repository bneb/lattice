import sys

with open('lettuce/tests/test_aof.salt', 'r') as f:
    content = f.read()

content = content.replace("use std.fs.fs.{VfsConnection_connect}", "use std.fs.fs.{VfsConnection, FileHandle}")

with open('lettuce/tests/test_aof.salt', 'w') as f:
    f.write(content)

