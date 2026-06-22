// =============================================================================
// tcp_native_bridge.c — macOS-native TCP + kqueue for Salt benchmarks
// =============================================================================
// Provides real BSD socket operations so LETTUCE can run natively on macOS
// for benchmarking against redis-server.
//
// Functions are extern'd from std.net.tcp_native and follow the C ABI.
// =============================================================================

#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/event.h>
#include <sys/socket.h>
#include <sys/time.h>
#include <unistd.h>

// ─── TCP ─────────────────────────────────────────────────────────────────────

int32_t native_tcp_listen(int32_t port) {
    int fd = socket(AF_INET, SOCK_STREAM, 0);
    if (fd < 0) return -1;

    int opt = 1;
    setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt));
    setsockopt(fd, SOL_SOCKET, SO_REUSEPORT, &opt, sizeof(opt));

    struct sockaddr_in addr;
    memset(&addr, 0, sizeof(addr));
    addr.sin_family = AF_INET;
    addr.sin_addr.s_addr = INADDR_ANY;
    addr.sin_port = htons((uint16_t)port);

    if (bind(fd, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
        close(fd);
        return -1;
    }
    if (listen(fd, 128) < 0) {
        close(fd);
        return -1;
    }

    // Set non-blocking
    int flags = fcntl(fd, F_GETFL, 0);
    fcntl(fd, F_SETFL, flags | O_NONBLOCK);

    return fd;
}

int32_t native_tcp_accept(int32_t listen_fd) {
    struct sockaddr_in client_addr;
    socklen_t client_len = sizeof(client_addr);
    int fd = accept(listen_fd, (struct sockaddr *)&client_addr, &client_len);
    if (fd < 0) return -1;

    int opt = 1;
    setsockopt(fd, IPPROTO_TCP, TCP_NODELAY, &opt, sizeof(opt));

    int flags = fcntl(fd, F_GETFL, 0);
    fcntl(fd, F_SETFL, flags | O_NONBLOCK);

    return fd;
}

int64_t native_tcp_recv(int32_t fd, uint8_t *buf, int64_t len) {
    ssize_t n = recv(fd, buf, (size_t)len, 0);
    if (n < 0) {
        if (errno == EAGAIN || errno == EWOULDBLOCK) return 0;
        return -1;
    }
    return (int64_t)n;
}

int64_t native_tcp_send(int32_t fd, uint8_t *buf, int64_t len) {
    ssize_t n = send(fd, buf, (size_t)len, 0);
    if (n < 0) {
        if (errno == EAGAIN || errno == EWOULDBLOCK) return 0;
        return -1;
    }
    return (int64_t)n;
}

void native_tcp_close(int32_t fd) {
    if (fd >= 0) close(fd);
}

// ─── kqueue ──────────────────────────────────────────────────────────────────

int32_t native_kq_create(void) {
    return kqueue();
}

int32_t native_kq_register(int32_t kq, int32_t fd, int32_t filter) {
    struct kevent ev;
    int16_t kq_filter = (filter == 1) ? EVFILT_READ : EVFILT_WRITE;
    EV_SET(&ev, fd, kq_filter, EV_ADD | EV_ENABLE, 0, 0, NULL);
    return kevent(kq, &ev, 1, NULL, 0, NULL);
}

int32_t native_kq_wait(int32_t kq, int64_t *events, int32_t max_events, int32_t timeout_ms) {
    struct kevent evs[64];
    struct timespec ts;
    struct timespec *tsp = NULL;

    if (timeout_ms >= 0) {
        ts.tv_sec = timeout_ms / 1000;
        ts.tv_nsec = (timeout_ms % 1000) * 1000000L;
        tsp = &ts;
    }

    int n = kevent(kq, NULL, 0, evs, max_events < 64 ? max_events : 64, tsp);
    if (n < 0) return -1;

    // Pack results as [fd0, filter0, fd1, filter1, ...]
    for (int i = 0; i < n; i++) {
        events[i * 2] = (int64_t)(intptr_t)evs[i].ident;
        events[i * 2 + 1] = (evs[i].filter == EVFILT_READ) ? 1 : 2;
    }
    return n;
}

void native_kq_close_fd(int32_t kq, int32_t fd) {
    struct kevent ev;
    EV_SET(&ev, fd, EVFILT_READ, EV_DELETE, 0, 0, NULL);
    kevent(kq, &ev, 1, NULL, 0, NULL);
    EV_SET(&ev, fd, EVFILT_WRITE, EV_DELETE, 0, 0, NULL);
    kevent(kq, &ev, 1, NULL, 0, NULL);
}
