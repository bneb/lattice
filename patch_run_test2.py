import sys

with open('scripts/run_test.sh', 'r') as f:
    content = f.read()

# Add test_aof to the is_standalone=true branch
content = content.replace('if [[ "$BASENAME" == *chase_lev* ]] || [[ "$BASENAME" == *sliding_window* ]]; then', 'if [[ "$BASENAME" == *chase_lev* ]] || [[ "$BASENAME" == *sliding_window* ]] || [[ "$BASENAME" == "test_aof" ]]; then')

with open('scripts/run_test.sh', 'w') as f:
    f.write(content)

with open('lettuce/aof.salt', 'r') as f:
    content = f.read()

content = content.replace("extern fn VfsConnection_connect_ptr() -> Ptr<VfsConnection>;", "")
content = content.replace("let conn_ptr = VfsConnection_connect_ptr();", "let mut conn = VfsConnection::connect();\n    let conn_ptr = malloc(32) as Ptr<VfsConnection>;\n    conn_ptr.write(conn);")
content = content.replace("let res = (conn_ptr as Ptr<VfsConnection>).read().open(\"lettuce.aof\\0\" as &u8);", "let res = conn.open(\"lettuce.aof\\0\" as &u8);")

with open('lettuce/aof.salt', 'w') as f:
    f.write(content)

