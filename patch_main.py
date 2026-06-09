import sys

with open('user/stored/main.salt', 'r') as f:
    content = f.read()

dispatch = """
    let buf_ptr = vol_read64(data + start_idx + 32); // For data commands, first 8 bytes of path buffer is the pointer
    
    if op == vfs_protocol.OP_EXISTS {
        let exists = host_bridge::exists(path_ptr as &u8);
        status = if exists { 0 } else { 2 }; // 2 = ENOENT map
    } else if op == vfs_protocol.OP_MKDIR {
        let res = host_bridge::create_dir(path_ptr as &u8);
        if res.is_err() {
            status = -1; // General error for now
        }
    } else if op == vfs_protocol.OP_UNLINK {
        let res = host_bridge::remove_file(path_ptr as &u8);
        if res.is_err() {
            status = -1;
        }
    } else if op == vfs_protocol.OP_OPEN {
        // open(path, flags)
        // We'll pass standard read/write create flags (O_CREAT | O_RDWR = 0x0202 on macOS, typically just use 0x0202 for tests)
        // Let's just use O_RDWR | O_CREAT which is usually 0x0202 on macOS or 0x42 on Linux. 
        // A generic 0x0202 works for POSIX macOS for this harness.
        let flags = 0x0202; // O_RDWR | O_CREAT
        let res = host_bridge::open_file(path_ptr as &u8, flags);
        if res.is_err() {
            status = -1;
        } else {
            out_fd = res.unwrap() as u64;
        }
    } else if op == vfs_protocol.OP_READ {
        let res = host_bridge::read_file(fd as i64, buf_ptr as Ptr<u8>, size);
        if res.is_err() {
            status = -1;
        } else {
            out_size = res.unwrap();
        }
    } else if op == vfs_protocol.OP_WRITE {
        let res = host_bridge::write_file(fd as i64, buf_ptr as Ptr<u8>, size);
        if res.is_err() {
            status = -1;
        } else {
            out_size = res.unwrap();
        }
    } else if op == vfs_protocol.OP_CLOSE {
        let res = host_bridge::close_file(fd as i64);
        if res.is_err() {
            status = -1;
        }
    } else if op == vfs_protocol.OP_MMAP {
        // mmap(fd, size, offset). Note offset is in buf_ptr slot
        let offset = buf_ptr;
        let res = host_bridge::mmap_file(fd as i64, size, offset);
        if res.is_err() {
            status = -1;
        } else {
            out_fd = res.unwrap(); // Pass pointer back in fd slot
        }
    }
"""

old_dispatch = """    if op == vfs_protocol.OP_EXISTS {
        let exists = host_bridge::exists(path_ptr as &u8);
        status = if exists { 0 } else { 2 }; // 2 = ENOENT map
    } else if op == vfs_protocol.OP_MKDIR {
        let res = host_bridge::create_dir(path_ptr as &u8);
        if res.is_err() {
            status = -1; // General error for now
        }
    } else if op == vfs_protocol.OP_UNLINK {
        let res = host_bridge::remove_file(path_ptr as &u8);
        if res.is_err() {
            status = -1;
        }
    }"""

content = content.replace(old_dispatch, dispatch.strip())

with open('user/stored/main.salt', 'w') as f:
    f.write(content)
