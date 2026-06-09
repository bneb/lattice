import sys

with open('lettuce/aof.salt', 'r') as f:
    content = f.read()

content = content.replace("let mut conn = VfsConnection::connect();\n    let conn_ptr = malloc(32) as Ptr<VfsConnection>;\n    conn_ptr.write(conn);", "let conn_ptr = VfsConnection::connect();")

# Remove extern fn VfsConnection_connect_ptr() -> Ptr<VfsConnection>; if it exists
content = content.replace("extern fn VfsConnection_connect_ptr() -> Ptr<VfsConnection>;\n", "")

with open('lettuce/aof.salt', 'w') as f:
    f.write(content)

