import sys

# Patch store.salt
with open('lettuce/store.salt', 'r') as f:
    content = f.read()

content = content.replace("use lettuce.aof.{Aof_append_set, Aof_append_del}", "use lettuce.aof.{AofContext, Aof_append_set, Aof_append_del}")
content = content.replace("pub fn execute(smap: Ptr<StringMap>, input: StringView, out_buf: Ptr<u8>) -> ExecResult {", "pub fn execute(aof_ctx: AofContext, smap: Ptr<StringMap>, input: StringView, out_buf: Ptr<u8>) -> ExecResult {")
content = content.replace("Aof_append_set(key, val);", "Aof_append_set(aof_ctx, key, val);")
content = content.replace("Aof_append_del(key);", "Aof_append_del(aof_ctx, key);")

with open('lettuce/store.salt', 'w') as f:
    f.write(content)

# Patch server.salt
with open('lettuce/src/server.salt', 'r') as f:
    content = f.read()

content = content.replace("use lettuce.aof.{Aof_init, Aof_replay, VFS_CONN_PTR}", "use lettuce.aof.{AofContext, Aof_init, Aof_replay}")
content = content.replace("fn handle_client(fd: i32, poll: &mut Poller, smap: Ptr<StringMap>, slab: &mut Slab<ClientSession>, ai_rx_ring: u64) {", "fn handle_client(aof_ctx: AofContext, fd: i32, poll: &mut Poller, smap: Ptr<StringMap>, slab: &mut Slab<ClientSession>, ai_rx_ring: u64) {")
content = content.replace("execute(smap, StringView::from_raw", "execute(aof_ctx, smap, StringView::from_raw")
content = content.replace("handle_client(event_fd, &mut poll, &mut smap, &mut slab, ai_rx_ring);", "handle_client(aof_ctx, event_fd, &mut poll, &mut smap, &mut slab, ai_rx_ring);")

init_old = """    puts("Initializing AOF persistence..." as Ptr<u8>);
    if Aof_init() {
        Aof_replay(smap);
        puts("AOF replay complete." as Ptr<u8>);
    } else {
        puts("WARN: Failed to initialize AOF!" as Ptr<u8>);
    }"""
    
init_new = """    puts("Initializing AOF persistence..." as Ptr<u8>);
    let aof_res = Aof_init();
    let mut aof_ctx = AofContext { conn_ptr: 0, fd: 0 };
    if aof_res.is_ok() {
        aof_ctx = aof_res.unwrap();
        Aof_replay(aof_ctx, smap);
        puts("AOF replay complete." as Ptr<u8>);
    } else {
        puts("WARN: Failed to initialize AOF!" as Ptr<u8>);
    }"""

content = content.replace(init_old, init_new)
content = content.replace("if VFS_CONN_PTR != 0 {", "if aof_ctx.conn_ptr != 0 {")
content = content.replace("let conn_ptr = VFS_CONN_PTR", "let conn_ptr = aof_ctx.conn_ptr")

with open('lettuce/src/server.salt', 'w') as f:
    f.write(content)

# Patch test_aof.salt
with open('lettuce/tests/test_aof.salt', 'r') as f:
    content = f.read()

content = content.replace("Aof_init, Aof_append_set", "AofContext, Aof_init, Aof_append_set")
test_old = """    if !Aof_init() {
        puts("Failed to init AOF" as Ptr<u8>);
        return 1;
    }
    
    Aof_append_set("key1" as StringView, "value1" as StringView);
    Aof_append_set("key2" as StringView, "value2" as StringView);
    Aof_append_del("key1" as StringView);
    
    // Now replay into a fresh StringMap
    let mut smap = StringMap_new();
    Aof_replay(smap);"""

test_new = """    let aof_res = Aof_init();
    if aof_res.is_err() {
        puts("Failed to init AOF" as Ptr<u8>);
        return 1;
    }
    let aof_ctx = aof_res.unwrap();
    
    Aof_append_set(aof_ctx, "key1" as StringView, "value1" as StringView);
    Aof_append_set(aof_ctx, "key2" as StringView, "value2" as StringView);
    Aof_append_del(aof_ctx, "key1" as StringView);
    
    // Now replay into a fresh StringMap
    let mut smap = StringMap_new();
    Aof_replay(aof_ctx, smap);"""

content = content.replace(test_old, test_new)

with open('lettuce/tests/test_aof.salt', 'w') as f:
    f.write(content)

