import sys

with open('lettuce/src/server.salt', 'r') as f:
    content = f.read()

imports = "use std.collections.string_map::{StringMap_new, StringMap}\nuse lettuce.aof::{Aof_init, Aof_replay}"
content = content.replace("use std.collections.string_map::{StringMap_new, StringMap}", imports)

init_block = """    let mut smap = StringMap_new();

    puts("Initializing AOF persistence..." as Ptr<u8>);
    if Aof_init() {
        Aof_replay(smap);
        puts("AOF replay complete." as Ptr<u8>);
    } else {
        puts("WARN: Failed to initialize AOF!" as Ptr<u8>);
    }"""

content = content.replace("    let mut smap = StringMap_new();", init_block)

with open('lettuce/src/server.salt', 'w') as f:
    f.write(content)
