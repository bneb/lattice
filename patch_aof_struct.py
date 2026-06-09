import sys

with open('lettuce/aof.salt', 'r') as f:
    content = f.read()

struct_def = """pub struct AofContext {
    pub conn_ptr: u64,
    pub fd: u64
}

pub fn Aof_init() -> Result<AofContext> {
    let mut conn = VfsConnection::connect();
    let conn_ptr = malloc(32) as Ptr<VfsConnection>;
    conn_ptr.write(conn);
    
    let res = (conn_ptr as Ptr<VfsConnection>).read().open("lettuce.aof\\0" as &u8);
    if res.is_ok() {
        let fd = res.unwrap().fd;
        return Result::Ok(AofContext { conn_ptr: conn_ptr as u64, fd: fd });
    }
    return Result::Err(1); // Error
}

fn write_all(ctx: AofContext, buf: Ptr<u8>, size: u64) {
    if ctx.conn_ptr == 0 || ctx.fd == 0 {
        return;
    }
    let conn_ptr = ctx.conn_ptr as Ptr<VfsConnection>;
    let fh = FileHandle { fd: ctx.fd };
    let mut written: u64 = 0;
    while written < size {
        let res = conn_ptr.read().write(&fh, buf.offset(written as i64), size - written);
        if res.is_ok() {
            written = written + res.unwrap();
        } else {
            break;
        }
    }
}

pub fn Aof_append_set(ctx: AofContext, key: StringView, val: StringView) {
    if ctx.conn_ptr == 0 { return; }
    let total_size = 1 + 4 + 4 + key.length() + val.length();
    let buf = malloc(total_size);
    
    buf.write(1); // SET
    *((buf.offset(1) as u64) as &mut u32) = key.length() as u32;
    *((buf.offset(5) as u64) as &mut u32) = val.length() as u32;
    
    memmove(buf.offset(9), key.ptr, key.length());
    memmove(buf.offset(9 + key.length()), val.ptr, val.length());
    
    write_all(ctx, buf, total_size as u64);
}

pub fn Aof_append_del(ctx: AofContext, key: StringView) {
    if ctx.conn_ptr == 0 { return; }
    let total_size = 1 + 4 + key.length();
    let buf = malloc(total_size);
    
    buf.write(2); // DEL
    *((buf.offset(1) as u64) as &mut u32) = key.length() as u32;
    
    memmove(buf.offset(5), key.ptr, key.length());
    
    write_all(ctx, buf, total_size as u64);
}

pub fn Aof_replay(ctx: AofContext, smap: Ptr<StringMap>) {
    if ctx.conn_ptr == 0 || ctx.fd == 0 {
        return;
    }
    let conn_ptr = ctx.conn_ptr as Ptr<VfsConnection>;
    let fh = FileHandle { fd: ctx.fd };
    
    let buf = malloc(1024 * 1024 * 10);
    let res = conn_ptr.read().read(&fh, buf, 1024 * 1024 * 10);
    
    if res.is_err() { return; }
    
    let total_read = res.unwrap();
    let mut cursor: u64 = 0;
    
    while cursor < total_read {
        let op = buf.offset(cursor as i64).read();
        if op == 1 { // SET
            let klen = *((buf.offset((cursor + 1) as i64) as u64) as &u32) as i64;
            let vlen = *((buf.offset((cursor + 5) as i64) as u64) as &u32) as i64;
            let key_ptr = buf.offset((cursor + 9) as i64);
            let val_ptr = buf.offset((cursor + 9 + klen as u64) as i64);
            
            let key_sv = StringView::from_raw(key_ptr, klen);
            let val_sv = StringView::from_raw(val_ptr, vlen);
            
            StringMap_set(smap, key_sv, val_sv);
            
            cursor = cursor + 9 + (klen as u64) + (vlen as u64);
        } else if op == 2 { // DEL
            let klen = *((buf.offset((cursor + 1) as i64) as u64) as &u32) as i64;
            let key_ptr = buf.offset((cursor + 5) as i64);
            
            let key_sv = StringView::from_raw(key_ptr, klen);
            StringMap_del(smap, key_sv);
            
            cursor = cursor + 5 + (klen as u64);
        } else {
            break;
        }
    }
}"""

content = content[0:content.find("pub global VFS_CONN_PTR")] + struct_def

with open('lettuce/aof.salt', 'w') as f:
    f.write(content)
