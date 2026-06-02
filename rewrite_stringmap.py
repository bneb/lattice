import re

with open("std/collections/string_map.salt", "r") as f:
    text = f.read()

# 1. We keep StringMap struct exactly as it is, BUT we rename it to StringMapInner.
# Actually, let's keep StringMap struct name the same!
# We just DELETE `impl StringMap {` and replace all methods with standalone functions.
# Wait, `StringMap` is perfectly fine as a struct name.

text = text.replace("impl StringMap {", "")
text = text.replace("pub fn new() -> StringMap {", "pub fn StringMap_new() -> Ptr<StringMap> {")
text = text.replace("pub fn with_capacity(min_cap: i64) -> StringMap {", "pub fn StringMap_with_capacity(min_cap: i64) -> Ptr<StringMap> {")
text = text.replace("fn arena_view(&self, off: i64, len: i64) -> StringView {", "fn StringMap_arena_view(self: StringMap, off: i64, len: i64) -> StringView {")
text = text.replace("fn mirror_ctrl(&mut self, idx: i64, tag: i8) {", "fn StringMap_mirror_ctrl(mut self: StringMap, idx: i64, tag: i8) -> StringMap {")
text = text.replace("fn key_eq(&self, slot: i64, needle: StringView) -> bool {", "fn StringMap_key_eq(self: StringMap, slot: i64, needle: StringView) -> bool {")
text = text.replace("pub fn get(&self, key: StringView) -> i64 {", "pub fn StringMap_get(smap: Ptr<StringMap>, key: StringView) -> i64 {\\n        let self = smap.read();")
text = text.replace("pub fn value_at(&self, slot: i64) -> StringView {", "pub fn StringMap_value_at(smap: Ptr<StringMap>, slot: i64) -> StringView {\\n        let self = smap.read();")
text = text.replace("pub fn set(&mut self, key: StringView, val: StringView) {", "pub fn StringMap_set(smap: Ptr<StringMap>, key: StringView, val: StringView) {\\n        let mut self = smap.read();")
text = text.replace("pub fn del(&mut self, key: StringView) -> bool {", "pub fn StringMap_del(smap: Ptr<StringMap>, key: StringView) -> bool {\\n        let mut self = smap.read();")
text = text.replace("fn grow(&mut self) {", "fn StringMap_grow(mut self: StringMap) -> StringMap {")
text = text.replace("pub fn length(&self) -> i64 {", "pub fn StringMap_length(smap: Ptr<StringMap>) -> i64 {\\n        let self = smap.read();")
text = text.replace("pub fn is_empty(&self) -> bool {", "pub fn StringMap_is_empty(smap: Ptr<StringMap>) -> bool {\\n        let self = smap.read();")
text = text.replace("pub fn drop(&mut self) {", "pub fn StringMap_drop(smap: Ptr<StringMap>) {\\n        let mut self = smap.read();")

# Fix new / with_capacity returns
text = text.replace("return StringMap::with_capacity(16);", "return StringMap_with_capacity(16);")
text = text.replace("        return StringMap {", "        let inner = StringMap {")

# At the end of with_capacity:
with_capacity_end = """        let inner = StringMap {
            ctrl: ctrl,
            key_offsets: key_off,
            key_lens: key_len,
            val_offsets: val_off,
            val_lens: val_len,
            cap: cap,
            cap_mask: cap - 1,
            len: 0,
            growth_left: growth,
            data_arena: arena,
            arena_cap: arena_cap,
            arena_used: 0,
        };
        let p = malloc(96) as Ptr<StringMap>;
        p.write(inner);
        return p;"""

# Replace the block inside with_capacity
text = re.sub(r'        return StringMap \{\n.*?(?=    // =)', with_capacity_end + "\n    }\n\n", text, flags=re.DOTALL)

# Fix internal method calls inside StringMap
text = text.replace("self.mirror_ctrl(ins_idx, tag);", "self = StringMap_mirror_ctrl(self, ins_idx, tag);")
text = text.replace("self.mirror_ctrl(first_deleted, tag);", "self = StringMap_mirror_ctrl(self, first_deleted, tag);")
text = text.replace("self.mirror_ctrl(idx, SM_DELETED());", "self = StringMap_mirror_ctrl(self, idx, SM_DELETED());")
text = text.replace("self.key_eq(idx, key_sv2)", "StringMap_key_eq(self, idx, key_sv2)")
text = text.replace("self.arena_view(off, len)", "StringMap_arena_view(self, off, len)")
text = text.replace("self.grow();", "self = StringMap_grow(self);")

# We need to add smap.write(self) at the end of mutating public methods!
# set()
text = text.replace("return;\\n                }\\n            }\\n\\n            if ctrl_byte == SM_DELETED()", "smap.write(self);\\n                    return;\\n                }\\n            }\\n\\n            if ctrl_byte == SM_DELETED()")
text = text.replace("self.growth_left = self.growth_left - 1;\\n                }\\n                return;", "self.growth_left = self.growth_left - 1;\\n                }\\n                smap.write(self);\\n                return;")
text = text.replace("self.len = self.len + 1;\\n        }", "self.len = self.len + 1;\\n        }\\n        smap.write(self);\\n    }")

# del()
text = text.replace("return true;\\n                }", "smap.write(self);\\n                    return true;\\n                }")
# del ends with return false;
text = text.replace("return false;\\n    }\\n\\n    // =================================================================", "smap.write(self);\\n        return false;\\n    }\\n\\n    // =================================================================")

# For drop, we need to free the Ptr<StringMap> itself
text = text.replace("self.len = 0;\\n        }\\n    }\\n}", "self.len = 0;\\n        }\\n        free(smap as u64);\\n    }")
# Also need to remove the trailing `}` that belonged to `impl StringMap`
# The last line of the file should be the end of drop.

# At the end of mirror_ctrl and grow, add `return self;`
text = text.replace("call void @std__core__ptr__Ptr__write_i8(ptr %26, i8 %2)\\n  br label %41", "") # wait, not LLVM IR!

text = text.replace("        self.ctrl.offset(idx).write(tag);\\n        if idx < Group::width() {\\n            self.ctrl.offset(self.cap + idx).write(tag);\\n        }\\n    }", "        self.ctrl.offset(idx).write(tag);\\n        if idx < Group::width() {\\n            self.ctrl.offset(self.cap + idx).write(tag);\\n        }\\n        return self;\\n    }")

text = text.replace("self.arena_used = new_arena_used;\\n    }", "self.arena_used = new_arena_used;\\n        return self;\\n    }")

# Remove the trailing '}' of the impl block
text = re.sub(r'\}\s*$', '', text)

with open("std/collections/string_map.salt", "w") as f:
    f.write(text)

