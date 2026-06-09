import sys

with open('lettuce/tests/test_aof.salt', 'r') as f:
    content = f.read()

content = content.replace('let mut smap = StringMap_new();', 'let mut smap = StringMap_new();\n    puts("Calling Aof_replay..." as Ptr<u8>);')

with open('lettuce/tests/test_aof.salt', 'w') as f:
    f.write(content)

with open('lettuce/aof.salt', 'r') as f:
    content = f.read()

content = content.replace('if ctx.conn_ptr == 0 || ctx.fd == 0 {', 'puts("Aof_replay started" as Ptr<u8>);\n    if ctx.conn_ptr == 0 || ctx.fd == 0 { puts("EARLY EXIT" as Ptr<u8>);')

with open('lettuce/aof.salt', 'w') as f:
    f.write(content)

