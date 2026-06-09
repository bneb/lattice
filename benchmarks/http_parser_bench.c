// =============================================================================
// HTTP Parser Benchmark — C Baseline
// =============================================================================
// Hand-rolled HTTP/1.1 parser using memchr for fair byte-level comparison
// with the Salt SWAR implementation.
//
// Build: clang -O3 -march=native http_parser_bench.c -o bin/http_parser_bench_c
// =============================================================================

#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <time.h>

#define ITERATIONS 1000000
#define MAX_HEADERS 32

typedef struct {
  const char *ptr;
  int64_t len;
} Slice;

typedef struct {
  Slice method;
  Slice uri;
  Slice version;
  int64_t header_count;
  Slice last_header_name;
  Slice last_header_value;
} HttpRequest;

static inline Slice make_slice(const char *ptr, int64_t len) {
  return (Slice){ptr, len};
}

// Parse HTTP/1.1 request — zero-copy (returns pointers into input buffer)
static int parse_request(const char *buf, int64_t len, HttpRequest *req) {
  // Find end of request line
  const char *line_end = (const char *)memchr(buf, '\r', len);
  if (!line_end)
    return 0;

  int64_t line_len = line_end - buf;

  // Find first space (method boundary)
  const char *sp1 = (const char *)memchr(buf, ' ', line_len);
  if (!sp1)
    return 0;

  req->method = make_slice(buf, sp1 - buf);

  // Find second space (URI boundary)
  int64_t remaining = line_len - (sp1 + 1 - buf);
  const char *sp2 = (const char *)memchr(sp1 + 1, ' ', remaining);
  if (!sp2)
    return 0;

  req->uri = make_slice(sp1 + 1, sp2 - sp1 - 1);
  req->version = make_slice(sp2 + 1, line_end - sp2 - 1);

  // Parse headers
  const char *cursor = line_end + 2; // skip \r\n
  const char *end = buf + len;
  req->header_count = 0;

  while (cursor < end && req->header_count < MAX_HEADERS) {
    // Check for empty line (end of headers)
    if (cursor + 1 < end && cursor[0] == '\r' && cursor[1] == '\n') {
      break;
    }

    // Find end of header line
    const char *hdr_end = (const char *)memchr(cursor, '\r', end - cursor);
    if (!hdr_end)
      break;

    int64_t hdr_len = hdr_end - cursor;

    // Split on ':'
    const char *colon = (const char *)memchr(cursor, ':', hdr_len);
    if (colon) {
      req->last_header_name = make_slice(cursor, colon - cursor);

      // Skip ': ' prefix on value
      const char *val_start = colon + 1;
      while (val_start < hdr_end && *val_start == ' ')
        val_start++;
      req->last_header_value = make_slice(val_start, hdr_end - val_start);
      req->header_count++;
    }

    cursor = hdr_end + 2; // skip \r\n
  }

  return 1;
}

static int64_t clock_ns(void) {
  struct timespec ts;
  clock_gettime(CLOCK_MONOTONIC, &ts);
  return (int64_t)ts.tv_sec * 1000000000LL + (int64_t)ts.tv_nsec;
}

int main(void) {
  const char *request = "GET /index.html HTTP/1.1\r\n"
                        "Host: www.example.com\r\n"
                        "User-Agent: Salt/1.0\r\n"
                        "Accept: text/html\r\n"
                        "Connection: keep-alive\r\n"
                        "\r\n";
  int64_t request_len = (int64_t)strlen(request);

  // Warm-up
  HttpRequest warmup;
  if (!parse_request(request, request_len, &warmup)) {
    printf("ERROR: parse failed\n");
    return 1;
  }

  printf("Method len: %lld\n", warmup.method.len);
  printf("URI len: %lld\n", warmup.uri.len);
  printf("Version len: %lld\n", warmup.version.len);
  printf("Headers: %lld\n", warmup.header_count);

  // Benchmark
  int64_t start = clock_ns();

  for (int64_t i = 0; i < ITERATIONS; i++) {
    HttpRequest req;
    if (!parse_request(request, request_len, &req)) {
      printf("FAIL at %lld\n", i);
      return 1;
    }
    __asm__ volatile("" : : "r"(&req) : "memory");
  }

  int64_t end = clock_ns();
  int64_t elapsed_ns = end - start;
  int64_t elapsed_ms = elapsed_ns / 1000000;
  int64_t req_per_sec = (ITERATIONS * 1000000000LL) / elapsed_ns;

  printf("=== C HTTP Parser Benchmark ===\n");
  printf("Iterations: %d\n", ITERATIONS);
  printf("Time: %lld ms\n", elapsed_ms);
  printf("Throughput: %lld req/s\n", req_per_sec);

  return 0;
}
