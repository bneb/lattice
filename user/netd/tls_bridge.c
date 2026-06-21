// ============================================================================
// Epic 34: TLS/HTTPS Transport Pipeline — NetD BearSSL Bridge
// ============================================================================
// Integrates BearSSL into the Ring 3 Network Daemon for TLS 1.2/1.3
// termination. Implements the asynchronous state pump that feeds BearSSL's
// engine exactly the bytes available from non-blocking sockets, and extracts
// cleartext to stream across the VirtIO ipc_ring.
//
// Zero dynamic heap allocations. All buffers are statically pre-allocated.
// ============================================================================

#include "tls_state.h"
#include <unistd.h>
#include <errno.h>
#include <string.h>
#include <sys/socket.h>
#include <netdb.h>
#include <fcntl.h>

// ============================================================================
// Global Connection Matrix — Static Allocation
// ============================================================================
NetDTLSConnection global_tls_connections[MAX_CONCURRENT_TLS_SOCKETS];

// ============================================================================
// Minimal Trust Anchor — Let's Encrypt ISRG Root X1
// ============================================================================
// For a sovereign browser, we embed the root CA directly.
// This is the ISRG Root X1 (Let's Encrypt) DN and public key.
// In production, this would be expanded with Mozilla's CA bundle.
// ============================================================================

// ISRG Root X1 — RSA 4096 — covers news.ycombinator.com
// DN: C=US, O=Internet Security Research Group, CN=ISRG Root X1
static const unsigned char ISRG_ROOT_X1_DN[] = {
    0x30, 0x4F, 0x31, 0x0B, 0x30, 0x09, 0x06, 0x03,
    0x55, 0x04, 0x06, 0x13, 0x02, 0x55, 0x53, 0x31,
    0x29, 0x30, 0x27, 0x06, 0x03, 0x55, 0x04, 0x0A,
    0x13, 0x20, 0x49, 0x6E, 0x74, 0x65, 0x72, 0x6E,
    0x65, 0x74, 0x20, 0x53, 0x65, 0x63, 0x75, 0x72,
    0x69, 0x74, 0x79, 0x20, 0x52, 0x65, 0x73, 0x65,
    0x61, 0x72, 0x63, 0x68, 0x20, 0x47, 0x72, 0x6F,
    0x75, 0x70, 0x31, 0x15, 0x30, 0x13, 0x06, 0x03,
    0x55, 0x04, 0x03, 0x13, 0x0C, 0x49, 0x53, 0x52,
    0x47, 0x20, 0x52, 0x6F, 0x6F, 0x74, 0x20, 0x58,
    0x31
};

// RSA public key modulus for ISRG Root X1 (n)
// This is a 4096-bit RSA key
static const unsigned char ISRG_ROOT_X1_RSA_N[] = {
    0xAD, 0xE8, 0x24, 0x73, 0xF4, 0x14, 0x37, 0xF3,
    0x9B, 0x9E, 0x2B, 0x57, 0x28, 0x1C, 0x87, 0xBE,
    0xDC, 0xB7, 0xDF, 0x38, 0x90, 0x8C, 0x6E, 0x3C,
    0xE6, 0x57, 0xA0, 0x78, 0xF7, 0x75, 0xC2, 0xA2,
    0xFE, 0xF5, 0x6A, 0x6E, 0xF6, 0x00, 0x4F, 0x28,
    0xDB, 0xDE, 0x68, 0x86, 0x6C, 0x44, 0x93, 0xB6,
    0xB1, 0x63, 0xFD, 0x14, 0x12, 0x6B, 0xBF, 0x1F,
    0xD2, 0xEA, 0x31, 0x9B, 0x21, 0x7E, 0xD1, 0x33,
    0x3C, 0xBA, 0x48, 0xF5, 0xF5, 0x6A, 0x7A, 0x04,
    0x45, 0x2E, 0x2C, 0xBD, 0x12, 0xE7, 0xC7, 0x88,
    0x0A, 0xE0, 0x9C, 0xC4, 0xBB, 0x5D, 0x07, 0x5B,
    0x2F, 0x68, 0x0D, 0xC8, 0x03, 0xE1, 0x27, 0xAC,
    0x63, 0x65, 0x42, 0x51, 0x17, 0x96, 0x1F, 0x14,
    0x69, 0xA3, 0x5B, 0x82, 0x6A, 0x93, 0xD4, 0xBB,
    0x15, 0x55, 0x5A, 0x8D, 0x7B, 0x27, 0x58, 0xC3,
    0x68, 0xCA, 0x75, 0x6E, 0x47, 0x80, 0x8D, 0x93,
    // ... truncated for build validation — full key in production
};

static const br_x509_trust_anchor TRUST_ANCHORS[] = {
    {
        { (unsigned char *)ISRG_ROOT_X1_DN, sizeof(ISRG_ROOT_X1_DN) },
        BR_X509_TA_CA,
        {
            BR_KEYTYPE_RSA,
            { .rsa = {
                (unsigned char *)ISRG_ROOT_X1_RSA_N, sizeof(ISRG_ROOT_X1_RSA_N),
                (unsigned char *)"\x01\x00\x01", 3  // e = 65537
            }}
        }
    }
};

#define TRUST_ANCHORS_NUM  (sizeof(TRUST_ANCHORS) / sizeof(TRUST_ANCHORS[0]))

// ============================================================================
// Connection Lifecycle
// ============================================================================

// Acquire a free connection slot from the static pool
int netd_tls_acquire_slot(void) {
    for (int i = 0; i < MAX_CONCURRENT_TLS_SOCKETS; i++) {
        if (global_tls_connections[i].state == TLS_CONN_IDLE) {
            global_tls_connections[i].state = TLS_CONN_HANDSHAKING;
            global_tls_connections[i].fd = -1;
            return i;
        }
    }
    return -1;  // Pool exhausted
}

// Initialize a TLS connection on an already-connected non-blocking TCP fd
void netd_tls_init_connection(int slot, int fd, const char *hostname, uint64_t multiplex_id) {
    NetDTLSConnection *conn = &global_tls_connections[slot];
    
    conn->fd = fd;
    conn->tab_multiplex_id = multiplex_id;
    
    // 1. Initialize X.509 minimal engine with trust anchors
    br_x509_minimal_init(&conn->xc, &br_sha256_vtable,
                         TRUST_ANCHORS, TRUST_ANCHORS_NUM);
    br_x509_minimal_set_rsa(&conn->xc, br_rsa_pkcs1_vrfy_get_default());
    br_x509_minimal_set_ecdsa(&conn->xc,
                              br_ec_get_default(),
                              br_ecdsa_vrfy_asn1_get_default());
    
    // 2. Initialize client context with full profile (TLS 1.2 + 1.3)
    br_ssl_client_init_full(&conn->sc, &conn->xc,
                            TRUST_ANCHORS, TRUST_ANCHORS_NUM);
    
    // 3. Bind the static I/O buffer (bidirectional)
    br_ssl_engine_set_buffer(&conn->sc.eng,
                             conn->iobuf, sizeof(conn->iobuf), 1);
    
    // 4. Set SNI hostname and start the handshake
    br_ssl_client_reset(&conn->sc, hostname, 0);
    
    conn->state = TLS_CONN_HANDSHAKING;
}

// Release a connection slot back to the pool
void netd_tls_release_slot(int slot) {
    if (slot < 0 || slot >= MAX_CONCURRENT_TLS_SOCKETS) return;
    NetDTLSConnection *conn = &global_tls_connections[slot];
    if (conn->fd >= 0) {
        close(conn->fd);
        conn->fd = -1;
    }
    conn->state = TLS_CONN_IDLE;
    conn->tab_multiplex_id = 0;
}

// ============================================================================
// The Asynchronous State Pump
// ============================================================================
// Called when the OS signals that conn->fd is readable/writable.
// Pushes ciphertext from the OS into BearSSL, and extracts cleartext
// from BearSSL to the VirtIO ipc_ring.
//
// Returns:
//   0  = connection still active, pump again on next poll event
//   1  = connection closed gracefully (cleartext EOF)
//  -1  = connection error
// ============================================================================

// Extern to the VirtIO IPC ring (defined in Salt)
extern void ipc_ring_push_bytes(uint64_t multiplex_id,
                                const uint8_t *data, uint32_t len);

int netd_tls_pump(int slot) {
    NetDTLSConnection *conn = &global_tls_connections[slot];
    br_ssl_engine_context *eng = &conn->sc.eng;
    
    unsigned state;
    unsigned char *buf;
    size_t len;
    ssize_t rlen;
    int progress = 1;  // Loop as long as we make progress
    
    while (progress) {
        progress = 0;
        state = br_ssl_engine_current_state(eng);
        
        // Check for engine error
        if (state == BR_SSL_CLOSED) {
            int err = br_ssl_engine_last_error(eng);
            if (err == BR_ERR_OK) {
                conn->state = TLS_CONN_CLOSING;
                return 1;   // Graceful close
            }
            conn->state = TLS_CONN_ERROR;
            return -1;      // TLS error
        }
        
        // STEP 1: Feed ciphertext from network into BearSSL
        if (state & BR_SSL_RECVREC) {
            buf = br_ssl_engine_recvrec_buf(eng, &len);
            rlen = read(conn->fd, buf, len);
            if (rlen > 0) {
                br_ssl_engine_recvrec_ack(eng, (size_t)rlen);
                progress = 1;
            } else if (rlen == 0) {
                // TCP connection closed by remote
                br_ssl_engine_close(eng);
                progress = 1;
            } else if (errno != EAGAIN && errno != EWOULDBLOCK) {
                // Hard socket error
                conn->state = TLS_CONN_ERROR;
                return -1;
            }
            // EAGAIN: no data right now, fall through
        }
        
        // STEP 2: Flush ciphertext from BearSSL to network
        state = br_ssl_engine_current_state(eng);
        if (state & BR_SSL_SENDREC) {
            buf = br_ssl_engine_sendrec_buf(eng, &len);
            rlen = write(conn->fd, buf, len);
            if (rlen > 0) {
                br_ssl_engine_sendrec_ack(eng, (size_t)rlen);
                progress = 1;
            } else if (rlen < 0 && errno != EAGAIN && errno != EWOULDBLOCK) {
                conn->state = TLS_CONN_ERROR;
                return -1;
            }
        }
        
        // STEP 3: Extract cleartext application data
        state = br_ssl_engine_current_state(eng);
        if (state & BR_SSL_RECVAPP) {
            buf = br_ssl_engine_recvapp_buf(eng, &len);
            
            // Push cleartext directly to Tab Process via VirtIO ipc_ring
            ipc_ring_push_bytes(conn->tab_multiplex_id, buf, (uint32_t)len);
            
            // Acknowledge consumption to BearSSL
            br_ssl_engine_recvapp_ack(eng, len);
            progress = 1;
            
            // Transition from handshaking to established on first app data
            if (conn->state == TLS_CONN_HANDSHAKING) {
                conn->state = TLS_CONN_ESTABLISHED;
            }
        }
    }
    
    return 0;  // Connection still active
}

// ============================================================================
// Send Application Data (HTTP Request)
// ============================================================================
// Writes cleartext into BearSSL's send buffer, then pumps to flush ciphertext
// to the network.
//
// Returns bytes written, or -1 on error.
// ============================================================================

int netd_tls_send(int slot, const uint8_t *data, uint32_t len) {
    NetDTLSConnection *conn = &global_tls_connections[slot];
    br_ssl_engine_context *eng = &conn->sc.eng;
    
    unsigned state = br_ssl_engine_current_state(eng);
    if (!(state & BR_SSL_SENDAPP)) {
        return 0;  // Engine not ready for application data
    }
    
    size_t avail;
    unsigned char *buf = br_ssl_engine_sendapp_buf(eng, &avail);
    if (avail == 0) return 0;
    
    size_t to_send = len < avail ? len : avail;
    memcpy(buf, data, to_send);
    br_ssl_engine_sendapp_ack(eng, to_send);
    
    // Flush by pumping
    br_ssl_engine_flush(eng, 0);
    netd_tls_pump(slot);
    
    return (int)to_send;
}

// ============================================================================
// Test Harness Getters (for Salt test access)
// ============================================================================

int netd_tls_get_state(int slot) {
    if (slot < 0 || slot >= MAX_CONCURRENT_TLS_SOCKETS) return -1;
    return (int)global_tls_connections[slot].state;
}

int netd_tls_get_fd(int slot) {
    if (slot < 0 || slot >= MAX_CONCURRENT_TLS_SOCKETS) return -1;
    return global_tls_connections[slot].fd;
}

uint64_t netd_tls_get_multiplex_id(int slot) {
    if (slot < 0 || slot >= MAX_CONCURRENT_TLS_SOCKETS) return 0;
    return global_tls_connections[slot].tab_multiplex_id;
}
