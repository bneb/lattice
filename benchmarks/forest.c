#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <stdint.h>

typedef struct Node {
  uint64_t left;
  uint64_t right;
  int val;
} Node;

typedef struct Arena {
  Node *data;
  uint32_t size;
  uint32_t capacity;
} Arena;

Arena *arena_new(uint32_t capacity) {
  Arena *a = malloc(sizeof(Arena));
  a->data = malloc(sizeof(Node) * capacity);
  a->size = 0;
  a->capacity = capacity;
  return a;
}

uint32_t arena_alloc(Arena *a, Node n) {
  uint32_t idx = a->size++;
  a->data[idx] = n;
  return idx;
}

void arena_reset(Arena *a) {
  a->size = 0;
}

#define NULL_NODE 0xFFFFFFFFFFFFFFFFULL

uint64_t make_tree(Arena *a, int depth) {
  if (depth == 0) {
    return NULL_NODE;
  }
  uint64_t left = make_tree(a, depth - 1);
  uint64_t right = make_tree(a, depth - 1);
  Node n = {left, right, depth};
  return arena_alloc(a, n);
}

int main() {
  struct timespec ts;
  clock_gettime(CLOCK_MONOTONIC, &ts);
  long long t0 = ts.tv_sec * 1000000000LL + ts.tv_nsec;

  // Depth 22 -> ~4M nodes (2^22 - 1)
  Arena *arena = arena_new(4200000);
  uint32_t root = make_tree(arena, 22);

  clock_gettime(CLOCK_MONOTONIC, &ts);
  long long t1 = ts.tv_sec * 1000000000LL + ts.tv_nsec;

  printf("Build Time: %lld ns\n", t1 - t0);

  clock_gettime(CLOCK_MONOTONIC, &ts);
  long long t2 = ts.tv_sec * 1000000000LL + ts.tv_nsec;

  arena_reset(arena);

  clock_gettime(CLOCK_MONOTONIC, &ts);
  long long t3 = ts.tv_sec * 1000000000LL + ts.tv_nsec;

  printf("Free Time: %lld ns\n", t3 - t2);
  printf("Total Churn: %lld ns\n", t3 - t0);

  return 0;
}
