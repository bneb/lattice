// =============================================================================
// C10M Echo Server — C/kqueue (Baseline)
//
// Single-threaded, non-blocking TCP echo server using raw kqueue on Darwin.
// Accepts connections, reads data, echoes it back with case-swapped first byte.
//
// Build:  clang -O3 -o echo_c echo_c.c
// Run:    ./echo_c <port>
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
#include <unistd.h>

#define MAX_EVENTS 256
#define BUF_SIZE 4096

static int make_nonblocking(int fd) {
  int flags = fcntl(fd, F_GETFL, 0);
  return fcntl(fd, F_SETFL, flags | O_NONBLOCK);
}

int main(int argc, char *argv[]) {
  int port = argc > 1 ? atoi(argv[1]) : 8080;

  // Create listener socket
  int listen_fd = socket(AF_INET, SOCK_STREAM, 0);
  if (listen_fd < 0) {
    perror("socket");
    return 1;
  }

  int opt = 1;
  setsockopt(listen_fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt));
  setsockopt(listen_fd, SOL_SOCKET, SO_REUSEPORT, &opt, sizeof(opt));
  setsockopt(listen_fd, IPPROTO_TCP, TCP_NODELAY, &opt, sizeof(opt));
  make_nonblocking(listen_fd);

  struct sockaddr_in addr = {
      .sin_family = AF_INET,
      .sin_addr.s_addr = INADDR_ANY,
      .sin_port = htons(port),
  };
  if (bind(listen_fd, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
    perror("bind");
    return 1;
  }
  if (listen(listen_fd, 4096) < 0) {
    perror("listen");
    return 1;
  }

  // Create kqueue
  int kq = kqueue();
  if (kq < 0) {
    perror("kqueue");
    return 1;
  }

  // Register listener for read events
  struct kevent ev;
  EV_SET(&ev, listen_fd, EVFILT_READ, EV_ADD | EV_ENABLE, 0, 0, NULL);
  kevent(kq, &ev, 1, NULL, 0, NULL);

  printf("[C/kqueue] Echo server listening on port %d\n", port);

  struct kevent events[MAX_EVENTS];
  char buf[BUF_SIZE];
  uint64_t total_bytes = 0;
  uint64_t total_conns = 0;

  while (1) {
    int n = kevent(kq, NULL, 0, events, MAX_EVENTS, NULL);
    if (n < 0) {
      if (errno == EINTR)
        continue;
      perror("kevent");
      break;
    }

    for (int i = 0; i < n; i++) {
      int fd = (int)events[i].ident;

      if (fd == listen_fd) {
        // Accept new connections
        while (1) {
          int client = accept(listen_fd, NULL, NULL);
          if (client < 0) {
            if (errno == EAGAIN || errno == EWOULDBLOCK)
              break;
            continue;
          }
          make_nonblocking(client);
          int tcp_opt = 1;
          setsockopt(client, IPPROTO_TCP, TCP_NODELAY, &tcp_opt,
                     sizeof(tcp_opt));

          EV_SET(&ev, client, EVFILT_READ, EV_ADD | EV_ENABLE, 0, 0, NULL);
          kevent(kq, &ev, 1, NULL, 0, NULL);
          total_conns++;
        }
      } else if (events[i].flags & EV_EOF) {
        close(fd);
      } else {
        // Echo: read → case-swap first byte → write
        ssize_t bytes = read(fd, buf, BUF_SIZE);
        if (bytes > 0) {
          buf[0] ^= 0x20; // Case swap
          write(fd, buf, bytes);
          total_bytes += bytes;
        } else if (bytes == 0) {
          close(fd);
        }
      }
    }
  }

  close(kq);
  close(listen_fd);
  return 0;
}
