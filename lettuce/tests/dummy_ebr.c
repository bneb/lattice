#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void ebr_retire(unsigned long long core, unsigned long long ptr) {}
void ebr_enter_epoch(unsigned long long core) {}
void ebr_exit_epoch(unsigned long long core) {}
void ebr_advance_epoch(void) {}
void ebr_reclaim(unsigned long long core) {}

uint64_t g_vfs_mem = 0;

uint8_t fake_file_buf[1024 * 1024];
uint64_t fake_file_pos = 0;

void sys_ipc_send(uint64_t pid, uint32_t cmd, uint64_t arg1, uint32_t arg2) {}
uint64_t sys_ipc_recv() { return 6; }
uint64_t sys_ipc_get_msg0() { return 0; }
uint64_t my_pid() { return 1; }
void assert_eq(uint64_t a, uint64_t b) { 
    if (a != b) __builtin_trap(); 
}

void sys_shm_grant(uint64_t dst_pid, uint64_t src_vaddr, uint64_t size, uint64_t flags, uint64_t arg) {
    g_vfs_mem = src_vaddr;
}

void r3_sys_yield() {
    if (g_vfs_mem == 0) return;
    
    while (1) {
        uint64_t cmd_head = *(uint64_t*)(g_vfs_mem + 0);
        uint64_t cmd_tail = *(uint64_t*)(g_vfs_mem + 8);
        
        if (cmd_head != cmd_tail) {
            uint64_t slot = (cmd_tail % 3968) + 128;
            uint32_t op = *(uint32_t*)(g_vfs_mem + slot + 0);
            uint64_t cmd_id = *(uint64_t*)(g_vfs_mem + slot + 8);
            uint64_t arg_size = *(uint64_t*)(g_vfs_mem + slot + 16);
            uint64_t arg_ptr = *(uint64_t*)(g_vfs_mem + slot + 24);
            
            *(uint64_t*)(g_vfs_mem + 8) = cmd_tail + 256; // consume
            
            uint64_t comp_base = g_vfs_mem + 4096;
            uint64_t comp_head = *(uint64_t*)(comp_base + 0);
            
            uint64_t comp_slot = (comp_head % 3968) + 128;
            *(uint64_t*)(comp_base + comp_slot + 0) = cmd_id;
            *(int64_t*)(comp_base + comp_slot + 8) = 0; // status OK
            
            if (op == 1) { // OP_OPEN
                *(uint64_t*)(comp_base + comp_slot + 16) = 42;
                fake_file_pos = 0;
            } else if (op == 2) { // OP_READ
                uint64_t read_len = arg_size;
                if (read_len > fake_file_pos) read_len = fake_file_pos;
                memcpy((void*)arg_ptr, fake_file_buf, read_len);
                *(uint64_t*)(comp_base + comp_slot + 24) = read_len;
            } else if (op == 3) { // OP_WRITE
                memcpy(fake_file_buf + fake_file_pos, (void*)arg_ptr, arg_size);
                fake_file_pos += arg_size;
                *(uint64_t*)(comp_base + comp_slot + 24) = arg_size;
            } else if (op == 4) { // OP_CLOSE
                *(uint64_t*)(comp_base + comp_slot + 24) = 0;
            } else if (op == 5) { // OP_MMAP
                *(uint64_t*)(comp_base + comp_slot + 24) = arg_size;
            }
            
            *(uint64_t*)(comp_base + 0) = comp_head + 32; // publish
        } else {
            break;
        }
    }
}

// vfs_connect — allocates SPSC rings, initializes headers,
// grants SHM to StoreD, sends INIT, waits for RESP_OK.
// Returns pointer to VfsConnection { cmd_ring: u64, comp_ring: u64, seq: u64 }.
void* std__fs__fs__vfs_connect(void) {
    uint64_t* mem = (uint64_t*)malloc(8192);
    if (!mem) return 0;

    // Command Ring (Page 0): HEAD=0, TAIL=0, CAPACITY=3840
    mem[0] = 0;      // HEAD
    mem[1] = 0;      // TAIL
    mem[2] = 3840;   // CAPACITY

    // Completion Ring (Page 1 at offset 4096 = 512 * 8)
    uint64_t* comp = mem + 512;
    comp[0] = 0;      // HEAD
    comp[1] = 0;      // TAIL
    comp[2] = 4096 - 128; // CAPACITY

    // Grant SHM to StoreD
    uint64_t mem_addr = (uint64_t)mem;
    sys_shm_grant(6 /* STORED_PID */, mem_addr, 0, 3 /* READ|WRITE */, 0);

    // Send INIT command
    sys_ipc_send(6, 0x20 /* CMD_VFS_INIT */, 0, 0);

    // Wait for RESP_OK — these stubs return STORED_PID(6) and RESP_OK(0)
    while (1) {
        uint64_t sender = sys_ipc_recv();
        if (sender == 6) {
            uint64_t resp = sys_ipc_get_msg0();
            if (resp == 0) break;
        }
    }

    // Build VfsConnection (3 u64 fields) on heap and return
    uint64_t* conn = (uint64_t*)malloc(24);  // 3 * 8 bytes
    conn[0] = mem_addr;          // cmd_ring
    conn[1] = mem_addr + 4096;   // comp_ring
    conn[2] = 0;                 // seq
    return conn;
}
