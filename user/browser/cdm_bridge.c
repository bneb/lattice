#include <stdint.h>
#include <stdio.h>

void user__browser__ipc_shared__ext_ipc_send_r2m_command_with_payload(uint32_t cmd_type, uint64_t arg1, uint64_t p_ptr, uint32_t p_len) {
    // CDM handles its own IPC rings manually.
}

void ext_ipc_send_r2m_command_with_payload(uint32_t cmd_type, uint64_t arg1, uint64_t p_ptr, uint32_t p_len) {
    // No-op for CDM process
}

void sys_ipc_send_r2m_command_with_payload(uint32_t cmd_type, uint64_t arg1, uint64_t p_ptr, uint32_t p_len) {
    // No-op for CDM process
}

// Minimal stub for sandbox_init if needed
int sys_sandbox_init(const char *profile, uint64_t flags, char **errorbuf) {
    return 0;
}

uint64_t ext_ipc_get_bulk_ingress_ptr() { return 0; }
void ext_ipc_init_shared_memory() {}
void sys_memcpy(uint64_t dst, uint64_t src, uint32_t len) {
    uint8_t *d = (uint8_t *)dst;
    uint8_t *s = (uint8_t *)src;
    for (uint32_t i = 0; i < len; i++) {
        d[i] = s[i];
    }
}
