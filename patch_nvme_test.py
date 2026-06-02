import sys

with open("kernel/tests/nvme_test.salt", "r") as f:
    content = f.read()

# Replace the end of the Identity block and add IO Queue creation + Block R/W tests
old_end = """        serial.print("NVMe Model Number: ");
        let mut i = 0;
        let buf_ptr = id_buf_phys as Ptr<u8>;
        while i < 40 {
            let c = buf_ptr.offset(24 + i).read();
            if c != 0 {
                // simple hack to print char using outb since serial doesn't have print_char yet
                unsafe { outb(0x3F8, c); }
            }
            i = i + 1;
        }
        serial.print("\\n");"""

new_end = """        serial.print("NVMe Model Number: ");
        let mut i = 0;
        let buf_ptr = id_buf_phys as Ptr<u8>;
        while i < 40 {
            let c = buf_ptr.offset(24 + i).read();
            if c != 0 {
                unsafe { outb(0x3F8, c); }
            }
            i = i + 1;
        }
        serial.print("\\n");
        
        // --- PHASE 2: IO QUEUES AND BLOCK R/W ---
        serial.print("Creating NVMe I/O Queues...\\n");
        let iosq_phys: u64 = 0x2004000;
        let iocq_phys: u64 = 0x2005000;
        nvme.zero_memory(iosq_phys as Ptr<u8>, 4096);
        nvme.zero_memory(iocq_phys as Ptr<u8>, 4096);
        
        nvme.init_io_queues(ctrl_ptr, iosq_phys, iocq_phys);
        
        let write_buf_phys: u64 = 0x2006000;
        let read_buf_phys: u64 = 0x2007000;
        
        // Fill write_buf with a pattern
        let write_ptr = write_buf_phys as Ptr<u32>;
        write_ptr.offset(0).write(0xDEADBEEF);
        write_ptr.offset(1).write(0xCAFEBABE);
        write_ptr.offset(2).write(0x8BADF00D);
        write_ptr.offset(3).write(0x0DEFACED);
        
        serial.print("Writing Block to LBA 0...\\n");
        nvme.write_block(ctrl_ptr, 0, write_buf_phys, 1);
        let write_status = nvme.poll_io_cq(ctrl_ptr);
        serial.print("Write Command Complete. Status: ");
        serial.print_hex(write_status as u64);
        serial.print("\\n");
        
        serial.print("Reading Block from LBA 0...\\n");
        nvme.read_block(ctrl_ptr, 0, read_buf_phys, 1);
        let read_status = nvme.poll_io_cq(ctrl_ptr);
        serial.print("Read Command Complete. Status: ");
        serial.print_hex(read_status as u64);
        serial.print("\\n");
        
        // Verify
        let read_ptr = read_buf_phys as Ptr<u32>;
        if read_ptr.offset(0).read() == 0xDEADBEEF {
            if read_ptr.offset(1).read() == 0xCAFEBABE {
                serial.print("NVMe Block R/W Verified!\\n");
            } else {
                serial.print("Error: Block read mismatch at offset 1.\\n");
            }
        } else {
            serial.print("Error: Block read mismatch at offset 0.\\n");
        }
"""
content = content.replace(old_end, new_end)

with open("kernel/tests/nvme_test.salt", "w") as f:
    f.write(content)
