import sys

with open('lettuce/aof.salt', 'r') as f:
    content = f.read()

content = content.replace("let total_read = res.unwrap();", "let total_read = res.unwrap();\n    puts(\"Total read:\" as Ptr<u8>);\n")
content = content.replace("if op == 1 { // SET", "puts(\"OP SET\" as Ptr<u8>);\n        if op == 1 { // SET")
content = content.replace("} else if op == 2 { // DEL", "} else if op == 2 { // DEL\n            puts(\"OP DEL\" as Ptr<u8>);")

with open('lettuce/aof.salt', 'w') as f:
    f.write(content)

