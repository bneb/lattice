import re
import sys

def modify_string():
    with open('string_copy.salt', 'r') as f:
        content = f.read()

    # 1. struct String
    content = re.sub(
        r'pub struct String \{\n    data: Ptr<u8>,\n    len: i64,\n    cap: i64,\n\}',
        'pub struct String {\n    data: Ptr<u8>,\n    len: i64,\n    cap: i64,\n    is_inline: bool,\n    inline_buf: [u8; 23],\n}',
        content
    )

    # 2. String::new
    content = re.sub(
        r'pub fn new\(\) -> String \{\n        return String \{ data: 1 as Ptr<u8>, len: 0, cap: 0 \};\n    \}',
        'pub fn new() -> String {\n        return String { data: 1 as Ptr<u8>, len: 0, cap: 23, is_inline: true, inline_buf: [0; 23] };\n    }',
        content
    )

    # 3. String::with_capacity
    content = re.sub(
        r'pub fn with_capacity\(cap: i64\) -> String\n        // requires cap >= 0\n    \{\n        if cap == 0 \{\n            return String::new\(\);\n        \}\n        let ptr = malloc\(cap\);\n        let p = ptr as Ptr<u8>;\n        return String \{ data: p, len: 0, cap: cap \};\n    \}',
        'pub fn with_capacity(cap: i64) -> String\n        // requires cap >= 0\n    {\n        if cap <= 23 {\n            return String::new();\n        }\n        let ptr = malloc(cap);\n        let p = ptr as Ptr<u8>;\n        return String { data: p, len: 0, cap: cap, is_inline: false, inline_buf: [0; 23] };\n    }',
        content
    )

    # 4. String::with_arena_capacity
    content = re.sub(
        r'pub fn with_arena_capacity\(cap: i64\) -> String \{\n        if cap == 0 \{\n            return String::new\(\);\n        \}\n        let addr = malloc\(cap\);\n        let p = addr as Ptr<u8>;\n        return String \{ data: p, len: 0, cap: cap \};\n    \}',
        'pub fn with_arena_capacity(cap: i64) -> String {\n        if cap <= 23 {\n            return String::new();\n        }\n        let addr = malloc(cap);\n        let p = addr as Ptr<u8>;\n        return String { data: p, len: 0, cap: cap, is_inline: false, inline_buf: [0; 23] };\n    }',
        content
    )

    # 5. mut_ptr and as_ptr
    ptr_funcs = '''
    // Get mutable raw pointer to underlying buffer
    @inline
    pub fn mut_ptr(&mut self) -> Ptr<u8> {
        if self.is_inline {
            return reinterpret_cast::<Ptr<u8>>(&self.inline_buf[0]);
        }
        return self.data;
    }

    // Get raw pointer to underlying buffer
    @inline
    pub fn as_ptr(&self) -> Ptr<u8> {
        if self.is_inline {
            return reinterpret_cast::<Ptr<u8>>(&self.inline_buf[0]);
        }
        return self.data;
    }
'''
    content = re.sub(
        r'    // Get raw pointer to underlying buffer\n    @inline\n    pub fn as_ptr\(&self\) -> Ptr<u8> \{\n        return self\.data;\n    \}',
        ptr_funcs,
        content
    )

    # 6. self.data.offset -> self.mut_ptr().offset (Except in Eq)
    # Be careful not to replace in Eq or other read-only contexts if it's &self
    # Let's replace specifically in `push_byte`
    content = content.replace(
        'self.data.offset(self.len).write(b);',
        'self.mut_ptr().offset(self.len).write(b);'
    )

    # In `push`
    content = content.replace(
        'self.data.offset(self.len).write((0xC0 | (ch >> 6)) as u8);',
        'self.mut_ptr().offset(self.len).write((0xC0 | (ch >> 6)) as u8);'
    )
    content = content.replace(
        'self.data.offset(self.len + 1).write((0x80 | (ch & 0x3F)) as u8);',
        'self.mut_ptr().offset(self.len + 1).write((0x80 | (ch & 0x3F)) as u8);'
    )
    content = content.replace(
        'self.data.offset(self.len).write((0xE0 | (ch >> 12)) as u8);',
        'self.mut_ptr().offset(self.len).write((0xE0 | (ch >> 12)) as u8);'
    )
    content = content.replace(
        'self.data.offset(self.len + 2).write((0x80 | (ch & 0x3F)) as u8);',
        'self.mut_ptr().offset(self.len + 2).write((0x80 | (ch & 0x3F)) as u8);'
    )
    content = content.replace(
        'self.data.offset(self.len).write((0xF0 | (ch >> 18)) as u8);',
        'self.mut_ptr().offset(self.len).write((0xF0 | (ch >> 18)) as u8);'
    )
    content = content.replace(
        'self.data.offset(self.len + 3).write((0x80 | (ch & 0x3F)) as u8);',
        'self.mut_ptr().offset(self.len + 3).write((0x80 | (ch & 0x3F)) as u8);'
    )

    # byte_at
    content = content.replace(
        'return self.data.index(idx);',
        'return self.as_ptr().index(idx);'
    )

    # as_view
    content = content.replace(
        'return StringView { ptr: self.data, len: self.len };',
        'return StringView { ptr: self.as_ptr(), len: self.len };'
    )

    # free
    content = re.sub(
        r'    pub fn free\(self\) \{\n        if self\.cap > 0 \{\n            salt_sys_free\(self\.data as u64\);\n        \}\n    \}',
        '    pub fn free(self) {\n        if !self.is_inline && self.cap > 0 {\n            salt_sys_free(self.data as u64);\n        }\n    }',
        content
    )

    # grow_slow
    grow_slow_new = '''    @noinline
    fn grow_slow(&mut self, additional: i64) {
        let required = self.len + additional;

        // Deterministic growth: double or meet requirement
        let new_cap = if self.cap * 2 > required {
            self.cap * 2
        } else {
            required
        };

        if self.is_inline {
            let ptr = malloc(new_cap);
            let dst_ptr = ptr as Ptr<u8>;
            for i in 0..self.len {
                dst_ptr.offset(i).write(self.inline_buf[i as i32]);
            }
            self.data = dst_ptr;
            self.is_inline = false;
        } else {
            if self.cap == 0 {
                let ptr = malloc(new_cap);
                self.data = ptr as Ptr<u8>;
            } else {
                let new_ptr = salt_sys_realloc(self.data as u64, new_cap);
                self.data = new_ptr as Ptr<u8>;
            }
        }
        self.cap = new_cap;
    }'''
    content = re.sub(
        r'    @noinline\n    fn grow_slow\(&mut self, additional: i64\) \{.*?self\.cap = new_cap;\n    \}',
        grow_slow_new,
        content,
        flags=re.DOTALL
    )

    # write_str_unchecked
    content = content.replace(
        'let dst = self.data.offset(self.len);',
        'let dst = self.mut_ptr().offset(self.len);'
    )
    content = content.replace(
        'memcpy(dst as i64, src_addr, len);',
        'memcpy(reinterpret_cast::<i64>(dst), src_addr, len);'
    )

    # Eq
    content = content.replace(
        'let p1 = self.data;',
        'let p1 = self.as_ptr();'
    )
    content = content.replace(
        'let p2 = other.data;',
        'let p2 = other.as_ptr();'
    )

    # write_i32_unchecked / write_i64_unchecked
    content = content.replace(
        'self.data[self.len] = 48;',
        'self.mut_ptr()[self.len] = 48;'
    )
    content = content.replace(
        'let write_ptr = self.data.offset(self.len);',
        'let write_ptr = self.mut_ptr().offset(self.len);'
    )

    with open('string_modified.salt', 'w') as f:
        f.write(content)

if __name__ == "__main__":
    modify_string()
