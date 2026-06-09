import sys

with open('lettuce/src/server.salt', 'r') as f:
    content = f.read()

malloc_line = "let nvme_addr = malloc(4096) as u64;"
mmap_code = """    // Instead of malloc, we will zero-copy mmap the weights via VFS
    let mut nvme_addr: u64 = 0;
    if VFS_CONN_PTR != 0 {
        let conn_ptr = VFS_CONN_PTR as Ptr<std::fs::VfsConnection>;
        let mmap_res = conn_ptr.read().open("weights.bin\\0" as &u8);
        if mmap_res.is_ok() {
            let fd = mmap_res.unwrap();
            let ptr_res = conn_ptr.read().mmap(&fd, 4096, 0);
            if ptr_res.is_ok() {
                nvme_addr = ptr_res.unwrap();
            }
        }
    }
    if nvme_addr == 0 {
        nvme_addr = malloc(4096) as u64;
    }"""

content = content.replace(malloc_line, mmap_code)
content = content.replace("use lettuce.aof::{Aof_init, Aof_replay}", "use lettuce.aof::{Aof_init, Aof_replay, VFS_CONN_PTR}")

with open('lettuce/src/server.salt', 'w') as f:
    f.write(content)
