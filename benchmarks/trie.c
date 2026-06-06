#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

typedef struct TrieNode {
  uint32_t children[26];
  bool is_word;
} TrieNode;

typedef struct Arena {
  TrieNode *data;
  uint32_t size;
  uint32_t capacity;
} Arena;

Arena *arena_new(uint32_t capacity) {
  Arena *a = malloc(sizeof(Arena));
  a->data = malloc(sizeof(TrieNode) * capacity);
  a->size = 0;
  a->capacity = capacity;
  return a;
}

uint32_t arena_alloc(Arena *a) {
  uint32_t idx = a->size++;
  for (int i = 0; i < 26; i++) {
    a->data[idx].children[i] = 0xFFFFFFFF;
  }
  a->data[idx].is_word = false;
  return idx;
}

void insert(Arena *a, uint32_t root, const char *word) {
  uint32_t curr = root;
  for (int i = 0; word[i] != '\0'; i++) {
    int idx = word[i] - 'a';
    if (a->data[curr].children[idx] == 0xFFFFFFFF) {
      a->data[curr].children[idx] = arena_alloc(a);
    }
    curr = a->data[curr].children[idx];
  }
  a->data[curr].is_word = true;
}

bool search(Arena *a, uint32_t root, const char *word) {
  uint32_t curr = root;
  for (int i = 0; word[i] != '\0'; i++) {
    int idx = word[i] - 'a';
    if (a->data[curr].children[idx] == 0xFFFFFFFF) {
      return false;
    }
    curr = a->data[curr].children[idx];
  }
  return a->data[curr].is_word;
}

int main() {
  Arena *arena = arena_new(3000000);
  uint32_t root = arena_alloc(arena);

  char word[6];
  word[5] = '\0';

  printf("Inserting 700k words...\n");
  for (int i = 0; i < 700000; i++) {
    word[0] = (i % 26) + 97;
    word[1] = ((i / 26) % 26) + 97;
    word[2] = ((i / 676) % 26) + 97;
    word[3] = ((i / 17576) % 26) + 97;
    word[4] = (i % 7) + 97;
    insert(arena, root, word);
  }

  printf("Searching 700k words...\n");
  int found = 0;
  for (int i = 0; i < 700000; i++) {
    word[0] = (i % 26) + 97;
    word[1] = ((i / 26) % 26) + 97;
    word[2] = ((i / 676) % 26) + 97;
    word[3] = ((i / 17576) % 26) + 97;
    word[4] = (i % 7) + 97;
    if (search(arena, root, word)) {
      found++;
    }
  }
  printf("Found: %d\n", found);

  return 0;
}
