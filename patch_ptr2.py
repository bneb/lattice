import sys

with open('std/fs/fs.salt', 'r') as f:
    content = f.read()

old_connect = """    pub fn connect() -> VfsConnection {
        // 1. Allocate 2 pages (8192 bytes)
        let memory = malloc(8192) as u64;"""
new_connect = """    pub fn connect() -> Ptr<VfsConnection> {
        // 1. Allocate 2 pages (8192 bytes)
        let memory = malloc(8192) as u64;"""
content = content.replace(old_connect, new_connect)

content = content.replace("return VfsConnection {", """let conn_ptr = malloc(32) as Ptr<VfsConnection>;
        conn_ptr.write(VfsConnection {""")
content = content.replace("seq: 0\n        };", "seq: 0\n        });\n        return conn_ptr;")

# Remove the VfsConnection_connect hack
content = content.replace("""pub fn VfsConnection_connect() -> VfsConnection {
    return VfsConnection::connect();
}

""", "")

with open('std/fs/fs.salt', 'w') as f:
    f.write(content)

with open('lettuce/aof.salt', 'r') as f:
    content = f.read()

content = content.replace("use std.fs.fs.{VfsConnection, FileHandle, VfsConnection_connect}", "use std.fs.fs.{VfsConnection, FileHandle}")
content = content.replace("let mut conn = std.fs.fs.VfsConnection_connect();\n    let conn_ptr = malloc(32) as Ptr<VfsConnection>;\n    conn_ptr.write(conn);", "let conn_ptr = VfsConnection::connect();")

with open('lettuce/aof.salt', 'w') as f:
    f.write(content)

