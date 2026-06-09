import sys

with open('std/fs/fs.salt', 'r') as f:
    content = f.read()

new_code = """
pub struct FileHandle {
    pub fd: u64
}

impl VfsConnection {
    pub fn open(&mut self, path: &u8) -> Result<FileHandle> {
        let id = self.push_command(OP_OPEN, 0, 0, path);
        let comp = self.wait_completion(id);
        if comp.status < 0 {
            return Result::Err(map_status(comp.status));
        }
        return Result::Ok(FileHandle { fd: comp.fd });
    }

    pub fn read(&mut self, handle: &FileHandle, buffer: &mut u8, size: u64) -> Result<u64> {
        // We push OP_READ and pass the buffer as the path pointer for simplicity
        // In a real VFS we'd use a data ring or similar, but for KeuOS IPC testing this works.
        let id = self.push_command(OP_READ, handle.fd, size, buffer as &u8);
        let comp = self.wait_completion(id);
        if comp.status < 0 {
            return Result::Err(map_status(comp.status));
        }
        return Result::Ok(comp.size);
    }

    pub fn write(&mut self, handle: &FileHandle, buffer: &u8, size: u64) -> Result<u64> {
        let id = self.push_command(OP_WRITE, handle.fd, size, buffer);
        let comp = self.wait_completion(id);
        if comp.status < 0 {
            return Result::Err(map_status(comp.status));
        }
        return Result::Ok(comp.size);
    }

    pub fn close(&mut self, handle: &FileHandle) -> Result<i32> {
        // Provide an empty path for close
        let empty_path = 0;
        let id = self.push_command(OP_CLOSE, handle.fd, 0, &empty_path as &u8);
        let comp = self.wait_completion(id);
        if comp.status < 0 {
            return Result::Err(map_status(comp.status));
        }
        return Result::Ok(0);
    }
}
"""

content = content.replace("    pub fn remove_file(&mut self, path: &u8) -> Result<i32> {\n        let id = self.push_command(OP_UNLINK, 0, 0, path);\n        let comp = self.wait_completion(id);\n        if comp.status < 0 {\n            return Result::Err(map_status(comp.status));\n        }\n        return Result::Ok(0);\n    }\n}", "    pub fn remove_file(&mut self, path: &u8) -> Result<i32> {\n        let id = self.push_command(OP_UNLINK, 0, 0, path);\n        let comp = self.wait_completion(id);\n        if comp.status < 0 {\n            return Result::Err(map_status(comp.status));\n        }\n        return Result::Ok(0);\n    }\n" + new_code + "\n}")

with open('std/fs/fs.salt', 'w') as f:
    f.write(content)
