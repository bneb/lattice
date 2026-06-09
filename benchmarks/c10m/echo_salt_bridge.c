// =============================================================================
// Echo Salt Bridge — C implementations for Salt extern fn declarations
//
// Consolidates socket/kqueue logic into three functions to minimize
// the number of extern fn declarations Salt needs. The bridge handles:
//   1. Server setup (socket + bind + listen + nonblock + reuseaddr)
//   2. kqueue creation and fd registration
//   3. Single poll step: accept new client OR echo existing client
//
// Build: clang -O3 -c echo_salt_bridge.c -o echo_salt_bridge.o
// =============================================================================

#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/event.h>
#include <sys/socket.h>
#include <sys/time.h>
#include <unistd.h>

// =============================================================================
// 0. Port from CLI — reads argv[1] via salt runtime's salt_get_argv
// =============================================================================
extern int32_t salt_get_argc(void);
extern const char *salt_get_argv(int32_t idx);

int32_t echo_get_port(int32_t default_port) {
  if (salt_get_argc() > 1) {
    const char *arg = salt_get_argv(1);
    if (arg) {
      int p = atoi(arg);
      if (p > 0 && p < 65536) return p;
    }
  }
  return default_port;
}

// =============================================================================
// 1. Server Init — returns listen fd or -1
// =============================================================================
int32_t echo_server_init(int32_t port) {
  int fd = socket(AF_INET, SOCK_STREAM, 0);
  if (fd < 0)
    return -1;

  int opt = 1;
  setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt));
  setsockopt(fd, SOL_SOCKET, SO_REUSEPORT, &opt, sizeof(opt));

  struct sockaddr_in addr;
  memset(&addr, 0, sizeof(addr));
  addr.sin_family = AF_INET;
  addr.sin_addr.s_addr = INADDR_ANY;
  addr.sin_port = htons(port);

  if (bind(fd, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
    close(fd);
    return -1;
  }
  if (listen(fd, 4096) < 0) {
    close(fd);
    return -1;
  }

  // Set non-blocking
  int flags = fcntl(fd, F_GETFL, 0);
  fcntl(fd, F_SETFL, flags | O_NONBLOCK);

  return fd;
}

// =============================================================================
// 2. kqueue Create — creates kq and registers listen fd
// =============================================================================
int32_t echo_kq_create(int32_t listen_fd) {
  int kq = kqueue();
  if (kq < 0)
    return -1;

  struct kevent ev;
  EV_SET(&ev, listen_fd, EVFILT_READ, EV_ADD | EV_ENABLE, 0, 0, NULL);
  kevent(kq, &ev, 1, NULL, 0, NULL);

  return kq;
}

// =============================================================================
// 3. Accept Loop
// =============================================================================
void echo_accept_all(int32_t kq, int32_t listen_fd) {
  while (1) {
    int client = accept(listen_fd, NULL, NULL);
    if (client < 0) {
      if (errno == EAGAIN || errno == EWOULDBLOCK) break;
      continue;
    }
    // Set non-blocking + TCP_NODELAY
    int flags = fcntl(client, F_GETFL, 0);
    fcntl(client, F_SETFL, flags | O_NONBLOCK);
    int opt = 1;
    setsockopt(client, IPPROTO_TCP, TCP_NODELAY, &opt, sizeof(opt));

    // Register with kqueue
    struct kevent cev;
    EV_SET(&cev, client, EVFILT_READ, EV_ADD | EV_ENABLE, 0, 0, NULL);
    kevent(kq, &cev, 1, NULL, 0, NULL);
  }
}

// =============================================================================
// Timing (also used by runtime.c, but included here for standalone build)
// =============================================================================
int64_t clock_gettime_ns(void) {
  struct timeval tv;
  gettimeofday(&tv, NULL);
  return (int64_t)tv.tv_sec * 1000000000LL + (int64_t)tv.tv_usec * 1000LL;
}
