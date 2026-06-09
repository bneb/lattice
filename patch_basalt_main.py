import sys

with open('user/basalt/main.salt', 'r') as f:
    content = f.read()

content = content.replace("package user.basalt.main", "package main")
content += "\nfn main() -> i32 { return 0; }\n"

with open('user/basalt/main.salt', 'w') as f:
    f.write(content)

