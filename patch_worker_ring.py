import sys

with open("user/os/worker_ring.salt", "r") as f:
    content = f.read()

new_content = content.replace("global UP_READ: u32 = 0;", "global UP_READ: u32 = 0;\nextern fn sys_mfence();\nextern fn memcpy(dst: u64, src: u64, len: u32);")

bulk_funcs = """
pub fn push_down_bytes(src_ptr: u64, len: u32) -> u32 {
    let used = (DOWN_WRITE + RING_SIZE - DOWN_READ) % RING_SIZE;
    let available_space = RING_SIZE - used - 1;
    if len > available_space { return 0; }
    
    let first_chunk = RING_SIZE - DOWN_WRITE;
    if len <= first_chunk {
        memcpy(WORKER_DOWN_BUFFER + (DOWN_WRITE as u64), src_ptr, len);
    } else {
        memcpy(WORKER_DOWN_BUFFER + (DOWN_WRITE as u64), src_ptr, first_chunk);
        memcpy(WORKER_DOWN_BUFFER, src_ptr + (first_chunk as u64), len - first_chunk);
    }
    
    sys_mfence();
    DOWN_WRITE = (DOWN_WRITE + len) % RING_SIZE;
    return 1;
}

pub fn read_down_bytes(dst_ptr: u64, len: u32) -> u32 {
    let used = (DOWN_WRITE + RING_SIZE - DOWN_READ) % RING_SIZE;
    if len > used { return 0; }
    
    sys_mfence();
    let first_chunk = RING_SIZE - DOWN_READ;
    if len <= first_chunk {
        memcpy(dst_ptr, WORKER_DOWN_BUFFER + (DOWN_READ as u64), len);
    } else {
        memcpy(dst_ptr, WORKER_DOWN_BUFFER + (DOWN_READ as u64), first_chunk);
        memcpy(dst_ptr + (first_chunk as u64), WORKER_DOWN_BUFFER, len - first_chunk);
    }
    
    DOWN_READ = (DOWN_READ + len) % RING_SIZE;
    return 1;
}

pub fn push_up_bytes(src_ptr: u64, len: u32) -> u32 {
    let used = (UP_WRITE + RING_SIZE - UP_READ) % RING_SIZE;
    let available_space = RING_SIZE - used - 1;
    if len > available_space { return 0; }
    
    let first_chunk = RING_SIZE - UP_WRITE;
    if len <= first_chunk {
        memcpy(WORKER_UP_BUFFER + (UP_WRITE as u64), src_ptr, len);
    } else {
        memcpy(WORKER_UP_BUFFER + (UP_WRITE as u64), src_ptr, first_chunk);
        memcpy(WORKER_UP_BUFFER, src_ptr + (first_chunk as u64), len - first_chunk);
    }
    
    sys_mfence();
    UP_WRITE = (UP_WRITE + len) % RING_SIZE;
    return 1;
}

pub fn read_up_bytes(dst_ptr: u64, len: u32) -> u32 {
    let used = (UP_WRITE + RING_SIZE - UP_READ) % RING_SIZE;
    if len > used { return 0; }
    
    sys_mfence();
    let first_chunk = RING_SIZE - UP_READ;
    if len <= first_chunk {
        memcpy(dst_ptr, WORKER_UP_BUFFER + (UP_READ as u64), len);
    } else {
        memcpy(dst_ptr, WORKER_UP_BUFFER + (UP_READ as u64), first_chunk);
        memcpy(dst_ptr + (first_chunk as u64), WORKER_UP_BUFFER, len - first_chunk);
    }
    
    UP_READ = (UP_READ + len) % RING_SIZE;
    return 1;
}
"""

new_content += bulk_funcs

with open("user/os/worker_ring.salt", "w") as f:
    f.write(new_content)
