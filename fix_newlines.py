with open("tests/test_lettuce_integration.salt", "r") as f:
    text = f.read()

# I will just write a clean version of the test file
content = """// =============================================================================
// Test: Lettuce Database Integration
// =============================================================================

import user.lettuce.store
use std.collections.string_map.StringMap
use std.collections.string_map.StringMap_new
use std.collections.string_map.StringMap_drop
use std.core.str.StringView
use std.core.ptr.Ptr

extern fn memcmp(s1: Ptr<u8>, s2: Ptr<u8>, n: i64) -> i32;

fn main() -> i32 {
    let smap = StringMap_new();
    let mut out_buf: [u8; 1024] = [0; 1024];
    let out_ptr = (&out_buf[0]) as Ptr<u8>;

    // 1. SET foo bar
    let req1: StringView = "*3\\r\\n$3\\r\\nSET\\r\\n$3\\r\\nfoo\\r\\n$3\\r\\nbar\\r\\n";
    let res1 = store.execute(smap, req1, out_ptr);
    
    if res1.resp_len != 5 { return 1; }
    if memcmp(out_ptr, "+OK\\r\\n" as Ptr<u8>, 5) != 0 { return 2; }

    // 2. GET foo
    let req2: StringView = "*2\\r\\n$3\\r\\nGET\\r\\n$3\\r\\nfoo\\r\\n";
    let res2 = store.execute(smap, req2, out_ptr);

    if res2.resp_len != 9 { return 3; }
    if memcmp(out_ptr, "$3\\r\\nbar\\r\\n" as Ptr<u8>, 9) != 0 { return 4; }

    // 3. Inline PING
    let req3: StringView = "PING\\r\\n";
    let res3 = store.execute(smap, req3, out_ptr);

    if res3.resp_len != 7 { return 5; }
    if memcmp(out_ptr, "+PONG\\r\\n" as Ptr<u8>, 7) != 0 { return 6; }

    StringMap_drop(smap);
    return 0; // Success
}
"""

with open("tests/test_lettuce_integration.salt", "w") as f:
    f.write(content)

