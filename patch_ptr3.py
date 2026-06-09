import sys

with open('std/fs/fs.salt', 'r') as f:
    content = f.read()

wrapper = """pub fn VfsConnection_connect_ptr() -> Ptr<VfsConnection> {
    let conn_ptr = malloc(32) as Ptr<VfsConnection>;
    conn_ptr.write(VfsConnection::connect());
    return conn_ptr;
}

impl VfsConnection {"""

content = content.replace("impl VfsConnection {", wrapper)

with open('std/fs/fs.salt', 'w') as f:
    f.write(content)

with open('lettuce/aof.salt', 'r') as f:
    content = f.read()

content = content.replace("use std.fs.fs.{VfsConnection, FileHandle}", "use std.fs.fs.{VfsConnection, FileHandle, VfsConnection_connect_ptr}")
content = content.replace("let conn_ptr = VfsConnection::connect();", "let conn_ptr = VfsConnection_connect_ptr();")

with open('lettuce/aof.salt', 'w') as f:
    f.write(content)

