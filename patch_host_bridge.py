import sys

with open('user/stored/host_bridge.salt', 'r') as f:
    content = f.read()

externs = """
extern fn salt_open(path: Ptr<u8>, flags: i32) -> i64;
extern fn salt_read(fd: i64, buf: Ptr<u8>, size: u64) -> i64;
extern fn salt_write(fd: i64, buf: Ptr<u8>, size: u64) -> i64;
extern fn salt_close(fd: i64) -> i32;
extern fn salt_mmap(fd: i64, size: u64, offset: u64) -> u64;
extern fn salt_munmap(ptr: u64, size: u64) -> i32;
"""

content = content.replace("extern fn salt_errno() -> i32;", "extern fn salt_errno() -> i32;\n" + externs)

public_wrappers = """
@trusted
pub fn open_file(path: &u8, flags: i32) -> Result<i64> {
    let fd = salt_open(path as Ptr<u8>, flags);
    if fd < 0 {
        let e = salt_errno();
        return Result::Err(errno_to_status(e));
    }
    return Result::Ok(fd);
}

@trusted
pub fn read_file(fd: i64, buf: Ptr<u8>, size: u64) -> Result<u64> {
    let res = salt_read(fd, buf, size);
    if res < 0 {
        let e = salt_errno();
        return Result::Err(errno_to_status(e));
    }
    return Result::Ok(res as u64);
}

@trusted
pub fn write_file(fd: i64, buf: Ptr<u8>, size: u64) -> Result<u64> {
    let res = salt_write(fd, buf, size);
    if res < 0 {
        let e = salt_errno();
        return Result::Err(errno_to_status(e));
    }
    return Result::Ok(res as u64);
}

@trusted
pub fn close_file(fd: i64) -> Result<i32> {
    let res = salt_close(fd);
    if res < 0 {
        let e = salt_errno();
        return Result::Err(errno_to_status(e));
    }
    return Result::Ok(0);
}

@trusted
pub fn mmap_file(fd: i64, size: u64, offset: u64) -> Result<u64> {
    let ptr = salt_mmap(fd, size, offset);
    if ptr == 0 {
        let e = salt_errno();
        return Result::Err(errno_to_status(e));
    }
    return Result::Ok(ptr);
}

@trusted
pub fn munmap_file(ptr: u64, size: u64) -> Result<i32> {
    let res = salt_munmap(ptr, size);
    if res < 0 {
        let e = salt_errno();
        return Result::Err(errno_to_status(e));
    }
    return Result::Ok(0);
}
"""

content = content.replace("pub fn exists(path: &u8) -> bool {", public_wrappers + "\npub fn exists(path: &u8) -> bool {")

with open('user/stored/host_bridge.salt', 'w') as f:
    f.write(content)
