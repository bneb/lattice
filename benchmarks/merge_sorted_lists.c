#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>

typedef struct ListNode {
  int val;
  uint32_t next;
} ListNode;

typedef struct Arena {
  ListNode *data;
  uint32_t size;
  uint32_t capacity;
} Arena;

Arena *arena_new(uint32_t capacity) {
  Arena *a = malloc(sizeof(Arena));
  a->data = malloc(sizeof(ListNode) * capacity);
  a->size = 0;
  a->capacity = capacity;
  return a;
}

uint32_t arena_alloc(Arena *a, ListNode n) {
  uint32_t idx = a->size++;
  a->data[idx] = n;
  return idx;
}

void arena_reset(Arena *a) {
  a->size = 0;
}

#define NULL_NODE 0xFFFFFFFF

uint32_t create_list(Arena *a, int len, int start, int step) {
  if (len <= 0)
    return NULL_NODE;
  uint32_t next = create_list(a, len - 1, start + step, step);
  ListNode node = {start, next};
  return arena_alloc(a, node);
}

uint32_t merge_two_lists(Arena *a, uint32_t l1, uint32_t l2) {
  if (l1 == NULL_NODE)
    return l2;
  if (l2 == NULL_NODE)
    return l1;

  if (a->data[l1].val < a->data[l2].val) {
    a->data[l1].next = merge_two_lists(a, a->data[l1].next, l2);
    return l1;
  } else {
    a->data[l2].next = merge_two_lists(a, l1, a->data[l2].next);
    return l2;
  }
}

int main() {
  int checksum = 0;

  Arena *arena = arena_new(1000);

  for (int i = 0; i < 5000; i++) {
    arena_reset(arena);
    uint32_t l1 = create_list(arena, 100, 0, 2);
    uint32_t l2 = create_list(arena, 100, 1, 2);
    uint32_t merged = merge_two_lists(arena, l1, l2);

    uint32_t curr = merged;
    while (curr != NULL_NODE) {
      checksum += arena->data[curr].val;
      curr = arena->data[curr].next;
    }
  }

  printf("Checksum: %d\n", checksum);
  return 0;
}
