import sys

with open('std/fs/fs.salt', 'r') as f:
    content = f.read()

content = content.replace("let memory = malloc(8192);", "let memory = malloc(8192) as u64;")

with open('std/fs/fs.salt', 'w') as f:
    f.write(content)
