import re

with open("std/collections/string_map.salt", "r") as f:
    content = f.read()

# 1. Rename StringMap to StringMapInner
content = content.replace("pub struct StringMap {", "pub struct StringMapInner {")

# 2. Add StringMap wrapper
wrapper = """
pub struct StringMap {
    ptr: Ptr<StringMapInner>,
}
"""
content = content.replace("pub struct StringMapInner {", wrapper + "\npub struct StringMapInner {")

# 3. Change all impl StringMap to standalone functions on StringMapInner
# Wait, I can just use impl StringMap but change methods!
