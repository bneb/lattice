import sys

with open('lettuce/aof.salt', 'r') as f:
    content = f.read()

content = content.replace("pub fn Aof_init() -> Result<AofContext> {", "pub fn Aof_init(ctx: Ptr<AofContext>) -> Result<u64> {")
content = content.replace("return Result::Ok(AofContext { conn_ptr: conn_ptr as u64, fd: fd });", """
        ctx.write(AofContext { conn_ptr: conn_ptr as u64, fd: fd });
        return Result::Ok(1);""")

# Update all AofContext args to Ptr<AofContext>
content = content.replace("fn write_all(ctx: AofContext", "fn write_all(ctx: Ptr<AofContext>")
content = content.replace("pub fn Aof_append_set(ctx: AofContext", "pub fn Aof_append_set(ctx: Ptr<AofContext>")
content = content.replace("pub fn Aof_append_del(ctx: AofContext", "pub fn Aof_append_del(ctx: Ptr<AofContext>")
content = content.replace("pub fn Aof_replay(ctx: AofContext", "pub fn Aof_replay(ctx: Ptr<AofContext>")

# Replace field accesses
content = content.replace("ctx.conn_ptr", "ctx.read().conn_ptr")
content = content.replace("ctx.fd", "ctx.read().fd")
content = content.replace('puts("Aof_replay started" as Ptr<u8>);\n    if ctx.read().conn_ptr == 0 || ctx.read().fd == 0 { puts("EARLY EXIT" as Ptr<u8>);', "if ctx.read().conn_ptr == 0 || ctx.read().fd == 0 {")

with open('lettuce/aof.salt', 'w') as f:
    f.write(content)


with open('lettuce/tests/test_aof.salt', 'r') as f:
    content = f.read()

old_test = """    let aof_res = Aof_init();
    if aof_res.is_err() {
        puts("Failed to init AOF" as Ptr<u8>);
        return 1;
    }
    let aof_ctx = aof_res.unwrap();"""
    
new_test = """    let aof_ctx = malloc(16) as Ptr<AofContext>;
    let aof_res = Aof_init(aof_ctx);
    if aof_res.is_err() {
        puts("Failed to init AOF" as Ptr<u8>);
        return 1;
    }"""
    
content = content.replace(old_test, new_test)
content = content.replace('let mut smap = StringMap_new();\n    puts("Calling Aof_replay..." as Ptr<u8>);', 'let mut smap = StringMap_new();')

with open('lettuce/tests/test_aof.salt', 'w') as f:
    f.write(content)


with open('lettuce/store.salt', 'r') as f:
    content = f.read()

content = content.replace("pub fn execute(aof_ctx: AofContext", "pub fn execute(aof_ctx: Ptr<AofContext>")

with open('lettuce/store.salt', 'w') as f:
    f.write(content)


with open('lettuce/src/server.salt', 'r') as f:
    content = f.read()

old_serv = """    let aof_res = Aof_init();
    let mut aof_ctx = AofContext { conn_ptr: 0, fd: 0 };
    if aof_res.is_ok() {
        aof_ctx = aof_res.unwrap();
        Aof_replay(aof_ctx, smap);"""
        
new_serv = """    let aof_ctx = malloc(16) as Ptr<std.fs.fs.AofContext>; // wait, just Ptr<AofContext>
    let aof_res = Aof_init(aof_ctx);
    if aof_res.is_ok() {
        Aof_replay(aof_ctx, smap);"""
        
content = content.replace(old_serv, new_serv)
content = content.replace("if aof_ctx.conn_ptr != 0", "if aof_ctx.read().conn_ptr != 0")
content = content.replace("let conn_ptr = aof_ctx.conn_ptr", "let conn_ptr = aof_ctx.read().conn_ptr")
content = content.replace("fn handle_client(aof_ctx: AofContext", "fn handle_client(aof_ctx: Ptr<AofContext>")
content = content.replace("Ptr<std.fs.fs.AofContext>", "Ptr<AofContext>")

with open('lettuce/src/server.salt', 'w') as f:
    f.write(content)

