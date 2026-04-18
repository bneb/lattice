// ============================================================================
// Epic 34: TLS/HTTPS Transport Pipeline — Connection Matrix
// ============================================================================
// Static block of TLS connection states for NetD.
// Each connection uses BearSSL's br_ssl_client_context with pre-allocated
// I/O buffers. Zero dynamic heap allocations during active connection lifecycle.
// ============================================================================

#ifndef NETD_TLS_STATE_H
#define NETD_TLS_STATE_H

#include <bearssl.h>
#include <stdint.h>
#include <stdbool.h>

#define MAX_CONCURRENT_TLS_SOCKETS 64
#define TLS_IO_BUFFER_SIZE 16384  // 16KB per BearSSL spec

typedef enum {
    TLS_CONN_IDLE = 0,
    TLS_CONN_HANDSHAKING,
    TLS_CONN_ESTABLISHED,
    TLS_CONN_CLOSING,
    TLS_CONN_ERROR
} NetDTLSState;

typedef struct {
    int fd;                      // Non-blocking TCP socket fd
    uint64_t tab_multiplex_id;   // For routing cleartext back over ipc_ring
    NetDTLSState state;

    br_ssl_client_context sc;
    br_x509_minimal_context xc;

    // BearSSL requires explicitly provided memory buffers
    unsigned char iobuf[BR_SSL_BUFSIZE_BIDI];
} NetDTLSConnection;

extern NetDTLSConnection global_tls_connections[MAX_CONCURRENT_TLS_SOCKETS];

#endif // NETD_TLS_STATE_H
