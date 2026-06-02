import sys

with open("user/os/ipc_ring.salt", "r") as f:
    content = f.read()

old_func = """@no_mangle
pub fn ipc_ring_push_bytes(multiplex_id: u64, data: u64, len: u32) {
    // Write 8-byte multiplex ID
    let mut i: u32 = 0;
    while i < 8 {
        let shift = i * 8;
        let v = ((multiplex_id >> shift) & 0xFF) as u8;
        ipc_push_byte(v);
        i = i + 1;
    }

    // Write 4-byte length
    i = 0;
    while i < 4 {
        let shift = i * 8;
        let v = ((len >> shift) & 0xFF) as u8;
        ipc_push_byte(v);
        i = i + 1;
    }

    // Write data bytes
    i = 0;
    while i < len {
        let val = *((data + (i as u64)) as &u8);
        ipc_push_byte(val);
        i = i + 1;
    }
}"""

new_func = """@no_mangle
pub fn ipc_ring_push_bytes(multiplex_id: u64, data: u64, len: u32) {
    // 8-byte ID + 4-byte len = 12 bytes header
    let total_len = len + 12;
    
    // Write header bytes first (just use push_byte since it's small)
    let mut i: u32 = 0;
    while i < 8 {
        let shift = i * 8;
        let v = ((multiplex_id >> shift) & 0xFF) as u8;
        ipc_push_byte(v);
        i = i + 1;
    }

    i = 0;
    while i < 4 {
        let shift = i * 8;
        let v = ((len >> shift) & 0xFF) as u8;
        ipc_push_byte(v);
        i = i + 1;
    }

    // Bulk transfer the actual payload
    if len > 0 {
        ipc_push_bytes(data, len);
    }
}"""

content = content.replace(old_func, new_func)

with open("user/os/ipc_ring.salt", "w") as f:
    f.write(content)
