import sys

with open('lettuce/aof.salt', 'r') as f:
    content = f.read()

content = content.replace("use std.fs.fs.{VfsConnection, FileHandle}", "use std.fs.fs.{VfsConnection, FileHandle}\nuse std.status.Status")
content = content.replace("return Result::Err(1);", "return Result::Err(Status::from_code(1));")

with open('lettuce/aof.salt', 'w') as f:
    f.write(content)
