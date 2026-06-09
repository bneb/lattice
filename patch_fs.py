import sys

with open('std/fs/fs.salt', 'r') as f:
    content = f.read()

content = content.replace(
    "const OP_EXISTS: u64 = 7;",
    "const OP_EXISTS: u64 = 7;\nconst OP_MMAP: u64 = 8;"
)

push_data_code = """
    fn push_data_command(&mut self, op: u64, fd: u64, buf_ptr: u64, size: u64) -> u64 {
        self.seq = self.seq + 1;
        let id = self.seq;
        
        let head = *((self.cmd_ring + 0) as &u64);
        let cap = *((self.cmd_ring + 16) as &u64);
        let data = self.cmd_ring + 128;
        
        let start_idx = head % cap;
        
        *((data + start_idx + 0) as &mut u64) = op;
        *((data + start_idx + 8) as &mut u64) = id;
        *((data + start_idx + 16) as &mut u64) = fd;
        *((data + start_idx + 24) as &mut u64) = size;
        *((data + start_idx + 32) as &mut u64) = buf_ptr;
        
        *((self.cmd_ring + 0) as &mut u64) = head + 256;
        return id;
    }
"""

content = content.replace(
    "fn wait_completion(&mut self, target_id: u64) -> VfsCompletion {",
    push_data_code + "\n    fn wait_completion(&mut self, target_id: u64) -> VfsCompletion {"
)

new_apis = """
    pub fn open(&mut self, path: &u8) -> Result<FileHandle> {
        let id = self.push_command(OP_OPEN, 0, 0, path);
        let comp = self.wait_completion(id);
        if comp.status < 0 {
            return Result::Err(map_status(comp.status));
        }
        return Result::Ok(FileHandle { fd: comp.fd });
    }

    pub fn read(&mut self, file: &FileHandle, buf: Ptr<u8>, size: u64) -> Result<u64> {
        let id = self.push_data_command(OP_READ, file.fd, buf as u64, size);
        let comp = self.wait_completion(id);
        if comp.status < 0 {
            return Result::Err(map_status(comp.status));
        }
        return Result::Ok(comp.size);
    }

    pub fn write(&mut self, file: &FileHandle, buf: Ptr<u8>, size: u64) -> Result<u64> {
        let id = self.push_data_command(OP_WRITE, file.fd, buf as u64, size);
        let comp = self.wait_completion(id);
        if comp.status < 0 {
            return Result::Err(map_status(comp.status));
        }
        return Result::Ok(comp.size);
    }

    pub fn close(&mut self, file: &FileHandle) -> Result<i32> {
        let id = self.push_data_command(OP_CLOSE, file.fd, 0, 0);
        let comp = self.wait_completion(id);
        if comp.status < 0 {
            return Result::Err(map_status(comp.status));
        }
        return Result::Ok(0);
    }

    pub fn mmap(&mut self, file: &FileHandle, size: u64, offset: u64) -> Result<u64> {
        // We will pass the 'offset' via the 'buf_ptr' parameter position
        let id = self.push_data_command(OP_MMAP, file.fd, offset, size);
        let comp = self.wait_completion(id);
        if comp.status < 0 {
            return Result::Err(map_status(comp.status));
        }
        // For MMAP, comp.fd contains the pointer to the mapped memory
        return Result::Ok(comp.fd);
    }
"""

content = content.replace(
    "pub fn remove_file(&mut self, path: &u8) -> Result<i32> {",
    new_apis + "\n    pub fn remove_file(&mut self, path: &u8) -> Result<i32> {"
)

file_handle_struct = """
pub struct FileHandle {
    pub fd: u64
}

// VfsCompletion { id, status, fd, size } (32 bytes = 4 u64s)
"""

content = content.replace(
    "// VfsCompletion { id, status, fd, size } (32 bytes = 4 u64s)",
    file_handle_struct
)

with open('std/fs/fs.salt', 'w') as f:
    f.write(content)
