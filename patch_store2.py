import sys

with open('lettuce/store.salt', 'r') as f:
    content = f.read()

content = content.replace("package lettuce.store\n\nuse std.core.ptr.Ptr", "package lettuce.store\n\nuse std.core.ptr.Ptr\nuse lettuce.aof.{Aof_append_set, Aof_append_del}")

with open('lettuce/store.salt', 'w') as f:
    f.write(content)
