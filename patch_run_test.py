import sys

with open('scripts/run_test.sh', 'r') as f:
    content = f.read()

old_deps = '"lettuce/store.salt" "lettuce/resp.salt" "std/simd/mod.salt" "std/collections/string_map.salt"'
new_deps = '"lettuce/store.salt" "lettuce/resp.salt" "lettuce/aof.salt" "std/fs/fs.salt" "std/simd/mod.salt" "std/collections/string_map.salt"'

content = content.replace(old_deps, new_deps)

with open('scripts/run_test.sh', 'w') as f:
    f.write(content)

