import sys

with open('lettuce/store.salt', 'r') as f:
    content = f.read()

aof_imports = """use std.collections.string_map::{StringMap, StringMap_set, StringMap_get, StringMap_value_at, StringMap_del}
use lettuce.aof::{Aof_append_set, Aof_append_del}"""

content = content.replace("use std.collections.string_map::{StringMap, StringMap_set, StringMap_get, StringMap_value_at, StringMap_del}", aof_imports)

set_replacement = """        StringMap_set(smap, key, val);
        Aof_append_set(key, val);
        let r = write_simple_string(out_buf, "OK" as StringView);"""

content = content.replace('        StringMap_set(smap, key, val);\n        let r = write_simple_string(out_buf, "OK" as StringView);', set_replacement)

del_replacement = """            let r = write_integer(out_buf, 1);
            Aof_append_del(key);
            return ExecResult { resp_len: r, input_consumed: consumed, context_ptr: 0 };"""

content = content.replace('            let r = write_integer(out_buf, 1);\n            return ExecResult { resp_len: r, input_consumed: consumed, context_ptr: 0 };', del_replacement)

with open('lettuce/store.salt', 'w') as f:
    f.write(content)
