import sys

with open("user/os/ipc_ring.salt", "r") as f:
    content = f.read()

new_content = content.replace("extern fn sys_mfence();", "extern fn sys_mfence();\nextern fn memcpy(dst: u64, src: u64, len: u32);")

bulk_funcs = """
pub fn ipc_push_bytes(src_ptr: u64, len: u32) -> u32 {
    let available = RING_SIZE - (WRITE_HEAD - READ_HEAD); // WRONG LOGIC
    // Proper modulo logic:
    let used = (WRITE_HEAD + RING_SIZE - READ_HEAD) % RING_SIZE;
    let available_space = RING_SIZE - used - 1; // Leave 1 byte to distinguish full/empty
    
    if len > available_space {
        return 0; // Not enough space
    }
    
    let first_chunk = RING_SIZE - WRITE_HEAD;
    if len <= first_chunk {
        memcpy(IPC_BUFFER_PTR + (WRITE_HEAD as u64), src_ptr, len);
    } else {
        memcpy(IPC_BUFFER_PTR + (WRITE_HEAD as u64), src_ptr, first_chunk);
        memcpy(IPC_BUFFER_PTR, src_ptr + (first_chunk as u64), len - first_chunk);
    }
    
    sys_mfence();
    WRITE_HEAD = (WRITE_HEAD + len) % RING_SIZE;
    return 1;
}

pub fn ipc_read_bytes(dst_ptr: u64, len: u32) -> u32 {
    let used = (WRITE_HEAD + RING_SIZE - READ_HEAD) % RING_SIZE;
    if len > used {
        return 0; // Not enough data
    }
    
    sys_mfence();
    
    let first_chunk = RING_SIZE - READ_HEAD;
    if len <= first_chunk {
        memcpy(dst_ptr, IPC_BUFFER_PTR + (READ_HEAD as u64), len);
    } else {
        memcpy(dst_ptr, IPC_BUFFER_PTR + (READ_HEAD as u64), first_chunk);
        memcpy(dst_ptr + (first_chunk as u64), IPC_BUFFER_PTR, len - first_chunk);
    }
    
    READ_HEAD = (READ_HEAD + len) % RING_SIZE;
    return 1;
}
"""

new_content += bulk_funcs

with open("user/os/ipc_ring.salt", "w") as f:
    f.write(new_content)
