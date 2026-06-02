import sys

with open("user/reactor/scheduler.salt", "r") as f:
    content = f.read()

new_content = content + """
// ============================================================================
// Reactor Event Loop & Task Spawning
// ============================================================================
// The reactor maintains a global registry of task slots.

pub global TASK_REGISTRY: u64 = 0;
pub global TASK_COUNT: u32 = 0;
pub const MAX_REACTOR_TASKS: u32 = 4096;

pub fn init_reactor() {
    // Allocate 4096 task slots
    // TaskSlot is 24 bytes (status, pending_token, result, state_addr, task_id - 5 u32s + u64 = 24 bytes? No, u64 is 8, others are 4. Total: 4+4+4+4 (padding)+8+4 = 28 bytes)
    // We'll just allocate a big chunk
    // Use mmap_shared from process
    TASK_REGISTRY = user.os.process.mmap_shared(131072); // 128KB
    TASK_COUNT = 0;
}

pub fn spawn_task(state_addr: u64, deque_addr: u64) -> u32 {
    let id = TASK_COUNT;
    if id >= MAX_REACTOR_TASKS {
        return 0xFFFFFFFF; // Error: Too many tasks
    }
    
    // Each TaskSlot is 24 bytes. 
    // struct layout: 
    // 0: status (u32)
    // 4: pending_token (u32)
    // 8: result (u32)
    // 12: task_id (u32)
    // 16: state_addr (u64)
    let slot_addr = TASK_REGISTRY + ((id as u64) * 24);
    
    // Initialize
    *((slot_addr) as &mut u32) = TASK_RUNNING;
    *((slot_addr + 4) as &mut u32) = 0;
    *((slot_addr + 8) as &mut u32) = 0;
    *((slot_addr + 12) as &mut u32) = id;
    *((slot_addr + 16) as &mut u64) = state_addr;
    
    TASK_COUNT = TASK_COUNT + 1;
    
    // Enqueue to the local deque
    push_bottom(deque_addr, id as i64);
    
    return id;
}

// Yield mechanism is inherently tied to the state machine polling.
// The reactor polls active tasks. If a task hits a blocking operation (like waiting for IPC),
// it calls yield_on_token(token_id). The reactor skips it until that token is ready.
"""

with open("user/reactor/scheduler.salt", "w") as f:
    f.write(new_content)
