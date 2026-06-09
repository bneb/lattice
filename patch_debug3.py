import sys

with open('lettuce/aof.salt', 'r') as f:
    content = f.read()

content = content.replace("let total_read = res.unwrap();", 'let total_read = res.unwrap();\n    extern fn printf(fmt: Ptr<u8>, arg: i64);\n    printf("Total read: %lld\\n" as Ptr<u8>, total_read as i64);\n')

with open('lettuce/aof.salt', 'w') as f:
    f.write(content)

