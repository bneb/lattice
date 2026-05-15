#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern int tests__test_e2e_multiprocess__tests_e2e_multiprocess_run();
extern int salt_browser_main(uint32_t argc, uint64_t argv);
extern int32_t ext_ipc_init_shared_memory(int32_t fd);

// Epic 68: Global IPC FD for the renderer process
static int32_t g_ipc_fd = -1;

int main(int argc, char **argv) {
  // Parse CLI arguments for IPC FD (must happen before Salt main)
  for (int i = 1; i < argc; i++) {
    if (strcmp(argv[i], "-e") == 0 && i + 1 < argc) {
      if (strcmp(argv[i + 1],
                 "_tests__test_e2e_multiprocess__tests_e2e_multiprocess_run") ==
          0) {
        return tests__test_e2e_multiprocess__tests_e2e_multiprocess_run();
      }
    }
    if (strcmp(argv[i], "--ipc-fd") == 0 && i + 1 < argc) {
      g_ipc_fd = atoi(argv[i + 1]);
      fprintf(stderr, "[Renderer] IPC FD received: %d\n", g_ipc_fd);
      int32_t res = ext_ipc_init_shared_memory(g_ipc_fd);
      if (res < 0) {
        fprintf(stderr,
                "[Renderer] FATAL: Failed to init shared memory from FD %d\n",
                g_ipc_fd);
        return 1;
      }
      i++; // skip the value
    }
  }

  // Epic 88: If IPC is active, run the UI forever natively.
  if (g_ipc_fd >= 0) {
    extern void set_max_test_frames(uint64_t);
    set_max_test_frames(0);
  }

  // Call the Salt main() with argc/argv so it can parse --url,
  // initialize JSC, DOM, fonts, create root node, and enter app_run_loop.
  extern uint64_t dom_ptr_LAYOUT_W();
  extern uint64_t dom_ptr_LAYOUT_SCROLL_X();
  extern uint64_t dom_ptr_LAYOUT_SCROLL_Y();
  extern uint64_t dom_ptr_VIEWPORT_W();
  extern uint64_t dom_ptr_VIEWPORT_H();
  
  fprintf(stderr, "[DIAG-C] LAYOUT_W: %llx\n", dom_ptr_LAYOUT_W());
  fprintf(stderr, "[DIAG-C] LAYOUT_SCROLL_X: %llx\n", dom_ptr_LAYOUT_SCROLL_X());
  fprintf(stderr, "[DIAG-C] LAYOUT_SCROLL_Y: %llx\n", dom_ptr_LAYOUT_SCROLL_Y());
  fprintf(stderr, "[DIAG-C] VIEWPORT_W: %llx\n", dom_ptr_VIEWPORT_W());
  fprintf(stderr, "[DIAG-C] VIEWPORT_H: %llx\n", dom_ptr_VIEWPORT_H());

  return salt_browser_main((uint32_t)argc, (uint64_t)argv);
}

#include <stdint.h>
#include <sys/wait.h>

int ext_WIFSIGNALED(int status) { return WIFSIGNALED(status); }

void ext_mac_update_omnibox(uint64_t ptr, uint32_t len) {
  // Stub for renderer
}

// ============================================================================
// Salt Unmangled Wrappers
// ============================================================================
// These functions are requested by Salt via fully mangled names
// but are exported by their modules as unmangled names due to @no_mangle.

extern void ext_salt_append_child(uint64_t, uint64_t);
void user__browser__dom__append_child(uint64_t p, uint64_t c) {
  ext_salt_append_child(p, c);
}

extern int32_t bind_event_listener(uint32_t, uint32_t, uint32_t);
int32_t user__browser__dom__bind_event_listener(uint32_t n, uint32_t s,
                                                uint32_t pc) {
  return bind_event_listener(n, s, pc);
}

extern uint64_t ext_salt_create_node(uint32_t);
uint64_t user__browser__dom__create_node(uint32_t tag) {
  return ext_salt_create_node(tag);
}

extern int32_t dom_set_text(uint64_t, uint64_t, uint32_t);
int32_t user__browser__dom__dom_set_text(uint64_t id, uint64_t ptr,
                                         uint32_t len) {
  return dom_set_text(id, ptr, len);
}

extern uint32_t get_element_by_id_hash(uint64_t);
uint32_t user__browser__dom__get_element_by_id_hash(uint64_t h) {
  return get_element_by_id_hash(h);
}

extern void remove_child(uint64_t, uint64_t);
void user__browser__dom__remove_child(uint64_t p, uint64_t c) {
  remove_child(p, c);
}

extern uint64_t airlock_get_ptr();
uint64_t user__browser__alloc__airlock__airlock_get_ptr() {
  return airlock_get_ptr();
}

extern int32_t sys_ipc_init_shared_memory(int32_t);
int32_t user__browser__ipc_shared__sys_ipc_init_shared_memory(int32_t fd) {
  return sys_ipc_init_shared_memory(fd);
}

// ============================================================================
// Media Globals Fix
// ============================================================================
// LLVM GlobalMerge strips exported scalar globals in Salt, so we define them
// in C and declare them as extern in Salt to ensure stable linking.

extern uint32_t user__browser__media__MEDIA_HEAD;
extern uint32_t user__browser__media__MEDIA_TAIL;

uint32_t ext_get_media_head() { return user__browser__media__MEDIA_HEAD; }
void ext_set_media_head(uint32_t val) {
  user__browser__media__MEDIA_HEAD = val;
}
uint32_t ext_get_media_tail() { return user__browser__media__MEDIA_TAIL; }
void ext_set_media_tail(uint32_t val) {
  user__browser__media__MEDIA_TAIL = val;
}

// C-ABI trampoline: Salt cannot call @no_mangle'd flush_frame cross-module
extern void flush_frame(int32_t width, int32_t height);
void ext_flush_frame(int32_t width, int32_t height) {
  flush_frame(width, height);
}

// ============================================================================
// Missing Symbols Fix (Epic 108)
// ============================================================================

uint32_t hash_string(uint64_t ptr, uint32_t len) {
  uint32_t hash = 2166136261U;
  const uint8_t *data = (const uint8_t *)ptr;
  for (uint32_t i = 0; i < len; i++) {
    hash ^= data[i];
    hash *= 16777619U;
  }
  return hash;
}

void css_arena_inc_count(void) {}
void css_arena_set_hash(uint32_t slot, uint32_t hash) {}
void ext_engine_process_mouse_down(float x, float y) {}

uint64_t sovereign_arena_alloc(uint64_t size) {
  // Bridge implementation for ResilientArena in macOS multiprocess mode
  return (uint64_t)calloc(1, size);
}
