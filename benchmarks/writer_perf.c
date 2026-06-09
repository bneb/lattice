// Writer Protocol Performance Benchmark - C baseline
// Matches Salt's memcpy and direct pointer write implementation

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define ITERATIONS 10000000
#define BUFFER_SIZE 4096

typedef struct {
  char *data;
  size_t len;
  size_t capacity;
} Buffer;

static inline int write_i32_to_ptr(char *base, int val) {
  if (val == 0) {
    base[0] = '0';
    return 1;
  }
  char tmp[16];
  int len = 0;
  int n = val;
  int neg = n < 0;
  if (neg) n = -n;
  
  while (n > 0) {
    tmp[len++] = '0' + (n % 10);
    n /= 10;
  }
  
  int offset = 0;
  if (neg) base[offset++] = '-';
  for (int i = len - 1; i >= 0; i--) {
    base[offset++] = tmp[i];
  }
  return offset;
}

static inline int write_i64_to_ptr(char *base, long val) {
  if (val == 0) {
    base[0] = '0';
    return 1;
  }
  char tmp[24];
  int len = 0;
  long n = val;
  int neg = n < 0;
  if (neg) n = -n;
  
  while (n > 0) {
    tmp[len++] = '0' + (n % 10);
    n /= 10;
  }
  
  int offset = 0;
  if (neg) base[offset++] = '-';
  for (int i = len - 1; i >= 0; i--) {
    base[offset++] = tmp[i];
  }
  return offset;
}

int main(int argc, char **argv) {
  char storage[BUFFER_SIZE];
  Buffer buf = {.data = storage, .len = 0, .capacity = BUFFER_SIZE};

  long total_len = 0;
  char checksum = 0;

  for (int i = 0; i < ITERATIONS; i++) {
    buf.len = 0;
    int offset = 0;

    memcpy(buf.data + offset, "Item ", 5);
    offset += 5;
    
    offset += write_i32_to_ptr(buf.data + offset, i);
    
    memcpy(buf.data + offset, ": val = ", 8);
    offset += 8;
    
    offset += write_i64_to_ptr(buf.data + offset, (long)i * 1000);

    buf.len = offset;
    total_len += buf.len;
    
    checksum ^= buf.data[0];
    checksum ^= buf.data[buf.len - 1];
  }

  if (total_len == 0 || checksum == (char)255)
    return 1;
  return 0;
}
