// =============================================================================
// Pulse Cannon — High-Concurrency Echo Load Generator
//
// Maintains CONNS persistent TCP connections and blasts BATCH packets
// through them in a tight loop. Measures throughput (pkt/s) and
// average latency (µs). Designed to saturate the target server without
// becoming the bottleneck itself.
//
// Build:  clang -O3 -o pulse_cannon stress_echo.c
// Run:    ./pulse_cannon [host] [port] [connections] [packets]
//
// Default: 127.0.0.1:8080, 1000 conns, 1M packets
// =============================================================================

#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/time.h>
#include <unistd.h>

static double get_time_us(void) {
  struct timeval tv;
  gettimeofday(&tv, NULL);
  return (double)tv.tv_sec * 1e6 + (double)tv.tv_usec;
}

int main(int argc, char *argv[]) {
  const char *host = argc > 1 ? argv[1] : "127.0.0.1";
  int port = argc > 2 ? atoi(argv[2]) : 8080;
  int conns = argc > 3 ? atoi(argv[3]) : 1000;
  int packets = argc > 4 ? atoi(argv[4]) : 1000000;

  // Clamp connections to avoid fd exhaustion
  if (conns > 10000)
    conns = 10000;

  struct sockaddr_in addr;
  memset(&addr, 0, sizeof(addr));
  addr.sin_family = AF_INET;
  addr.sin_port = htons(port);
  inet_pton(AF_INET, host, &addr.sin_addr);

  // Phase 1: Open persistent connections
  printf("[PULSE CANNON] Target: %s:%d\n", host, port);
  printf("[PULSE CANNON] Opening %d connections...\n", conns);

  int *fds = (int *)malloc(conns * sizeof(int));
  int connected = 0;

  for (int i = 0; i < conns; i++) {
    fds[i] = socket(AF_INET, SOCK_STREAM, 0);
    if (fds[i] < 0) {
      fprintf(stderr, "  socket() failed at %d: %s\n", i, strerror(errno));
      break;
    }

    // TCP_NODELAY for minimal latency
    int opt = 1;
    setsockopt(fds[i], IPPROTO_TCP, TCP_NODELAY, &opt, sizeof(opt));

    if (connect(fds[i], (struct sockaddr *)&addr, sizeof(addr)) < 0) {
      fprintf(stderr, "  connect() failed at %d: %s\n", i, strerror(errno));
      close(fds[i]);
      fds[i] = -1;
      break;
    }
    connected++;
  }

  printf("[PULSE CANNON] %d / %d connections established\n", connected, conns);
  if (connected == 0) {
    fprintf(stderr, "No connections established. Is the server running?\n");
    free(fds);
    return 1;
  }

  // Phase 2: Blast packets in a tight loop
  const char *msg = "Hello\n";
  const int msg_len = 6;
  char recv_buf[256];

  printf("[PULSE CANNON] Blasting %d packets across %d connections...\n",
         packets, connected);

  double start_us = get_time_us();
  int successes = 0;
  int failures = 0;

  for (int i = 0; i < packets; i++) {
    int fd = fds[i % connected];
    if (fd < 0)
      continue;

    // Send
    ssize_t sent = send(fd, msg, msg_len, 0);
    if (sent <= 0) {
      failures++;
      continue;
    }

    // Receive (blocking — tight-loop reap)
    ssize_t recvd = recv(fd, recv_buf, sizeof(recv_buf), 0);
    if (recvd > 0) {
      successes++;
    } else {
      failures++;
    }
  }

  double end_us = get_time_us();
  double elapsed_s = (end_us - start_us) / 1e6;
  double elapsed_us = end_us - start_us;

  // Phase 3: Report
  printf("\n");
  printf("═══════════════════════════════════════════\n");
  printf("  PULSE CANNON RESULTS\n");
  printf("═══════════════════════════════════════════\n");
  printf("  Connections:  %d\n", connected);
  printf("  Packets sent: %d\n", packets);
  printf("  Successes:    %d\n", successes);
  printf("  Failures:     %d\n", failures);
  printf("  Total Time:   %.4f seconds\n", elapsed_s);
  printf("  Throughput:   %.0f packets/sec\n", successes / elapsed_s);
  printf("  Avg Latency:  %.2f µs/packet\n", elapsed_us / successes);
  printf("═══════════════════════════════════════════\n");

  // Cleanup
  for (int i = 0; i < conns; i++) {
    if (fds[i] >= 0)
      close(fds[i]);
  }
  free(fds);

  return 0;
}
