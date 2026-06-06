#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

typedef struct Arena {
  uint8_t *data;
} Arena;

Arena *arena_new(uint32_t capacity) {
  Arena *a = malloc(sizeof(Arena));
  a->data = malloc(capacity);
  return a;
}

int sieve(int limit, Arena *arena) {
  uint8_t *is_prime = arena->data;

  // Init
  for (int i = 0; i <= limit; i++) {
    is_prime[i] = 1;
  }
  is_prime[0] = 0;
  is_prime[1] = 0;

  // Sieve
  for (long p = 2; p <= limit; p++) {
    if ((long)p * p > limit)
      break;
    if (is_prime[p]) {
      long j = p * p;
      while (j <= limit) {
        is_prime[j] = 0;
        j += p;
      }
    }
  }

  // Count
  int count = 0;
  for (int k = 0; k <= limit; k++) {
    if (is_prime[k]) {
      count++;
    }
  }

  return count;
}

int main() {
  Arena *arena = arena_new(1000000 + 1);
  long total_primes = 0;
  for (int k = 0; k < 200; k++) {
    total_primes += sieve(1000000, arena);
  }

  if (total_primes != 15699600) {
    printf("Mismatch: %ld\n", total_primes);
    return 1;
  }
  return 0;
}
