import sys

with open('lettuce/aof.salt', 'r') as f:
    content = f.read()

content = content.replace("let res = conn.open(\"lettuce.aof\\0\" as &u8);", "let res = conn.open(\"lettuce.aof\\0\" as &u8);")

content = content.replace("return Result::Err(Status::from_code(1)); // Error", "return -1; // Error")

with open('lettuce/aof.salt', 'w') as f:
    f.write(content)

