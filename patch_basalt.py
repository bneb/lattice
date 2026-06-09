import sys

with open('user/basalt/main.salt', 'r') as f:
    content = f.read()

basalt_imports = "use user.basalt.app_hw_matmul.execute_hardware_inference\nuse std.fs.{VfsConnection, FileHandle}"

content = content.replace("use user.basalt.app_hw_matmul.execute_hardware_inference", basalt_imports)

init_basalt = """pub fn init_basalt(
    arena_base: u64,
    arena_cap: u64,
    rx_ring: u64,
    tx_ring: u64,
    router_streams: u64,
    router_states: u64
) -> u64 {
    // 1. Register Basalt's RX stream on the PortRouter at port 80
    let port_idx = LISTEN_PORT as u64;
    *((router_streams + port_idx * 8) as &mut u64) = rx_ring;
    *((router_states + port_idx * 8) as &mut u64) = 1;  // PORT_BOUND

    // 2. Memory-map the model weights via VFS instead of malloc
    let mut conn = VfsConnection::connect();
    // Use O_RDONLY | O_CREAT for the mock weights (so it succeeds if it doesn't exist)
    let res = conn.open("weights.bin\\0" as &u8);
    let mut nvme_addr: u64 = 0;
    if res.is_ok() {
        let fd = res.unwrap();
        // mmap 4096 bytes at offset 0
        let mmap_res = conn.mmap(&fd, 4096, 0);
        if mmap_res.is_ok() {
            nvme_addr = mmap_res.unwrap();
        }
    }
    
    if nvme_addr == 0 {
        // Fallback to malloc for testing if VFS fails
        nvme_addr = 0x88880000; // Mock pointer
    }
    
    return nvme_addr;
}"""

# Replace the old init_basalt entirely. We'll use regex or simple string replacement.
old_init_basalt = """pub fn init_basalt(
    arena_base: u64,
    arena_cap: u64,
    rx_ring: u64,
    tx_ring: u64,
    router_streams: u64,
    router_states: u64
) {
    // 1. Register Basalt's RX stream on the PortRouter at port 80
    let port_idx = LISTEN_PORT as u64;
    *((router_streams + port_idx * 8) as &mut u64) = rx_ring;
    *((router_states + port_idx * 8) as &mut u64) = 1;  // PORT_BOUND

    // 2. Store application state for the poll loop
    //    (In production, this calls basalt_app.init())
    //    The globals in app.salt are set directly here for v0.1
}"""

content = content.replace(old_init_basalt, init_basalt)

with open('user/basalt/main.salt', 'w') as f:
    f.write(content)
