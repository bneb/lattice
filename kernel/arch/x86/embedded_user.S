// =============================================================================
// kernel/arch/x86/embedded_user.S
// Embedded User ELF Binaries -- Three Distinct Ring 3 Programs
// =============================================================================
//
// Embeds three user-space ELF binaries into the kernel's .rodata section.
// Process A, B, and C have different payloads to demonstrate interleaving.
//
// In kmain():
//   &user_elf_a_start = Process A ELF bytes (test_memory)
//   &user_elf_b_start = Process B ELF bytes (ring3_test_b)
//   &user_elf_c_start = Process C ELF bytes (hello)
// =============================================================================

.section .rodata

// --- Process A: Memory Proof of Life Test ---
.global user_elf_a_start
.global user_elf_a_end

user_elf_a_start:
    .incbin "/tmp/test_memory"
user_elf_a_end:

// --- Process B ---
.global user_elf_b_start
.global user_elf_b_end

user_elf_b_start:
    .incbin "/tmp/ring3_test_b"
user_elf_b_end:

// --- Process C: Structured Lifecycle Verification ---
.global user_elf_c_start
.global user_elf_c_end

user_elf_c_start:
    .incbin "/tmp/hello"
user_elf_c_end:

// --- Process D: Grit Shell ---
.global user_elf_d_start
.global user_elf_d_end

user_elf_d_start:
    .incbin "/tmp/grit"
user_elf_d_end:

// --- NetD: Ring 3 Network Daemon (Zero-Trap SPSC Data Plane) ---
.global netd_elf_start
.global netd_elf_end

netd_elf_start:
    .incbin "/tmp/netd"
netd_elf_end:

// --- echo_server: TCP Echo Daemon ---
.global echo_server_elf_start
.global echo_server_elf_end

echo_server_elf_start:
    .incbin "/tmp/echo_server"
echo_server_elf_end:

// --- Mode B SIP: Ring 0 Software-Isolated Process ---
.global sip_elf_start
.global sip_elf_end

sip_elf_start:
    .incbin "/tmp/sip_app"
sip_elf_end:

// --- Process E: Echo ---
.global user_elf_e_start, user_elf_e_end
user_elf_e_start:
    .incbin "/tmp/echo"
user_elf_e_end:

// --- Process F: IPC Ping-Pong ---
.global user_elf_f_start, user_elf_f_end
user_elf_f_start:
    .incbin "/tmp/ipc_ping"
user_elf_f_end:

// --- Process G: Process List ---
.global user_elf_g_start, user_elf_g_end
user_elf_g_start:
    .incbin "/tmp/ps"
user_elf_g_end:

// --- Process H: Free ---
.global user_elf_h_start, user_elf_h_end
user_elf_h_start:
    .incbin "/tmp/free"
user_elf_h_end:

// --- Process I: Uptime ---
.global user_elf_i_start, user_elf_i_end
user_elf_i_start:
    .incbin "/tmp/uptime"
user_elf_i_end:

// --- Process J: Ping (ICMP Echo Client) ---
.global user_elf_j_start, user_elf_j_end
user_elf_j_start:
    .incbin "/tmp/ping"
user_elf_j_end:

// --- Process K: Fetch (HTTP GET Client via NetD Socket IPC) ---
.global user_elf_k_start, user_elf_k_end
user_elf_k_start:
    .incbin "/tmp/fetch"
user_elf_k_end:

// --- Process L: Echo Server (TCP listen/accept test) ---
.global user_elf_l_start, user_elf_l_end
user_elf_l_start:
    .incbin "/tmp/echo_server"
user_elf_l_end:


// --- Process M: Entity Write Test ---
.global user_elf_m_start, user_elf_m_end
user_elf_m_start:
    .incbin "/tmp/entity_write_test"
user_elf_m_end:

// --- Process N: Entity Map Test ---
.global user_elf_n_start, user_elf_n_end
user_elf_n_start:
    .incbin "/tmp/entity_map_test"
user_elf_n_end:

// --- Process O: Entity Alloc Test ---
.global user_elf_o_start, user_elf_o_end
user_elf_o_start:
    .incbin "/tmp/entity_alloc_test"
user_elf_o_end:

// --- Process P: ECS Package Demo ---
.global user_elf_p_start, user_elf_p_end
user_elf_p_start:
    .incbin "/tmp/ecs_pkg"
user_elf_p_end:
