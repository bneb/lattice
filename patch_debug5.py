import sys

with open('lettuce/aof.salt', 'r') as f:
    content = f.read()

content = content.replace("pub fn Aof_init(ctx: Ptr<AofContext>) -> Result<u64> {", "pub fn Aof_init(ctx: Ptr<AofContext>) -> i64 {")
content = content.replace("return Result::Ok(1);", "return 0;")
content = content.replace("return Result::Err(map_status(res.status));", "return -1;")

with open('lettuce/aof.salt', 'w') as f:
    f.write(content)

with open('lettuce/tests/test_aof.salt', 'r') as f:
    content = f.read()

content = content.replace("if aof_res.is_err() {", "if aof_res < 0 {")

with open('lettuce/tests/test_aof.salt', 'w') as f:
    f.write(content)

with open('lettuce/src/server.salt', 'r') as f:
    content = f.read()

content = content.replace("if aof_res.is_ok() {", "if aof_res == 0 {")

with open('lettuce/src/server.salt', 'w') as f:
    f.write(content)

