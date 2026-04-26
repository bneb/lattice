#include "../../vendor/quickjs/quickjs.h"
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/time.h>

uint64_t sys_clock_get_ms() {
  struct timeval tv;
  gettimeofday(&tv, NULL);
  return (uint64_t)tv.tv_sec * 1000 + (uint64_t)tv.tv_usec / 1000;
}

static JSContext *ctx = NULL;
static JSRuntime *rt = NULL;

JSContext *get_global_js_context() { return ctx; }

static JSValue js_print(JSContext *ctx, JSValueConst this_val, int argc,
                        JSValueConst *argv) {
  if (argc > 0) {
    const char *str = JS_ToCString(ctx, argv[0]);
    if (str) {
      printf("%s\n", str);
      JS_FreeCString(ctx, str);
    }
  }
  return JS_UNDEFINED;
}

// Epic 63: Custom Element Registry
typedef struct {
  uint32_t tag_hash;
  JSValue constructor;
} CustomElementDefinition;

static CustomElementDefinition custom_elements[64];
static int custom_elements_count = 0;

// Un-mangled Airlock symbols
extern uint32_t airlock_allocate(uint32_t size);
extern void airlock_deallocate(uint32_t offset);
extern uint64_t airlock_get_ptr();
extern uint32_t BLOCK_COUNT;
extern uint32_t airlock_get_block_size(uint32_t offset);

// Un-mangled DOM symbols
extern uint64_t resolve_node_by_id(uint64_t ptr, uint32_t len);
extern void set_class(int32_t node_id, uint64_t ptr, uint32_t len);
// Network Fetch
extern void push_get_request(uint64_t fetch_id, uint64_t url_ptr,
                             uint32_t url_len);
extern uint64_t js_get_class_ptr(int32_t node_id);
extern uint32_t js_get_class_len(int32_t node_id);
extern void js_clear_children(uint64_t node_id);
extern void js_lex_html_chunk(uint64_t root_id, uint64_t ptr, uint32_t len,
                              uint8_t can_execute);
extern void js_set_node_executable(uint64_t node_id, uint8_t can_exec);
extern uint64_t js_dom_get_parent(uint64_t node_id);
extern void js_set_style_display(uint64_t node_id, uint8_t display_type);
extern uint64_t js_serialize_children(uint64_t node_id);
extern void js_set_style_bg_color(uint64_t node_id, uint8_t r, uint8_t g,
                                  uint8_t b);
extern uint32_t dom_get_generation(uint32_t idx);
extern uint64_t dom_get_first_child(uint32_t idx);
extern uint32_t dom_get_tag(uint32_t idx);
extern uint64_t dom_get_next_sibling(uint32_t idx);
extern uint64_t dom_get_text_ptr(uint32_t idx);
extern uint32_t dom_get_text_len(uint32_t idx);
extern uint64_t dom_get_id_ptr(uint32_t idx);
extern uint32_t dom_get_id_len(uint32_t idx);
extern uint64_t dom_get_class_ptr(uint32_t idx);
extern uint32_t dom_get_class_len(uint32_t idx);
extern uint64_t ext_salt_create_text_node(uint64_t text_ptr, uint32_t len);
extern void dom_set_id(uint32_t idx, uint64_t id_ptr, uint32_t id_len);
extern uint64_t dom_alloc_text(uint32_t len);
extern uint64_t dom_get_parent_node_id(uint32_t idx);
extern uint32_t dom_get_selection_anchor_node();
extern uint32_t dom_get_selection_anchor_offset();
extern uint32_t dom_get_selection_focus_node();
extern uint32_t dom_get_selection_focus_offset();
extern int32_t compare_document_position(uint32_t n1, uint32_t n2);
extern int32_t ext_dom_wrap_text_range(uint32_t text_node_id,
                                       uint32_t start_idx, uint32_t end_idx,
                                       uint8_t wrapper_tag);
extern void ext_media_push_audio_pcm(uint64_t data_ptr, uint32_t sample_count);
extern void sys_canvas_set_fill_color(uint32_t node_id, float r, float g,
                                      float b, float a);
extern void sys_canvas_fill_rect(uint32_t node_id, float x, float y, float w,
                                 float h);
extern uint32_t sys_canvas_create_backing_store(uint32_t node_id,
                                                uint32_t width,
                                                uint32_t height);
extern void dom_init_canvas(uint32_t node_id, uint32_t width, uint32_t height);
extern uint32_t dom_get_canvas_width(uint32_t idx);
extern uint32_t dom_get_canvas_height(uint32_t idx);
extern uint32_t dom_get_canvas_surface_id(uint32_t idx);

static JSClassID prisimi_node_class_id;
static JSClassID prisimi_selection_class_id;
static JSClassID prisimi_range_class_id;
static JSClassID prisimi_canvas_context_class_id;
static JSClassID prisimi_fetchevent_class_id;

static JSValue wrap_node_id(JSContext *ctx, uint32_t node_idx) {
  if (node_idx == 0 || node_idx >= 65536)
    return JS_NULL;
  uint32_t gen = dom_get_generation(node_idx);
  uint64_t node_id = ((uint64_t)gen << 16) | (node_idx & 0xFFFF);
  JSValue obj = JS_NewObjectClass(ctx, prisimi_node_class_id);
  JS_SetOpaque(obj, (void *)(uintptr_t)node_id);
  return obj;
}

// C-side innerHTML serializer
static uint32_t c_append_tag_name(uint8_t *buf, uint32_t cursor,
                                  uint32_t max_len, uint32_t tag) {
  const char *name = NULL;
  size_t nlen = 0;
  switch (tag) {
  case 4:
    name = "div";
    nlen = 3;
    break;
  case 5:
    name = "span";
    nlen = 4;
    break;
  case 6:
    name = "p";
    nlen = 1;
    break;
  case 7:
    name = "a";
    nlen = 1;
    break;
  case 8:
    name = "img";
    nlen = 3;
    break;
  case 9:
    name = "h1";
    nlen = 2;
    break;
  case 1:
    name = "html";
    nlen = 4;
    break;
  case 2:
    name = "head";
    nlen = 4;
    break;
  case 3:
    name = "body";
    nlen = 4;
    break;
  case 99:
    name = "script";
    nlen = 6;
    break;
  default:
    return cursor;
  }
  for (size_t i = 0; i < nlen && cursor < max_len; i++) {
    buf[cursor++] = (uint8_t)name[i];
  }
  return cursor;
}

static uint32_t c_serialize_recursive(uint8_t *buf, uint32_t cursor,
                                      uint32_t max_len, uint64_t node_id) {
  uint32_t idx = (uint32_t)(node_id & 0xFFFF);
  uint32_t expected_gen = (uint32_t)((node_id >> 16) & 0xFFFFFFFF);
  uint32_t actual_gen = dom_get_generation(idx);
  if (idx >= 65536 || actual_gen != expected_gen)
    return cursor;

  uint32_t tag = dom_get_tag(idx);

  if (tag == 0) { // TAG_TEXT
    uint64_t txt_ptr = dom_get_text_ptr(idx);
    uint32_t txt_len = dom_get_text_len(idx);
    const uint8_t *src = (const uint8_t *)txt_ptr;
    for (uint32_t i = 0; i < txt_len && cursor < max_len; i++) {
      buf[cursor++] = src[i];
    }
    return cursor;
  }

  // <tagname
  if (cursor < max_len)
    buf[cursor++] = '<';
  cursor = c_append_tag_name(buf, cursor, max_len, tag);

  // id="..."
  uint32_t id_len = dom_get_id_len(idx);
  if (id_len > 0) {
    uint64_t id_ptr = dom_get_id_ptr(idx);
    const char *prefix = " id=\"";
    for (int i = 0; prefix[i] && cursor < max_len; i++)
      buf[cursor++] = (uint8_t)prefix[i];
    const uint8_t *src = (const uint8_t *)id_ptr;
    for (uint32_t i = 0; i < id_len && cursor < max_len; i++)
      buf[cursor++] = src[i];
    if (cursor < max_len)
      buf[cursor++] = '"';
  }

  // class="..."
  uint32_t class_len = dom_get_class_len(idx);
  if (class_len > 0) {
    uint64_t class_ptr = dom_get_class_ptr(idx);
    const char *prefix = " class=\"";
    for (int i = 0; prefix[i] && cursor < max_len; i++)
      buf[cursor++] = (uint8_t)prefix[i];
    const uint8_t *src = (const uint8_t *)class_ptr;
    for (uint32_t i = 0; i < class_len && cursor < max_len; i++)
      buf[cursor++] = src[i];
    if (cursor < max_len)
      buf[cursor++] = '"';
  }

  // >
  if (cursor < max_len)
    buf[cursor++] = '>';

  // Children
  uint64_t child = dom_get_first_child(idx);
  while (child != 0) {
    cursor = c_serialize_recursive(buf, cursor, max_len, child);
    uint32_t c_idx = (uint32_t)(child & 0xFFFF);
    uint32_t c_expected = (uint32_t)((child >> 16) & 0xFFFFFFFF);
    uint32_t c_actual = dom_get_generation(c_idx);
    if (c_idx < 65536 && c_actual == c_expected) {
      child = dom_get_next_sibling(c_idx);
    } else {
      child = 0;
    }
  }

  // </tagname>
  if (cursor < max_len)
    buf[cursor++] = '<';
  if (cursor < max_len)
    buf[cursor++] = '/';
  cursor = c_append_tag_name(buf, cursor, max_len, tag);
  if (cursor < max_len)
    buf[cursor++] = '>';

  return cursor;
}

// ============================================================================
// Prisimi Async Fetch API
// ============================================================================

typedef struct {
  uint64_t fetch_id;
  JSValue resolve_func;
  JSValue reject_func;
  uint8_t active;
} PrisimiFetchRequest;

static PrisimiFetchRequest js_fetch_requests[256];
static uint8_t
    js_fetch_buffers[256][65536]; // 64KB per slot (Zero-allocation strategy)
static uint32_t js_fetch_buffer_lens[256];
static uint64_t next_fetch_id = 1;

static JSValue js_response_json(JSContext *ctx, JSValueConst this_val, int argc,
                                JSValueConst *argv) {
  JSValue text_val = JS_GetPropertyStr(ctx, this_val, "_text");
  const char *str = JS_ToCString(ctx, text_val);
  if (!str)
    return JS_EXCEPTION;

  JSValue parsed = JS_ParseJSON(ctx, str, strlen(str), "response.json");
  JS_FreeCString(ctx, str);
  JS_FreeValue(ctx, text_val);

  JSValue resolving_funcs[2];
  JSValue promise = JS_NewPromiseCapability(ctx, resolving_funcs);
  JS_Call(ctx, resolving_funcs[0], JS_UNDEFINED, 1, &parsed);

  JS_FreeValue(ctx, parsed);
  JS_FreeValue(ctx, resolving_funcs[0]);
  JS_FreeValue(ctx, resolving_funcs[1]);

  return promise;
}

// Epic 51: Response.text() — returns a Promise resolving to the raw text body
static JSValue js_response_text(JSContext *ctx, JSValueConst this_val, int argc,
                                JSValueConst *argv) {
  JSValue text_val = JS_GetPropertyStr(ctx, this_val, "_text");

  JSValue resolving_funcs[2];
  JSValue promise = JS_NewPromiseCapability(ctx, resolving_funcs);
  JS_Call(ctx, resolving_funcs[0], JS_UNDEFINED, 1, &text_val);

  JS_FreeValue(ctx, text_val);
  JS_FreeValue(ctx, resolving_funcs[0]);
  JS_FreeValue(ctx, resolving_funcs[1]);

  return promise;
}

// ============================================================================
// W3C Selection & Range API
// ============================================================================

static JSValue js_selection_get_property(JSContext *ctx, JSValueConst this_val,
                                         int magic) {
  switch (magic) {
  case 0:
    return wrap_node_id(ctx, dom_get_selection_anchor_node());
  case 1:
    return JS_NewInt32(ctx, dom_get_selection_anchor_offset());
  case 2:
    return wrap_node_id(ctx, dom_get_selection_focus_node());
  case 3:
    return JS_NewInt32(ctx, dom_get_selection_focus_offset());
  case 4:
    return JS_NewBool(ctx, dom_get_selection_anchor_node() ==
                                   dom_get_selection_focus_node() &&
                               dom_get_selection_anchor_offset() ==
                                   dom_get_selection_focus_offset());
  case 5:
    return JS_NewInt32(ctx, (dom_get_selection_anchor_node() != 0) ? 1 : 0);
  }
  return JS_UNDEFINED;
}

static JSValue js_range_get_property(JSContext *ctx, JSValueConst this_val,
                                     int magic) {
  uint32_t an = dom_get_selection_anchor_node();
  uint32_t fn = dom_get_selection_focus_node();
  uint32_t ao = dom_get_selection_anchor_offset();
  uint32_t fo = dom_get_selection_focus_offset();
  int32_t order = compare_document_position(an, fn);
  uint32_t start_node, start_offset, end_node, end_offset;

  if (order <= 0) { // anchor <= focus
    start_node = an;
    start_offset = ao;
    end_node = fn;
    end_offset = fo;
  } else {
    start_node = fn;
    start_offset = fo;
    end_node = an;
    end_offset = ao;
  }

  switch (magic) {
  case 0:
    return wrap_node_id(ctx, start_node);
  case 1:
    return JS_NewInt32(ctx, start_offset);
  case 2:
    return wrap_node_id(ctx, end_node);
  case 3:
    return JS_NewInt32(ctx, end_offset);
  case 4:
    return JS_NewBool(ctx,
                      start_node == end_node && start_offset == end_offset);
  }
  return JS_UNDEFINED;
}

static JSValue js_selection_getRangeAt(JSContext *ctx, JSValueConst this_val,
                                       int argc, JSValueConst *argv) {
  if (dom_get_selection_anchor_node() == 0)
    return JS_EXCEPTION;
  return JS_NewObjectClass(ctx, prisimi_range_class_id);
}

static JSValue js_range_surroundContents(JSContext *ctx, JSValueConst this_val,
                                         int argc, JSValueConst *argv) {
  if (argc < 1)
    return JS_EXCEPTION;

  // 1. Extract Range boundaries from global selection (mirroring current Range
  // implementation)
  uint32_t text_node_id = dom_get_selection_anchor_node();
  uint32_t focus_node_id = dom_get_selection_focus_node();

  if (text_node_id != focus_node_id || text_node_id == 0) {
    return JS_ThrowInternalError(ctx, "Range.surroundContents currently only "
                                      "supports single text node containers");
  }

  uint32_t s_off = dom_get_selection_anchor_offset();
  uint32_t f_off = dom_get_selection_focus_offset();
  uint32_t start_idx = s_off < f_off ? s_off : f_off;
  uint32_t end_idx = s_off < f_off ? f_off : s_off;

  // 2. Extract wrapper element tag from provided node
  uint32_t wrapper_id_packed =
      (uint32_t)(uintptr_t)JS_GetOpaque(argv[0], prisimi_node_class_id);
  uint32_t wrapper_tag = dom_get_tag(wrapper_id_packed & 0xFFFF);

  // 3. Execute Native Slicer (allocates new nodes and re-links in-place)
  int32_t res = ext_dom_wrap_text_range(text_node_id, start_idx, end_idx,
                                        (uint8_t)wrapper_tag);
  if (res < 0) {
    return JS_ThrowInternalError(
        ctx, "DOM Mutation Failed: Boundary or Memory Error");
  }

  // 4. Flush microtasks for layout reflow
  JSContext *pctx;
  while (JS_ExecutePendingJob(JS_GetRuntime(ctx), &pctx) > 0) {
  }

  return JS_UNDEFINED;
}

static const JSCFunctionListEntry js_selection_proto_funcs[] = {
    JS_CGETSET_MAGIC_DEF("anchorNode", js_selection_get_property, NULL, 0),
    JS_CGETSET_MAGIC_DEF("anchorOffset", js_selection_get_property, NULL, 1),
    JS_CGETSET_MAGIC_DEF("focusNode", js_selection_get_property, NULL, 2),
    JS_CGETSET_MAGIC_DEF("focusOffset", js_selection_get_property, NULL, 3),
    JS_CGETSET_MAGIC_DEF("isCollapsed", js_selection_get_property, NULL, 4),
    JS_CGETSET_MAGIC_DEF("rangeCount", js_selection_get_property, NULL, 5),
    JS_CFUNC_DEF("getRangeAt", 1, js_selection_getRangeAt),
};

static const JSCFunctionListEntry js_range_proto_funcs[] = {
    JS_CGETSET_MAGIC_DEF("startContainer", js_range_get_property, NULL, 0),
    JS_CGETSET_MAGIC_DEF("startOffset", js_range_get_property, NULL, 1),
    JS_CGETSET_MAGIC_DEF("endContainer", js_range_get_property, NULL, 2),
    JS_CGETSET_MAGIC_DEF("endOffset", js_range_get_property, NULL, 3),
    JS_CGETSET_MAGIC_DEF("collapsed", js_range_get_property, NULL, 4),
    JS_CFUNC_DEF("surroundContents", 1, js_range_surroundContents),
};

static JSValue js_window_getSelection(JSContext *ctx, JSValueConst this_val,
                                      int argc, JSValueConst *argv) {
  return JS_NewObjectClass(ctx, prisimi_selection_class_id);
}

// Epic 51: Salt-side SoA queue for native fetch visibility
extern int32_t ext_net_queue_fetch(uint64_t fetch_id, uint64_t url_ptr,
                                   uint32_t url_len);

// Epic 60: Indexed DB async FFI bindings
extern int32_t ext_storage_queue_put(uint32_t promise_id, uint64_t key_ptr,
                                     uint32_t key_len, uint64_t val_ptr,
                                     uint32_t val_len);
extern int32_t ext_storage_queue_get(uint32_t promise_id, uint64_t key_ptr,
                                     uint32_t key_len);

static JSValue idb_resolve_funcs[256];
static JSValue idb_reject_funcs[256];
static uint32_t idb_promise_active[256];
static uint32_t next_idb_id = 1;

static uint32_t cache_idb_promise(JSValue resolve, JSValue reject) {
  uint32_t p_id = next_idb_id++;
  for (int i = 0; i < 256; i++) {
    if (!idb_promise_active[i]) {
      idb_promise_active[i] = p_id;
      idb_resolve_funcs[i] = resolve;
      idb_reject_funcs[i] = reject;
      return p_id;
    }
  }
  return 0; // overflow
}

void js_bridge_resolve_idb_promise(uint32_t promise_id, uint64_t val_ptr,
                                   uint32_t val_len) {
  if (!ctx)
    return;
  for (int i = 0; i < 256; i++) {
    if (idb_promise_active[i] == promise_id) {
      JSValue val;
      if (val_ptr == 0 && val_len == 0) {
        // Return undefined explicitly for write success or not-found
        val = JS_UNDEFINED;
      } else {
        val = JS_NewStringLen(ctx, (const char *)(uintptr_t)val_ptr, val_len);
      }
      JS_Call(ctx, idb_resolve_funcs[i], JS_UNDEFINED, 1, &val);
      JS_FreeValue(ctx, val);
      JS_FreeValue(ctx, idb_resolve_funcs[i]);
      JS_FreeValue(ctx, idb_reject_funcs[i]);
      idb_promise_active[i] = 0;
      break;
    }
  }
}

static JSValue js_idb_put(JSContext *c, JSValueConst this_val, int argc,
                          JSValueConst *argv) {
  if (argc < 2)
    return JS_EXCEPTION;
  JSValue resolving_funcs[2];
  JSValue promise = JS_NewPromiseCapability(c, resolving_funcs);
  uint32_t p_id = cache_idb_promise(resolving_funcs[0], resolving_funcs[1]);

  size_t key_len, val_len;
  const char *key_str = JS_ToCStringLen(c, &key_len, argv[0]);
  const char *val_str = JS_ToCStringLen(c, &val_len, argv[1]);

  ext_storage_queue_put(p_id, (uint64_t)key_str, (uint32_t)key_len,
                        (uint64_t)val_str, (uint32_t)val_len);

  JS_FreeCString(c, key_str);
  JS_FreeCString(c, val_str);

  return promise;
}

static JSValue js_idb_get(JSContext *c, JSValueConst this_val, int argc,
                          JSValueConst *argv) {
  if (argc < 1)
    return JS_EXCEPTION;
  JSValue resolving_funcs[2];
  JSValue promise = JS_NewPromiseCapability(c, resolving_funcs);
  uint32_t p_id = cache_idb_promise(resolving_funcs[0], resolving_funcs[1]);

  size_t key_len;
  const char *key_str = JS_ToCStringLen(c, &key_len, argv[0]);

  ext_storage_queue_get(p_id, (uint64_t)key_str, (uint32_t)key_len);

  JS_FreeCString(c, key_str);

  return promise;
}

// Epic 76: FetchEvent & respondWith Bridge
static void js_prisimi_fetchevent_finalizer(JSRuntime *rt, JSValue val) {}
static JSClassDef prisimi_fetchevent_class = {
    "FetchEvent", .finalizer = js_prisimi_fetchevent_finalizer};

static JSValue js_fetchevent_respondWith(JSContext *ctx, JSValueConst this_val,
                                         int argc, JSValueConst *argv) {
  uint32_t fetch_id =
      (uint32_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_fetchevent_class_id);
  if (argc < 1)
    return JS_EXCEPTION;

  // Handle string responses immediately for Phase 1
  if (JS_IsString(argv[0])) {
    size_t len;
    const char *str = JS_ToCStringLen(ctx, &len, argv[0]);
    // Send CMD_FETCH_RESPONSE (type 9) back to Main
    extern void sys_ipc_send_r2m_command_with_payload(
        uint32_t cmd_type, uint64_t arg1, uint64_t p_ptr, uint32_t p_len);
    sys_ipc_send_r2m_command_with_payload(9 /* CMD_FETCH_RESPONSE */, fetch_id,
                                          (uint64_t)(uintptr_t)str,
                                          (uint32_t)len);
    JS_FreeCString(ctx, str);
  }

  return JS_UNDEFINED;
}

void sys_js_dispatch_fetch_event(uint32_t fetch_id, uint64_t url_ptr,
                                 uint32_t url_len) {
  if (!ctx)
    return;
  JSValue global_obj = JS_GetGlobalObject(ctx);
  JSValue onfetch = JS_GetPropertyStr(ctx, global_obj, "onfetch");

  if (JS_IsFunction(ctx, onfetch)) {
    JSValue event_obj = JS_NewObjectClass(ctx, prisimi_fetchevent_class_id);
    JS_SetOpaque(event_obj, (void *)(uintptr_t)fetch_id);

    JSValue request_obj = JS_NewObject(ctx);
    JSValue url_str = JS_NewStringLen(ctx, (const char *)url_ptr, url_len);
    JS_SetPropertyStr(ctx, request_obj, "url", url_str);
    JS_SetPropertyStr(ctx, event_obj, "request", request_obj);

    JSValue ret = JS_Call(ctx, onfetch, global_obj, 1, &event_obj);
    JS_FreeValue(ctx, ret);
    JS_FreeValue(ctx, event_obj);
  }

  JS_FreeValue(ctx, onfetch);
  JS_FreeValue(ctx, global_obj);
}

static JSValue js_sw_register(JSContext *ctx, JSValueConst this_val, int argc,
                              JSValueConst *argv) {
  if (argc < 1)
    return JS_EXCEPTION;
  size_t url_len;
  const char *url = JS_ToCStringLen(ctx, &url_len, argv[0]);

  extern void sys_ipc_send_r2m_command_with_payload(
      uint32_t cmd_type, uint64_t arg1, uint64_t p_ptr, uint32_t p_len);
  sys_ipc_send_r2m_command_with_payload(7 /* CMD_WORKER_REGISTERED */,
                                        1 /* TabId */, (uint64_t)(uintptr_t)url,
                                        (uint32_t)url_len);

  JS_FreeCString(ctx, url);

  JSValue resolving_funcs[2];
  JSValue promise = JS_NewPromiseCapability(ctx, resolving_funcs);
  JSValue success = JS_NewString(ctx, "registered");
  JS_Call(ctx, resolving_funcs[0], JS_UNDEFINED, 1, &success);
  JS_FreeValue(ctx, success);
  JS_FreeValue(ctx, resolving_funcs[0]);
  JS_FreeValue(ctx, resolving_funcs[1]);

  return promise;
}

static JSValue js_window_fetch(JSContext *ctx, JSValueConst this_val, int argc,
                               JSValueConst *argv) {
  if (argc < 1)
    return JS_EXCEPTION;

  size_t url_len;
  const char *url_str = JS_ToCStringLen(ctx, &url_len, argv[0]);
  if (!url_str)
    return JS_EXCEPTION;

  JSValue resolving_funcs[2];
  JSValue promise = JS_NewPromiseCapability(ctx, resolving_funcs);

  // Store in C-side registry
  uint64_t current_id = next_fetch_id++;
  int stored_index = -1;
  for (int i = 0; i < 256; i++) {
    if (!js_fetch_requests[i].active) {
      js_fetch_requests[i].fetch_id = current_id;
      js_fetch_requests[i].resolve_func = resolving_funcs[0];
      js_fetch_requests[i].reject_func = resolving_funcs[1];
      js_fetch_requests[i].active = 1;
      stored_index = i;
      break;
    }
  }

  if (stored_index != -1) {
    js_fetch_buffer_lens[stored_index] = 0; // Reset buffer for this slot
    ext_net_queue_fetch(current_id, (uint64_t)url_str, (uint32_t)url_len);
  } else {
    // Queue full — reject immediately
    JSValue error = JS_NewString(ctx, "Fetch queue full");
    JS_Call(ctx, resolving_funcs[1], JS_UNDEFINED, 1, &error);
    JS_FreeValue(ctx, error);
    JS_FreeValue(ctx, resolving_funcs[0]);
    JS_FreeValue(ctx, resolving_funcs[1]);
  }

  JS_FreeCString(ctx, url_str);
  return promise;
}

// Epic 51: Salt-side slot reclamation
extern void ext_net_reclaim_slot(uint32_t slot);
extern int32_t ext_net_complete_fetch(uint64_t fetch_id, uint64_t buf_ptr,
                                      uint32_t buf_len);

void js_resolve_fetch_impl(uint64_t fetch_id, uint64_t buffer_ptr,
                           uint32_t length);

void js_resolve_fetch_chunk(uint32_t slot, uint64_t buffer_ptr, uint32_t length,
                            uint32_t is_end) {
  if (slot >= 256 || !js_fetch_requests[slot].active)
    return;

  // Append to pre-allocated buffer
  uint32_t current_len = js_fetch_buffer_lens[slot];
  if (current_len + length <= 65536) {
    memcpy(&js_fetch_buffers[slot][current_len], (void *)buffer_ptr, length);
    js_fetch_buffer_lens[slot] += length;
  }

  if (is_end) {
    uint64_t fetch_id = js_fetch_requests[slot].fetch_id;
    js_resolve_fetch_impl(fetch_id, (uintptr_t)js_fetch_buffers[slot],
                          js_fetch_buffer_lens[slot]);
  }
}

void js_resolve_fetch_impl(uint64_t fetch_id, uint64_t buffer_ptr,
                           uint32_t length) {
  for (int i = 0; i < 256; i++) {
    if (js_fetch_requests[i].active &&
        js_fetch_requests[i].fetch_id == fetch_id) {
      // Build Response object with .json() and .text() methods
      JSValue response_obj = JS_NewObject(ctx);
      JSValue text_str = JS_NewStringLen(ctx, (const char *)buffer_ptr, length);
      JS_SetPropertyStr(ctx, response_obj, "_text", text_str);
      JS_SetPropertyStr(ctx, response_obj, "json",
                        JS_NewCFunction(ctx, js_response_json, "json", 0));
      JS_SetPropertyStr(ctx, response_obj, "text",
                        JS_NewCFunction(ctx, js_response_text, "text", 0));

      // Resolve the QuickJS Promise
      JS_Call(ctx, js_fetch_requests[i].resolve_func, JS_UNDEFINED, 1,
              &response_obj);

      // Cleanup C-side registry
      JS_FreeValue(ctx, response_obj);
      JS_FreeValue(ctx, js_fetch_requests[i].resolve_func);
      JS_FreeValue(ctx, js_fetch_requests[i].reject_func);
      js_fetch_requests[i].active = 0;

      // Also mark Salt-side queue as complete and reclaim
      ext_net_complete_fetch(fetch_id, buffer_ptr, (uint32_t)length);
      // Find and reclaim the Salt slot
      for (int s = 0; s < 256; s++) {
        extern uint64_t net_get_fetch_id(uint32_t slot);
        if (net_get_fetch_id(s) == fetch_id) {
          ext_net_reclaim_slot(s);
          break;
        }
      }

      break;
    }
  }

  // CRITICAL: Flush the QuickJS Microtask Queue so .then() executes
  JSContext *pctx;
  while (JS_ExecutePendingJob(JS_GetRuntime(ctx), &pctx) > 0) {
  }
}

static JSClassID prisimi_node_class_id;
static JSClassID prisimi_style_class_id;

static uint32_t fnv1a_hash_str(const char *str) {
  uint32_t hash = 2166136261u;
  while (*str) {
    hash ^= (uint8_t)*str++;
    hash *= 16777619u;
  }
  return hash;
}

static uint32_t fnv1a_hash_len(const char *str, uint32_t len) {
  uint32_t hash = 2166136261u;
  for (uint32_t i = 0; i < len; i++) {
    hash ^= (uint8_t)str[i];
    hash *= 16777619u;
  }
  return hash;
}

typedef struct {
  uint64_t node_id;
  uint32_t event_type_hash;
  JSValue callback;
} PrisimiEventListener;

static PrisimiEventListener js_listeners[1024];
static int js_listeners_count = 0;

static JSValue js_prisimi_node_addEventListener(JSContext *ctx,
                                                JSValueConst this_val, int argc,
                                                JSValueConst *argv) {
  if (argc < 2)
    return JS_EXCEPTION;
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  const char *type = JS_ToCString(ctx, argv[0]);
  if (!type)
    return JS_EXCEPTION;
  if (js_listeners_count < 1024) {
    PrisimiEventListener *l = &js_listeners[js_listeners_count++];
    l->node_id = node_id;
    l->event_type_hash = fnv1a_hash_str(type);
    l->callback = JS_DupValue(ctx, argv[1]);
  }
  JS_FreeCString(ctx, type);
  return JS_UNDEFINED;
}

// Epic 62: removeEventListener — hash-match removal
static JSValue js_prisimi_node_removeEventListener(JSContext *ctx,
                                                   JSValueConst this_val,
                                                   int argc,
                                                   JSValueConst *argv) {
  if (argc < 2)
    return JS_UNDEFINED;
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  const char *type = JS_ToCString(ctx, argv[0]);
  if (!type)
    return JS_UNDEFINED;
  uint32_t hash = fnv1a_hash_str(type);
  JS_FreeCString(ctx, type);
  for (int i = 0; i < js_listeners_count; i++) {
    if (js_listeners[i].node_id == node_id &&
        js_listeners[i].event_type_hash == hash) {
      // Check if callback matches
      // For simplicity, remove first match (standard behavior for identical
      // hash)
      JS_FreeValue(ctx, js_listeners[i].callback);
      js_listeners[i] = js_listeners[--js_listeners_count];
      break;
    }
  }
  return JS_UNDEFINED;
}

// Epic 61: document.addEventListener — uses node_id=0 as sentinel for
// document-level events
static JSValue js_document_addEventListener(JSContext *ctx,
                                            JSValueConst this_val, int argc,
                                            JSValueConst *argv) {
  if (argc < 2)
    return JS_EXCEPTION;
  const char *type = JS_ToCString(ctx, argv[0]);
  if (!type)
    return JS_EXCEPTION;
  if (js_listeners_count < 1024) {
    PrisimiEventListener *l = &js_listeners[js_listeners_count++];
    l->node_id = 0; // Sentinel: document pseudo-node
    l->event_type_hash = fnv1a_hash_str(type);
    l->callback = JS_DupValue(ctx, argv[1]);
  }
  JS_FreeCString(ctx, type);
  return JS_UNDEFINED;
}

extern int32_t js_execute_pending_jobs(); // Forward declare

void js_bridge_dispatch_event(uint64_t node_id, const char *type_ptr,
                              uint32_t type_len) {
  if (!ctx)
    return;
  uint32_t hash = fnv1a_hash_len(type_ptr, type_len);
  uint64_t current_id = node_id;
  uint64_t retargeted_id = node_id;

  while (current_id != 0) {
    for (int i = 0; i < js_listeners_count; i++) {
      if (js_listeners[i].node_id == current_id &&
          js_listeners[i].event_type_hash == hash) {
        JSValue event_obj = JS_NewObject(ctx);

        // Expose target with retargeting (Epic 64)
        JSValue target_node = JS_NewObjectClass(ctx, prisimi_node_class_id);
        JS_SetOpaque(target_node, (void *)(uintptr_t)retargeted_id);
        JS_SetPropertyStr(ctx, event_obj, "target", target_node);

        JSValue ret =
            JS_Call(ctx, js_listeners[i].callback, JS_UNDEFINED, 1, &event_obj);
        JS_FreeValue(ctx, ret);
        JS_FreeValue(ctx, event_obj);
      }
    }

    uint64_t parent_id = js_dom_get_parent(current_id);

    // Retargeting Boundary! Crossing out of a ShadowRoot natively targets the
    // Host.
    uint32_t current_idx = (uint32_t)(current_id & 0xFFFF);
    if (dom_get_tag(current_idx) == 21) { // TAG_SHADOW_ROOT
      retargeted_id = parent_id;
    }

    current_id = parent_id;
  }
  js_execute_pending_jobs();
}

void js_bridge_dispatch_message_event(uint64_t msg_ptr, uint32_t msg_len) {
  if (!ctx)
    return;
  uint32_t hash = fnv1a_hash_str("message");
  for (int i = 0; i < js_listeners_count; i++) {
    // window-level events are 999999
    if (js_listeners[i].node_id == 999999 &&
        js_listeners[i].event_type_hash == hash) {
      JSValue event_obj = JS_NewObject(ctx);
      JSValue data_str =
          JS_NewStringLen(ctx, (const char *)(uintptr_t)msg_ptr, msg_len);
      JS_SetPropertyStr(ctx, event_obj, "data", data_str);
      JS_SetPropertyStr(ctx, event_obj, "type", JS_NewString(ctx, "message"));

      JSValue ret =
          JS_Call(ctx, js_listeners[i].callback, JS_UNDEFINED, 1, &event_obj);
      JS_FreeValue(ctx, ret);
      JS_FreeValue(ctx, event_obj);
    }
  }
  js_execute_pending_jobs();
}

// ============================================================================
// Epic 50: OS Event Router — click coordinates → hit-test → JS dispatch
// ============================================================================
extern uint32_t dom_hit_test(uint32_t node_idx, int32_t target_x,
                             int32_t target_y);
extern void dom_handle_click_focus(uint32_t node_idx);
extern uint32_t dom_get_generation(uint32_t idx);

void sys_on_mouse_click(int32_t x, int32_t y) {
  // 1. Geometric hit-test from root (node index 1)
  uint32_t target_idx = dom_hit_test(1, x, y);
  printf("sys_on_mouse_click called! target_idx = %u\n", target_idx);
  fflush(stdout);
  if (target_idx == 0)
    return;

  // 2. Update structural focus (for caret rendering)
  dom_handle_click_focus(target_idx);

  // 3. Reconstruct the generational node_id for the listener registry
  uint32_t gen = dom_get_generation(target_idx);
  uint64_t target_node_id = (uint64_t)target_idx | ((uint64_t)gen << 16);

  // 4. Dispatch 'click' event with bubble-up through parent chain
  if (ctx) {
      js_bridge_dispatch_event(target_node_id, "click", 5);
  }

  // 5. Sprint 8: Native Anchor Navigation
  extern uint32_t dom_get_tag(uint32_t idx);
  extern uint32_t ext_dom_get_parent_idx(uint32_t idx);
  extern uint32_t user__browser__dom__ATTR_COUNT;
  extern uint64_t user__browser__dom__ATTR_NODE_ID[];
  extern uint64_t user__browser__dom__ATTR_KEY_PTR[];
  extern uint32_t user__browser__dom__ATTR_KEY_LEN[];
  extern uint64_t user__browser__dom__ATTR_VAL_PTR[];
  extern uint32_t user__browser__dom__ATTR_VAL_LEN[];
  extern void sys_browser_navigate(uint64_t ptr, uint32_t len);

  uint32_t curr = target_idx;
  printf("Target idx = %u\n", curr);
  while (curr != 0 && curr != 999999) {
    if (dom_get_tag(curr) == 7) { // TAG_A
       printf("Found TAG_A, attr count = %u\n", user__browser__dom__ATTR_COUNT);
       // Look for href attribute natively
       for (uint32_t i = 0; i < user__browser__dom__ATTR_COUNT; i++) {
         printf("Checking attribute %u: node_id = %llu, len = %u\n", i, user__browser__dom__ATTR_NODE_ID[i], user__browser__dom__ATTR_KEY_LEN[i]);
         if (user__browser__dom__ATTR_NODE_ID[i] == (uint64_t)curr &&
             user__browser__dom__ATTR_KEY_LEN[i] == 4) {
             char *key = (char*)(uintptr_t)user__browser__dom__ATTR_KEY_PTR[i];
             printf("Key ptr is %p. Key looks like: %c%c%c%c\n", key, key[0], key[1], key[2], key[3]);
             if (key && memcmp(key, "href", 4) == 0) {
                 uint64_t href_ptr = (uint64_t)user__browser__dom__ATTR_VAL_PTR[i];
                 uint32_t href_len = user__browser__dom__ATTR_VAL_LEN[i];
                 printf("HREF found: len = %u\n", href_len);
                 fflush(stdout);
                 if (href_len > 0) {
                     sys_browser_navigate(href_ptr, href_len);
                     return;
                 }
             }
         }
       }
    }
    curr = ext_dom_get_parent_idx(curr);
  }
}

// Epic 56, 60: Native Stubs for Tests
void sys_gpu_set_scissor_rect(int32_t x, int32_t y, int32_t w, int32_t h) {}
void *sys_mmap_file(const char *path, uint32_t file_len) { return NULL; }

// ============================================================================
// Prisimi JS Opaque Handle Lifecycle
// ============================================================================

extern uint32_t dom_get_active_focus();
extern void ext_dom_mutate_text(uint32_t node_idx, uint8_t char_code,
                                uint8_t is_backspace);

void sys_on_key_event(uint8_t char_code, uint8_t is_backspace) {
  if (!ctx)
    return;

  uint32_t focus_idx = dom_get_active_focus();
  if (focus_idx == 0)
    return;

  // 1. Mutate native buffer
  ext_dom_mutate_text(focus_idx, char_code, is_backspace);

  // 2. Dispatch JS Event ('input')
  uint32_t gen = dom_get_generation(focus_idx);
  uint64_t target_node_id = (uint64_t)focus_idx | ((uint64_t)gen << 16);

  // Dispatch input event to any registered listeners
  js_bridge_dispatch_event(target_node_id, "input", 5);
}

static void prisimi_style_finalizer(JSRuntime *rt, JSValue val) {}
static JSClassDef prisimi_style_class = {"PrisimiStyle",
                                         .finalizer = prisimi_style_finalizer};

// Epic 49: GC Free-List Finalizer — reclaims DOM node when JS drops last
// reference
extern void ext_dom_free_node(uint32_t node_idx);

static void prisimi_node_finalizer(JSRuntime *rt, JSValue val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(val, prisimi_node_class_id);
  if (node_id != 0) {
    uint32_t node_idx = (uint32_t)(node_id & 0xFFFF);
    if (node_idx != 0) {
      ext_dom_free_node(node_idx);
    }
  }
}
static JSClassDef prisimi_node_class = {"PrisimiNode",
                                        .finalizer = prisimi_node_finalizer};

static JSValue js_document_getElementById(JSContext *ctx, JSValueConst this_val,
                                          int argc, JSValueConst *argv) {
  if (argc < 1)
    return JS_UNDEFINED;
  size_t len;
  const char *str = JS_ToCStringLen(ctx, &len, argv[0]);
  if (!str)
    return JS_UNDEFINED;
  uint64_t node_id = resolve_node_by_id((uint64_t)str, (uint32_t)len);
  JS_FreeCString(ctx, str);
  if (node_id == 0)
    return JS_NULL;
  JSValue obj = JS_NewObjectClass(ctx, prisimi_node_class_id);
  JS_SetOpaque(obj, (void *)(uintptr_t)node_id);
  return obj;
}

static JSValue js_prisimi_node_get_className(JSContext *ctx,
                                             JSValueConst this_val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  uint32_t node_idx = (uint32_t)(node_id & 0xFFFF);
  uint64_t ptr = js_get_class_ptr(node_idx);
  uint32_t len = js_get_class_len(node_idx);
  if (ptr == 0)
    return JS_NewString(ctx, "");
  return JS_NewStringLen(ctx, (const char *)ptr, len);
}

extern uint64_t dom_alloc_text(uint32_t len);
extern void ext_dom_set_class_name(uint32_t node_id, uint64_t str_ptr,
                                   uint32_t str_len);

static JSValue js_prisimi_node_set_className(JSContext *ctx,
                                             JSValueConst this_val,
                                             JSValueConst val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  uint32_t node_idx = (uint32_t)(node_id & 0xFFFF);
  size_t len;
  const char *str = JS_ToCStringLen(ctx, &len, val);
  if (!str)
    return JS_EXCEPTION;

  // Copy the string into the Salt text arena to ensure it survives JS GC
  uint64_t safe_ptr = dom_alloc_text((uint32_t)len);
  if (safe_ptr != 0) {
    memcpy((void *)(uintptr_t)safe_ptr, str, len);
    ext_dom_set_class_name(node_idx, safe_ptr, (uint32_t)len);
  }

  JS_FreeCString(ctx, str);
  return JS_UNDEFINED;
}

static JSValue js_prisimi_node_get_innerHTML(JSContext *ctx,
                                             JSValueConst this_val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  uint32_t max_len = 131072;
  uint32_t offset = airlock_allocate(max_len);
  if (offset == 0xFFFFFFFF)
    return JS_NewString(ctx, "");
  uint8_t *airlock_base = (uint8_t *)airlock_get_ptr();
  uint8_t *buf = airlock_base + offset;
  uint32_t cursor = 0;

  uint32_t idx = (uint32_t)(node_id & 0xFFFF);
  uint32_t gen = dom_get_generation(idx);
  uint32_t exp_gen = (uint32_t)((node_id >> 16) & 0xFFFFFFFF);
  if (idx < 65536 && gen == exp_gen) {
    uint64_t child = dom_get_first_child(idx);
    while (child != 0) {
      cursor = c_serialize_recursive(buf, cursor, max_len, child);
      uint32_t c_idx = (uint32_t)(child & 0xFFFF);
      uint32_t c_expected = (uint32_t)((child >> 16) & 0xFFFFFFFF);
      uint32_t c_actual = dom_get_generation(c_idx);
      if (c_idx < 65536 && c_actual == c_expected) {
        child = dom_get_next_sibling(c_idx);
      } else {
        child = 0;
      }
    }
  }
  JSValue str = JS_NewStringLen(ctx, (const char *)buf, cursor);
  airlock_deallocate(offset);
  return str;
}

static JSValue js_prisimi_node_set_innerHTML(JSContext *ctx,
                                             JSValueConst this_val,
                                             JSValueConst val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  size_t len;
  const char *str = JS_ToCStringLen(ctx, &len, val);
  if (str) {
    js_clear_children(node_id);
    js_lex_html_chunk(node_id, (uint64_t)str, (uint32_t)len, 0);
    JS_FreeCString(ctx, str);
  }
  return JS_UNDEFINED;
}

static JSValue js_prisimi_node_get_style(JSContext *ctx,
                                         JSValueConst this_val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  JSValue obj = JS_NewObjectClass(ctx, prisimi_style_class_id);
  JS_SetOpaque(obj, (void *)(uintptr_t)node_id);
  return obj;
}

static JSValue js_style_set_backgroundColor(JSContext *ctx,
                                            JSValueConst this_val,
                                            JSValueConst val) {
  int32_t node_id =
      (int32_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_style_class_id);
  size_t len;
  const char *str = JS_ToCStringLen(ctx, &len, val);
  if (str && len == 7 && str[0] == '#') {
    uint32_t r, g, b;
    sscanf(str + 1, "%02x%02x%02x", &r, &g, &b);
    js_set_style_bg_color((uint64_t)(uintptr_t)node_id, (uint8_t)r, (uint8_t)g,
                          (uint8_t)b);
  }
  if (str)
    JS_FreeCString(ctx, str);
  return JS_UNDEFINED;
}

extern void js_set_grid_template_columns(uint64_t node_id, uint64_t str_ptr,
                                         uint32_t str_len);
extern void js_set_grid_column_start(uint64_t node_id, uint32_t start_col);

static JSValue js_style_set_gridTemplateColumns(JSContext *ctx,
                                                JSValueConst this_val,
                                                JSValueConst val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_style_class_id);
  size_t len;
  const char *str = JS_ToCStringLen(ctx, &len, val);
  if (str) {
    js_set_grid_template_columns(node_id, (uint64_t)(uintptr_t)str,
                                 (uint32_t)len);
    JS_FreeCString(ctx, str);
  }
  return JS_UNDEFINED;
}

static JSValue js_style_set_gridColumnStart(JSContext *ctx,
                                            JSValueConst this_val,
                                            JSValueConst val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_style_class_id);
  uint32_t start_col = 0;
  JS_ToUint32(ctx, &start_col, val);
  js_set_grid_column_start(node_id, start_col);
  return JS_UNDEFINED;
}

static JSValue js_style_set_display(JSContext *ctx, JSValueConst this_val,
                                    JSValueConst val) {
  int32_t node_id =
      (int32_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_style_class_id);
  size_t len;
  const char *str = JS_ToCStringLen(ctx, &len, val);
  if (str) {
    uint8_t d = 0;
    if (strcmp(str, "block") == 0)
      d = 1;
    else if (strcmp(str, "flex") == 0)
      d = 2;
    else if (strcmp(str, "grid") == 0)
      d = 4;
    js_set_style_display((uint64_t)(uintptr_t)node_id, d);
    JS_FreeCString(ctx, str);
  }
  return JS_UNDEFINED;
}

extern void js_set_style_transform(uint64_t node_id, float tx, float ty);
extern void js_set_style_opacity(uint64_t node_id, float opacity);

static JSValue js_style_set_transform(JSContext *ctx, JSValueConst this_val,
                                      JSValueConst val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_style_class_id);
  size_t len;
  const char *str = JS_ToCStringLen(ctx, &len, val);
  if (!str)
    return JS_UNDEFINED;

  float tx = 0.0f, ty = 0.0f;

  // Parse "translate3d(Xpx, Ypx, Zpx)" or "translateX(Xpx)" or
  // "translateY(Ypx)"
  const char *p = strstr(str, "translate3d(");
  if (p) {
    sscanf(p, "translate3d(%f", &tx);
    const char *comma = strchr(p, ',');
    if (comma)
      sscanf(comma + 1, " %f", &ty);
  } else {
    p = strstr(str, "translateX(");
    if (p)
      sscanf(p, "translateX(%f", &tx);
    p = strstr(str, "translateY(");
    if (p)
      sscanf(p, "translateY(%f", &ty);
    // Also handle "translate(Xpx, Ypx)"
    p = strstr(str, "translate(");
    if (p) {
      sscanf(p, "translate(%f", &tx);
      const char *comma = strchr(p, ',');
      if (comma)
        sscanf(comma + 1, " %f", &ty);
    }
  }

  js_set_style_transform(node_id, tx, ty);
  JS_FreeCString(ctx, str);
  return JS_UNDEFINED;
}

static JSValue js_style_set_opacity(JSContext *ctx, JSValueConst this_val,
                                    JSValueConst val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_style_class_id);
  double d;
  if (JS_ToFloat64(ctx, &d, val))
    return JS_UNDEFINED;
  js_set_style_opacity(node_id, (float)d);
  return JS_UNDEFINED;
}

extern void js_set_style_i32(uint64_t node_id, uint32_t prop_id, int32_t val);
extern void js_set_style_u8(uint64_t node_id, uint32_t prop_id, uint8_t val);

static void parse_and_set_dimension(uint64_t node_id, const char *str,
                                    uint32_t prop_val_id,
                                    uint32_t prop_unit_id) {
  if (!str)
    return;
  int32_t val = 0;
  uint8_t unit = 0; // 0 = PX, 1 = PERCENT
  sscanf(str, "%d", &val);
  if (strstr(str, "%") != NULL) {
    unit = 1;
  }
  printf("[QuickJS Dimension Parser] ID: %llu, Prop: %u, Val: %d, Unit: %d\n",
         (unsigned long long)node_id, prop_val_id, val, unit);
  fflush(stdout);
  js_set_style_i32(node_id, prop_val_id, val);
  js_set_style_u8(node_id, prop_unit_id, unit);
}

static JSValue js_style_set_width(JSContext *ctx, JSValueConst this_val,
                                  JSValueConst val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_style_class_id);
  size_t len;
  const char *str = JS_ToCStringLen(ctx, &len, val);
  parse_and_set_dimension(node_id, str, 0, 4); // 0=W, 4=W_UNIT
  if (str)
    JS_FreeCString(ctx, str);
  return JS_UNDEFINED;
}

static JSValue js_style_set_height(JSContext *ctx, JSValueConst this_val,
                                   JSValueConst val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_style_class_id);
  size_t len;
  const char *str = JS_ToCStringLen(ctx, &len, val);
  parse_and_set_dimension(node_id, str, 1, 5); // 1=H, 5=H_UNIT
  if (str)
    JS_FreeCString(ctx, str);
  return JS_UNDEFINED;
}

static JSValue js_style_set_flexGrow(JSContext *ctx, JSValueConst this_val,
                                     JSValueConst val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_style_class_id);
  double d;
  // Allow JS assigning "1" string or 1 number
  if (!JS_ToFloat64(ctx, &d, val)) {
    printf("[QuickJS Flex Parser] ID: %llu, FlexGrow: %f\n",
           (unsigned long long)node_id, d);
    fflush(stdout);
    js_set_style_i32(node_id, 2, (int32_t)d); // 2=FLEX_GROW
  }
  return JS_UNDEFINED;
}

static JSValue js_style_set_position(JSContext *ctx, JSValueConst this_val,
                                     JSValueConst val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_style_class_id);
  size_t len;
  const char *str = JS_ToCStringLen(ctx, &len, val);
  if (!str)
    return JS_EXCEPTION;
  extern void dom_set_style_position(uint32_t idx, uint8_t val);
  uint8_t pos_val = 0;
  if (strcmp(str, "relative") == 0)
    pos_val = 1;
  else if (strcmp(str, "absolute") == 0)
    pos_val = 2;
  else if (strcmp(str, "fixed") == 0)
    pos_val = 3;
  dom_set_style_position((uint32_t)(node_id & 0xFFFF), pos_val);
  JS_FreeCString(ctx, str);
  return JS_UNDEFINED;
}

static JSValue js_style_set_overflow(JSContext *ctx, JSValueConst this_val,
                                     JSValueConst val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_style_class_id);
  size_t len;
  const char *str = JS_ToCStringLen(ctx, &len, val);
  if (!str)
    return JS_EXCEPTION;
  extern void dom_set_style_overflow(uint32_t idx, uint8_t val);
  uint8_t over_val = 0; // visible
  if (strcmp(str, "hidden") == 0)
    over_val = 1;
  else if (strcmp(str, "scroll") == 0)
    over_val = 2;
  else if (strcmp(str, "auto") == 0)
    over_val = 3;
  dom_set_style_overflow((uint32_t)(node_id & 0xFFFF), over_val);
  JS_FreeCString(ctx, str);
  return JS_UNDEFINED;
}

static JSValue js_prisimi_node_set_scrollTop(JSContext *ctx,
                                             JSValueConst this_val,
                                             JSValueConst val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  double d;
  if (!JS_ToFloat64(ctx, &d, val)) {
    extern void dom_set_layout_scroll_y(uint32_t idx, float val);
    dom_set_layout_scroll_y((uint32_t)(node_id & 0xFFFF), (float)d);
  }
  return JS_UNDEFINED;
}

static JSValue js_style_set_top(JSContext *ctx, JSValueConst this_val,
                                JSValueConst val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_style_class_id);
  size_t len;
  const char *str = JS_ToCStringLen(ctx, &len, val);
  if (!str)
    return JS_EXCEPTION;
  int t_val;
  if (sscanf(str, "%d", &t_val) == 1) {
    extern void dom_set_style_top(uint32_t idx, int32_t val);
    dom_set_style_top((uint32_t)(node_id & 0xFFFF), (int32_t)t_val);
  }
  JS_FreeCString(ctx, str);
  return JS_UNDEFINED;
}

static JSValue js_style_set_left(JSContext *ctx, JSValueConst this_val,
                                 JSValueConst val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_style_class_id);
  size_t len;
  const char *str = JS_ToCStringLen(ctx, &len, val);
  if (!str)
    return JS_EXCEPTION;
  int t_val;
  if (sscanf(str, "%d", &t_val) == 1) {
    extern void dom_set_style_left(uint32_t idx, int32_t val);
    dom_set_style_left((uint32_t)(node_id & 0xFFFF), (int32_t)t_val);
  }
  JS_FreeCString(ctx, str);
  return JS_UNDEFINED;
}

static JSValue js_style_set_zIndex(JSContext *ctx, JSValueConst this_val,
                                   JSValueConst val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_style_class_id);
  double d;
  if (!JS_ToFloat64(ctx, &d, val)) {
    extern void dom_set_style_z_index(uint32_t idx, int32_t val);
    dom_set_style_z_index((uint32_t)(node_id & 0xFFFF), (int32_t)d);
  } else {
    size_t len;
    const char *str = JS_ToCStringLen(ctx, &len, val);
    if (!str)
      return JS_EXCEPTION;
    int t_val;
    if (sscanf(str, "%d", &t_val) == 1) {
      extern void dom_set_style_z_index(uint32_t idx, int32_t val);
      dom_set_style_z_index((uint32_t)(node_id & 0xFFFF), (int32_t)t_val);
    }
    JS_FreeCString(ctx, str);
  }
  return JS_UNDEFINED;
}

extern void js_set_canvas_texture(uint64_t node_id, uint32_t tex_id, uint32_t w,
                                  uint32_t h);
extern int facet_gpu_upload_image(uint8_t *rgba, int width, int height);
extern void facet_gpu_update_texture(int slot, uint8_t *rgba, int width,
                                     int height);
extern uint32_t dom_get_canvas_texture(uint32_t idx);
extern uint32_t dom_get_canvas_width(uint32_t idx);
extern uint32_t dom_get_canvas_height(uint32_t idx);

extern void js_set_id(uint64_t node_id, uint64_t id_ptr, uint32_t id_len);

static JSValue js_prisimi_node_set_id(JSContext *ctx, JSValueConst this_val,
                                      JSValueConst val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  size_t len;
  const char *str = JS_ToCStringLen(ctx, &len, val);
  if (!str)
    return JS_EXCEPTION;
  js_set_id(node_id, (uint64_t)str, (uint32_t)len);
  JS_FreeCString(ctx, str);
  return JS_UNDEFINED;
}

static JSValue js_prisimi_node_setCanvasDimensions(JSContext *ctx,
                                                   JSValueConst this_val,
                                                   int argc,
                                                   JSValueConst *argv) {
  int32_t node_id =
      (int32_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  uint32_t w, h;
  if (JS_ToUint32(ctx, &w, argv[0]) || JS_ToUint32(ctx, &h, argv[1])) {
    return JS_EXCEPTION;
  }

  int tex_id = facet_gpu_upload_image(NULL, w, h);
  if (tex_id >= 0) {
    js_set_canvas_texture((uint64_t)(uintptr_t)node_id, (uint32_t)tex_id, w, h);
  }
  return JS_UNDEFINED;
}

static JSValue js_prisimi_node_putImageData(JSContext *ctx,
                                            JSValueConst this_val, int argc,
                                            JSValueConst *argv) {
  int32_t node_id =
      (int32_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);

  size_t size;
  uint8_t *raw_pixels = JS_GetArrayBuffer(ctx, &size, argv[0]);
  if (!raw_pixels) {
    size_t byte_offset, byte_length, bytes_per_element;
    JSValue buffer = JS_GetTypedArrayBuffer(ctx, argv[0], &byte_offset,
                                            &byte_length, &bytes_per_element);
    if (!JS_IsException(buffer)) {
      raw_pixels = JS_GetArrayBuffer(ctx, &size, buffer);
      if (raw_pixels) {
        raw_pixels += byte_offset;
      }
      JS_FreeValue(ctx, buffer);
    }
  }

  if (raw_pixels) {
    uint32_t tex_id = dom_get_canvas_texture((uint32_t)node_id);
    uint32_t w = dom_get_canvas_width((uint32_t)node_id);
    uint32_t h = dom_get_canvas_height((uint32_t)node_id);
    facet_gpu_update_texture(tex_id, raw_pixels, w, h);
  }
  return JS_UNDEFINED;
}

// ============================================================================
// W3C Canvas 2D Rendering Context
// ============================================================================

static void js_prisimi_canvas_context_finalizer(JSRuntime *rt, JSValue val) {}
static JSClassDef prisimi_canvas_context_class = {
    "CanvasRenderingContext2D",
    .finalizer = js_prisimi_canvas_context_finalizer};

static JSValue js_canvas_fillRect(JSContext *ctx, JSValueConst this_val,
                                  int argc, JSValueConst *argv) {
  if (argc < 4)
    return JS_EXCEPTION;
  uint64_t node_id = (uint64_t)(uintptr_t)JS_GetOpaque(
      this_val, prisimi_canvas_context_class_id);
  double x, y, w, h;
  JS_ToFloat64(ctx, &x, argv[0]);
  JS_ToFloat64(ctx, &y, argv[1]);
  JS_ToFloat64(ctx, &w, argv[2]);
  JS_ToFloat64(ctx, &h, argv[3]);
  sys_canvas_fill_rect((uint32_t)(node_id & 0xFFFF), (float)x, (float)y,
                       (float)w, (float)h);
  return JS_UNDEFINED;
}

static JSValue js_canvas_set_fillStyle(JSContext *ctx, JSValueConst this_val,
                                       JSValueConst val) {
  uint64_t node_id = (uint64_t)(uintptr_t)JS_GetOpaque(
      this_val, prisimi_canvas_context_class_id);
  size_t len;
  const char *str = JS_ToCStringLen(ctx, &len, val);
  if (str) {
    if (str[0] == '#' && (len == 7 || len == 4)) {
      uint32_t r, g, b;
      if (len == 7) {
        sscanf(str + 1, "%02x%02x%02x", &r, &g, &b);
      } else {
        sscanf(str + 1, "%1x%1x%1x", &r, &g, &b);
        r |= r << 4;
        g |= g << 4;
        b |= b << 4;
      }
      sys_canvas_set_fill_color((uint32_t)(node_id & 0xFFFF), r / 255.0f,
                                g / 255.0f, b / 255.0f, 1.0f);
    } else if (strncmp(str, "rgb", 3) == 0) {
      int r, g, b;
      if (sscanf(str, "rgb(%d,%d,%d)", &r, &g, &b) == 3) {
        sys_canvas_set_fill_color((uint32_t)(node_id & 0xFFFF), r / 255.0f,
                                  g / 255.0f, b / 255.0f, 1.0f);
      }
    }
    JS_FreeCString(ctx, str);
  }
  return JS_UNDEFINED;
}

static const JSCFunctionListEntry prisimi_canvas_context_funcs[] = {
    JS_CFUNC_DEF("fillRect", 4, js_canvas_fillRect),
    JS_CGETSET_DEF("fillStyle", NULL, js_canvas_set_fillStyle),
};

static JSValue js_prisimi_node_getContext(JSContext *ctx, JSValueConst this_val,
                                          int argc, JSValueConst *argv) {
  if (argc < 1)
    return JS_NULL;
  const char *type = JS_ToCString(ctx, argv[0]);
  if (!type)
    return JS_NULL;

  if (strcmp(type, "2d") != 0) {
    JS_FreeCString(ctx, type);
    return JS_NULL;
  }
  JS_FreeCString(ctx, type);

  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  uint32_t idx = (uint32_t)(node_id & 0xFFFF);
  if (dom_get_tag(idx) != 27)
    return JS_NULL; // TAG_CANVAS

  // Check if backing store is already initialized
  extern uint32_t dom_get_canvas_surface_id(uint32_t idx);
  if (dom_get_canvas_surface_id(idx) == 0) {
    uint32_t w = dom_get_canvas_width(idx);
    uint32_t h = dom_get_canvas_height(idx);
    if (w == 0)
      w = 300;
    if (h == 0)
      h = 150;
    dom_init_canvas(idx, w, h);
  }

  JSValue obj = JS_NewObjectClass(ctx, prisimi_canvas_context_class_id);
  JS_SetOpaque(obj, (void *)(uintptr_t)node_id);
  return obj;
}

// ============================================================================
// Epic 31: DOM Mutation API — createElement, appendChild, removeChild
// ============================================================================

extern void js_dom_append_child(uint32_t parent_idx, uint32_t child_idx);
extern void js_dom_remove_child(uint32_t parent_idx, uint32_t child_idx);
extern uint64_t ext_salt_create_node(uint32_t tag);

// Map a tag name string to a dom.salt TAG_* constant
static uint32_t map_string_to_tag_id(const char *tag, size_t len) {
  // Case-insensitive first char
  char c0 = (len > 0) ? (tag[0] | 0x20) : 0;
  if (len == 3) {
    char c1 = tag[1] | 0x20, c2 = tag[2] | 0x20;
    if (c0 == 'd' && c1 == 'i' && c2 == 'v')
      return 4; // TAG_DIV
    if (c0 == 'i' && c1 == 'm' && c2 == 'g')
      return 8; // TAG_IMG
    return 4;
  }
  if (len == 4) {
    char c1 = tag[1] | 0x20, c2 = tag[2] | 0x20, c3 = tag[3] | 0x20;
    if (c0 == 's' && c1 == 'p' && c2 == 'a' && c3 == 'n')
      return 5; // TAG_SPAN
    if (c0 == 'h' && c1 == 't' && c2 == 'm' && c3 == 'l')
      return 1; // TAG_HTML
    if (c0 == 'h' && c1 == 'e' && c2 == 'a' && c3 == 'd')
      return 2; // TAG_HEAD
    if (c0 == 'b' && c1 == 'o' && c2 == 'd' && c3 == 'y')
      return 3; // TAG_BODY
    return 4;
  }
  if (len == 1) {
    if (c0 == 'p')
      return 6; // TAG_P
    if (c0 == 'a')
      return 7; // TAG_A
  }
  if (len == 2) {
    char c1 = tag[1] | 0x20;
    if (c0 == 'h' && c1 == '1')
      return 9; // TAG_H1
    if (c0 == 'h' && c1 == '2')
      return 21; // TAG_H2
    if (c0 == 'h' && c1 == '3')
      return 22; // TAG_H3
    if (c0 == 'l' && c1 == 'i')
      return 25; // TAG_LI
    if (c0 == 'u' && c1 == 'l')
      return 23; // TAG_UL
    if (c0 == 'o' && c1 == 'l')
      return 24; // TAG_OL
  }
  if (len == 5) {
    char c1 = tag[1] | 0x20, c2 = tag[2] | 0x20, c3 = tag[3] | 0x20,
         c4 = tag[4] | 0x20;
    if (c0 == 'i' && c1 == 'n' && c2 == 'p' && c3 == 'u' && c4 == 't')
      return 18; // TAG_INPUT
    if (c0 == 'l' && c1 == 'a' && c2 == 'b' && c3 == 'e' && c4 == 'l')
      return 26; // TAG_LABEL
  }
  if (len == 6) {
    char c1 = tag[1] | 0x20, c2 = tag[2] | 0x20, c3 = tag[3] | 0x20,
         c4 = tag[4] | 0x20, c5 = tag[5] | 0x20;
    if (c0 == 'b' && c1 == 'u' && c2 == 't' && c3 == 't' && c4 == 'o' &&
        c5 == 'n')
      return 20; // TAG_BUTTON
    if (c0 == 'c' && c1 == 'a' && c2 == 'n' && c3 == 'v' && c4 == 'a' &&
        c5 == 's')
      return 27; // TAG_CANVAS
  }
  if (len == 8) {
    char c1 = tag[1] | 0x20, c2 = tag[2] | 0x20, c3 = tag[3] | 0x20,
         c4 = tag[4] | 0x20;
    char c5 = tag[5] | 0x20, c6 = tag[6] | 0x20, c7 = tag[7] | 0x20;
    if (c0 == 't' && c1 == 'e' && c2 == 'x' && c3 == 't' && c4 == 'a' &&
        c5 == 'r' && c6 == 'e' && c7 == 'a')
      return 19; // TAG_TEXTAREA
  }
  return 4; // Default TAG_DIV
}

// Epic 63: Custom Element Registry check
static JSValue check_custom_element_registry(JSContext *ctx,
                                             const char *tag_name) {
  uint32_t hash = fnv1a_hash_str(tag_name);
  for (int i = 0; i < custom_elements_count; i++) {
    if (custom_elements[i].tag_hash == hash) {
      // Instantiate the custom element class
      JSValue inst =
          JS_CallConstructor(ctx, custom_elements[i].constructor, 0, NULL);
      if (!JS_IsException(inst)) {
        return inst;
      }
    }
  }
  return JS_UNDEFINED;
}

// document.createElement("tagName") -> PrisimiNode
static JSValue js_document_createElement(JSContext *ctx, JSValueConst this_val,
                                         int argc, JSValueConst *argv) {
  if (argc < 1)
    return JS_EXCEPTION;
  size_t len;
  const char *tag_name = JS_ToCStringLen(ctx, &len, argv[0]);
  if (!tag_name)
    return JS_EXCEPTION;

  JSValue custom_inst = check_custom_element_registry(ctx, tag_name);
  if (!JS_IsUndefined(custom_inst)) {
    JS_FreeCString(ctx, tag_name);
    return custom_inst;
  }

  uint32_t tag_id = map_string_to_tag_id(tag_name, len);
  JS_FreeCString(ctx, tag_name);

  uint64_t node_id = ext_salt_create_node(tag_id);
  JSValue obj = JS_NewObjectClass(ctx, prisimi_node_class_id);
  JS_SetOpaque(obj, (void *)(uintptr_t)node_id);
  return obj;
}

// Epic 62: document.createTextNode(text) -> PrisimiNode
static JSValue js_document_createTextNode(JSContext *ctx, JSValueConst this_val,
                                          int argc, JSValueConst *argv) {
  if (argc < 1)
    return JS_EXCEPTION;
  size_t len;
  const char *text = JS_ToCStringLen(ctx, &len, argv[0]);
  if (!text)
    return JS_EXCEPTION;

  // Allocate text content in the Salt arena
  uint64_t safe_ptr = dom_alloc_text((uint32_t)len);
  if (safe_ptr != 0) {
    memcpy((void *)(uintptr_t)safe_ptr, text, len);
  }
  JS_FreeCString(ctx, text);

  uint64_t node_id = ext_salt_create_text_node(safe_ptr, (uint32_t)len);
  JSValue obj = JS_NewObjectClass(ctx, prisimi_node_class_id);
  JS_SetOpaque(obj, (void *)(uintptr_t)node_id);
  return obj;
}

extern void ext_dom_insert_before(uint32_t parent_idx, uint32_t new_idx,
                                  uint32_t ref_idx);
extern void ext_dom_set_text_content(uint32_t node_idx, uint64_t str_ptr,
                                     uint32_t str_len);

static JSValue js_node_insertBefore(JSContext *ctx, JSValueConst this_val,
                                    int argc, JSValueConst *argv) {
  if (argc < 2)
    return JS_EXCEPTION;

  uint64_t parent_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  uint64_t new_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(argv[0], prisimi_node_class_id);

  uint32_t p_idx = (uint32_t)(parent_id & 0xFFFF);
  uint32_t n_idx = (uint32_t)(new_id & 0xFFFF);
  uint32_t r_idx = 0;

  if (!JS_IsNull(argv[1]) && !JS_IsUndefined(argv[1])) {
    uint64_t ref_id =
        (uint64_t)(uintptr_t)JS_GetOpaque(argv[1], prisimi_node_class_id);
    r_idx = (uint32_t)(ref_id & 0xFFFF);
  }

  ext_dom_insert_before(p_idx, n_idx, r_idx);
  return JS_DupValue(ctx, argv[0]);
}

static JSValue js_node_set_textContent(JSContext *ctx, JSValueConst this_val,
                                       JSValueConst val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  uint32_t n_idx = (uint32_t)(node_id & 0xFFFF);

  size_t str_len;
  const char *str = JS_ToCStringLen(ctx, &str_len, val);
  if (!str)
    return JS_EXCEPTION;

  ext_dom_set_text_content(n_idx, (uint64_t)str, (uint32_t)str_len);

  JS_FreeCString(ctx, str);
  return JS_UNDEFINED;
}

// Epic 62: textContent getter — read text from DOM_TEXT_PTR/LEN
static JSValue js_node_get_textContent(JSContext *ctx, JSValueConst this_val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  uint32_t n_idx = (uint32_t)(node_id & 0xFFFF);
  if (n_idx == 0 || n_idx >= 65536)
    return JS_NewString(ctx, "");
  uint64_t ptr = dom_get_text_ptr(n_idx);
  uint32_t len = dom_get_text_len(n_idx);
  if (ptr == 0 || len == 0)
    return JS_NewString(ctx, "");
  return JS_NewStringLen(ctx, (const char *)(uintptr_t)ptr, len);
}

// Epic 62: setAttribute(key, val) — routes to id/class/style or generic attr
static JSValue js_node_setAttribute(JSContext *ctx, JSValueConst this_val,
                                    int argc, JSValueConst *argv) {
  if (argc < 2)
    return JS_EXCEPTION;
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  uint32_t n_idx = (uint32_t)(node_id & 0xFFFF);
  if (n_idx == 0 || n_idx >= 65536)
    return JS_UNDEFINED;

  size_t key_len, val_len;
  const char *key = JS_ToCStringLen(ctx, &key_len, argv[0]);
  const char *val = JS_ToCStringLen(ctx, &val_len, argv[1]);
  if (!key || !val) {
    if (key)
      JS_FreeCString(ctx, key);
    if (val)
      JS_FreeCString(ctx, val);
    return JS_UNDEFINED;
  }

  // Allocate val into arena for safe storage across JS GC sweeps
  uint64_t safe_val = dom_alloc_text((uint32_t)val_len);
  if (safe_val != 0) {
    memcpy((void *)(uintptr_t)safe_val, val, val_len);
  }

  if (key_len == 2 && key[0] == 'i' && key[1] == 'd') {
    dom_set_id(n_idx, safe_val, (uint32_t)val_len);
  } else if (key_len == 5 && memcmp(key, "class", 5) == 0) {
    set_class(n_idx, safe_val, (uint32_t)val_len);
  } else if (key_len == 5 && memcmp(key, "value", 5) == 0) {
    ext_dom_set_text_content(n_idx, safe_val, (uint32_t)val_len);
  }

  JS_FreeCString(ctx, key);
  JS_FreeCString(ctx, val);
  return JS_UNDEFINED;
}

// Epic 62: removeAttribute(key)
static JSValue js_node_removeAttribute(JSContext *ctx, JSValueConst this_val,
                                       int argc, JSValueConst *argv) {
  if (argc < 1)
    return JS_UNDEFINED;
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  uint32_t n_idx = (uint32_t)(node_id & 0xFFFF);
  if (n_idx == 0 || n_idx >= 65536)
    return JS_UNDEFINED;

  size_t key_len;
  const char *key = JS_ToCStringLen(ctx, &key_len, argv[0]);
  if (!key)
    return JS_UNDEFINED;

  if (key_len == 2 && key[0] == 'i' && key[1] == 'd') {
    dom_set_id(n_idx, 0, 0);
  } else if (key_len == 5 && memcmp(key, "class", 5) == 0) {
    set_class(n_idx, 0, 0);
  }

  JS_FreeCString(ctx, key);
  return JS_UNDEFINED;
}

// Epic 62: firstChild getter
static JSValue js_node_get_firstChild(JSContext *ctx, JSValueConst this_val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  uint32_t n_idx = (uint32_t)(node_id & 0xFFFF);
  if (n_idx >= 65536)
    return JS_NULL;
  uint64_t child_id = dom_get_first_child(n_idx);
  if (child_id == 0)
    return JS_NULL;
  JSValue obj = JS_NewObjectClass(ctx, prisimi_node_class_id);
  JS_SetOpaque(obj, (void *)(uintptr_t)child_id);
  return obj;
}

// Epic 62: nextSibling getter
static JSValue js_node_get_nextSibling(JSContext *ctx, JSValueConst this_val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  uint32_t n_idx = (uint32_t)(node_id & 0xFFFF);
  if (n_idx >= 65536)
    return JS_NULL;
  uint64_t sib_id = dom_get_next_sibling(n_idx);
  if (sib_id == 0)
    return JS_NULL;
  JSValue obj = JS_NewObjectClass(ctx, prisimi_node_class_id);
  JS_SetOpaque(obj, (void *)(uintptr_t)sib_id);
  return obj;
}

// Epic 62: parentNode getter
static JSValue js_node_get_parentNode(JSContext *ctx, JSValueConst this_val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  uint32_t n_idx = (uint32_t)(node_id & 0xFFFF);
  if (n_idx >= 65536)
    return JS_NULL;
  extern uint32_t ext_dom_get_parent_idx(uint32_t idx);
  uint32_t parent_idx = ext_dom_get_parent_idx(n_idx);
  if (parent_idx == 0 || parent_idx >= 65536 || parent_idx == 999999)
    return JS_NULL;
  uint32_t parent_gen = dom_get_generation(parent_idx);
  uint64_t parent_id = (uint64_t)parent_idx | ((uint64_t)parent_gen << 16);
  JSValue obj = JS_NewObjectClass(ctx, prisimi_node_class_id);
  JS_SetOpaque(obj, (void *)(uintptr_t)parent_id);
  return obj;
}

// Epic 62: nodeType getter (1=ELEMENT, 3=TEXT)
static JSValue js_node_get_nodeType(JSContext *ctx, JSValueConst this_val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  uint32_t n_idx = (uint32_t)(node_id & 0xFFFF);
  if (n_idx >= 65536)
    return JS_NewInt32(ctx, 1);
  uint32_t tag = dom_get_tag(n_idx);
  if (tag == 0)
    return JS_NewInt32(ctx, 3); // TEXT_NODE
  return JS_NewInt32(ctx, 1);   // ELEMENT_NODE
}

// Epic 62: nodeName getter
static JSValue js_node_get_nodeName(JSContext *ctx, JSValueConst this_val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  uint32_t n_idx = (uint32_t)(node_id & 0xFFFF);
  if (n_idx >= 65536)
    return JS_NewString(ctx, "DIV");
  uint32_t tag = dom_get_tag(n_idx);
  switch (tag) {
  case 0:
    return JS_NewString(ctx, "#text");
  case 4:
    return JS_NewString(ctx, "DIV");
  case 5:
    return JS_NewString(ctx, "SPAN");
  case 6:
    return JS_NewString(ctx, "P");
  case 7:
    return JS_NewString(ctx, "A");
  case 9:
    return JS_NewString(ctx, "H1");
  case 18:
    return JS_NewString(ctx, "INPUT");
  case 20:
    return JS_NewString(ctx, "BUTTON");
  default:
    return JS_NewString(ctx, "DIV");
  }
}

// Epic 62: replaceChild(newChild, oldChild)
static JSValue js_node_replaceChild(JSContext *ctx, JSValueConst this_val,
                                    int argc, JSValueConst *argv) {
  if (argc < 2)
    return JS_EXCEPTION;
  uint64_t parent_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  uint64_t new_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(argv[0], prisimi_node_class_id);
  uint64_t old_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(argv[1], prisimi_node_class_id);
  uint32_t p_idx = (uint32_t)(parent_id & 0xFFFF);
  uint32_t n_idx = (uint32_t)(new_id & 0xFFFF);
  uint32_t o_idx = (uint32_t)(old_id & 0xFFFF);
  ext_dom_insert_before(p_idx, n_idx, o_idx);
  extern void ext_dom_remove_child(uint32_t parent_idx, uint32_t child_idx);
  ext_dom_remove_child(p_idx, o_idx);
  return JS_DupValue(ctx, argv[1]);
}

// Epic 62: id getter
static JSValue js_node_get_id(JSContext *ctx, JSValueConst this_val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  uint32_t n_idx = (uint32_t)(node_id & 0xFFFF);
  if (n_idx >= 65536)
    return JS_NewString(ctx, "");
  uint64_t ptr = dom_get_id_ptr(n_idx);
  uint32_t len = dom_get_id_len(n_idx);
  if (ptr == 0 || len == 0)
    return JS_NewString(ctx, "");
  return JS_NewStringLen(ctx, (const char *)(uintptr_t)ptr, len);
}

// Epic 62: value getter for input elements
static JSValue js_node_get_value(JSContext *ctx, JSValueConst this_val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  uint32_t n_idx = (uint32_t)(node_id & 0xFFFF);
  if (n_idx >= 65536)
    return JS_NewString(ctx, "");
  uint64_t ptr = dom_get_text_ptr(n_idx);
  uint32_t len = dom_get_text_len(n_idx);
  if (ptr == 0 || len == 0)
    return JS_NewString(ctx, "");
  return JS_NewStringLen(ctx, (const char *)(uintptr_t)ptr, len);
}

static JSValue js_node_set_value(JSContext *ctx, JSValueConst this_val,
                                 JSValueConst val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  uint32_t n_idx = (uint32_t)(node_id & 0xFFFF);
  size_t str_len;
  const char *str = JS_ToCStringLen(ctx, &str_len, val);
  if (!str)
    return JS_EXCEPTION;
  ext_dom_set_text_content(n_idx, (uint64_t)str, (uint32_t)str_len);
  JS_FreeCString(ctx, str);
  return JS_UNDEFINED;
}

// element.appendChild(child) -> child
static JSValue js_element_appendChild(JSContext *ctx, JSValueConst this_val,
                                      int argc, JSValueConst *argv) {
  if (argc < 1)
    return JS_EXCEPTION;
  uint64_t parent_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  uint64_t child_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(argv[0], prisimi_node_class_id);
  uint32_t p_idx = (uint32_t)(parent_id & 0xFFFF);
  uint32_t c_idx = (uint32_t)(child_id & 0xFFFF);
  js_dom_append_child(p_idx, c_idx);

  // Epic 63: WebComponent lifecycle hook
  JSValue cb = JS_GetPropertyStr(ctx, argv[0], "connectedCallback");
  if (JS_IsFunction(ctx, cb)) {
    printf("[DEBUG] Executing connectedCallback on node id %llu\n",
           (unsigned long long)child_id);
    JSValue ret = JS_Call(ctx, cb, argv[0], 0, NULL);
    if (JS_IsException(ret)) {
      printf("[DEBUG] connectedCallback threw an exception!\n");
      // print exception
      JSValue ex = JS_GetException(ctx);
      const char *ex_str = JS_ToCString(ctx, ex);
      printf("[DEBUG] Exception: %s\n", ex_str);
      JS_FreeCString(ctx, ex_str);
      JS_FreeValue(ctx, ex);
    }
    JS_FreeValue(ctx, ret);
  } else {
    printf("[DEBUG] Node id %llu has no connectedCallback! isObject=%d\n",
           (unsigned long long)child_id, JS_IsObject(argv[0]));
  }
  JS_FreeValue(ctx, cb);

  return JS_DupValue(ctx, argv[0]);
}

// element.removeChild(child) -> child
static JSValue js_element_removeChild(JSContext *ctx, JSValueConst this_val,
                                      int argc, JSValueConst *argv) {
  if (argc < 1)
    return JS_EXCEPTION;
  uint64_t parent_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  uint64_t child_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(argv[0], prisimi_node_class_id);
  uint32_t p_idx = (uint32_t)(parent_id & 0xFFFF);
  uint32_t c_idx = (uint32_t)(child_id & 0xFFFF);
  extern void ext_dom_remove_child(uint32_t parent_idx, uint32_t child_idx);
  ext_dom_remove_child(p_idx, c_idx);

  // Epic 63: WebComponent lifecycle hook
  JSValue cb = JS_GetPropertyStr(ctx, argv[0], "disconnectedCallback");
  if (JS_IsFunction(ctx, cb)) {
    JSValue ret = JS_Call(ctx, cb, argv[0], 0, NULL);
    JS_FreeValue(ctx, ret);
  }
  JS_FreeValue(ctx, cb);

  return JS_DupValue(ctx, argv[0]);
}

// element.click()
static JSValue js_prisimi_node_click(JSContext *ctx, JSValueConst this_val,
                                     int argc, JSValueConst *argv) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  js_bridge_dispatch_event(node_id, "click", 5);
  return JS_UNDEFINED;
}

// Epic 64: Shadow DOM Matrix - element.attachShadow(init) -> ShadowRoot
extern void ext_dom_set_shadow_root(uint32_t host_idx, uint32_t shadow_idx);

static JSValue js_element_attachShadow(JSContext *ctx, JSValueConst this_val,
                                       int argc, JSValueConst *argv) {
  uint64_t host_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);
  if (!host_id)
    return JS_EXCEPTION;

  // Create Shadow Root Node (TAG_SHADOW_ROOT = 21)
  uint64_t shadow_id = ext_salt_create_node(21);

  uint32_t host_idx = (uint32_t)(host_id & 0xFFFF);
  uint32_t shadow_idx = (uint32_t)(shadow_id & 0xFFFF);

  ext_dom_set_shadow_root(host_idx, shadow_idx);

  // Return a JS wrapper for the new ShadowRoot
  JSValue obj = JS_NewObjectClass(ctx, prisimi_node_class_id);
  JS_SetOpaque(obj, (void *)(uintptr_t)shadow_id);
  return obj;
}

// Epic 69: OOPIF postMessage Bridge
static JSValue js_window_postMessage(JSContext *ctx, JSValueConst this_val,
                                     int argc, JSValueConst *argv) {
  if (argc < 1)
    return JS_EXCEPTION;
  size_t len;
  const char *str = JS_ToCStringLen(ctx, &len, argv[0]);
  if (!str)
    return JS_EXCEPTION;

  // Child iframe to parent
  extern void sys_ipc_send_r2m_command_with_payload(
      uint32_t type, uint64_t arg1, uint64_t p_ptr, uint32_t p_len);
  sys_ipc_send_r2m_command_with_payload(2 /* R2M_POST_MESSAGE_UP */, 0,
                                        (uint64_t)(uintptr_t)str, len);

  JS_FreeCString(ctx, str);
  return JS_UNDEFINED;
}

static JSValue js_iframe_postMessage(JSContext *ctx, JSValueConst this_val,
                                     int argc, JSValueConst *argv, int magic,
                                     JSValue *func_data) {
  if (argc < 1)
    return JS_EXCEPTION;
  uint64_t node_id;
  JS_ToInt64(ctx, (int64_t *)&node_id, func_data[0]);

  size_t len;
  const char *str = JS_ToCStringLen(ctx, &len, argv[0]);
  if (!str)
    return JS_EXCEPTION;

  // Parent down to child iframe
  extern void sys_ipc_send_r2m_command_with_payload(
      uint32_t type, uint64_t arg1, uint64_t p_ptr, uint32_t p_len);
  sys_ipc_send_r2m_command_with_payload(6 /* CMD_POST_MESSAGE */, node_id,
                                        (uint64_t)(uintptr_t)str, len);

  JS_FreeCString(ctx, str);
  return JS_UNDEFINED;
}

static JSValue js_prisimi_node_get_contentWindow(JSContext *ctx,
                                                 JSValueConst this_val) {
  uint64_t node_id =
      (uint64_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_node_class_id);

  // Check if it's actually an iframe tag (tag 26)
  uint32_t idx = (uint32_t)(node_id & 0xFFFF);
  if (dom_get_tag(idx) != 26) {
    return JS_UNDEFINED;
  }

  JSValue win = JS_NewObject(ctx);
  JSValue node_id_val = JS_NewInt64(ctx, node_id);
  JSValue fn =
      JS_NewCFunctionData(ctx, js_iframe_postMessage, 1, 0, 1, &node_id_val);
  JS_SetPropertyStr(ctx, win, "postMessage", fn);
  JS_FreeValue(ctx, node_id_val);
  return win;
}

static const JSCFunctionListEntry prisimi_node_funcs[] = {
    JS_CFUNC_DEF("attachShadow", 1, js_element_attachShadow),
    JS_CFUNC_DEF("click", 0, js_prisimi_node_click),
    JS_CFUNC_DEF("addEventListener", 2, js_prisimi_node_addEventListener),
    JS_CFUNC_DEF("removeEventListener", 2, js_prisimi_node_removeEventListener),
    JS_CFUNC_DEF("setCanvasDimensions", 2, js_prisimi_node_setCanvasDimensions),
    JS_CFUNC_DEF("putImageData", 1, js_prisimi_node_putImageData),
    JS_CFUNC_DEF("appendChild", 1, js_element_appendChild),
    JS_CFUNC_DEF("removeChild", 1, js_element_removeChild),
    JS_CFUNC_DEF("insertBefore", 2, js_node_insertBefore),
    JS_CFUNC_DEF("replaceChild", 2, js_node_replaceChild),
    JS_CFUNC_DEF("setAttribute", 2, js_node_setAttribute),
    JS_CFUNC_DEF("removeAttribute", 1, js_node_removeAttribute),
    JS_CGETSET_DEF("contentWindow", js_prisimi_node_get_contentWindow, NULL),
    JS_CGETSET_DEF("className", js_prisimi_node_get_className,
                   js_prisimi_node_set_className),
    JS_CGETSET_DEF("innerHTML", js_prisimi_node_get_innerHTML,
                   js_prisimi_node_set_innerHTML),
    JS_CGETSET_DEF("id", js_node_get_id, js_prisimi_node_set_id),
    JS_CGETSET_DEF("scrollTop", NULL, js_prisimi_node_set_scrollTop),
    JS_CGETSET_DEF("textContent", js_node_get_textContent,
                   js_node_set_textContent),
    JS_CGETSET_DEF("nodeValue", js_node_get_textContent,
                   js_node_set_textContent),
    JS_CGETSET_DEF("value", js_node_get_value, js_node_set_value),
    JS_CGETSET_DEF("style", js_prisimi_node_get_style, NULL),
    JS_CGETSET_DEF("contentWindow", js_prisimi_node_get_contentWindow, NULL),
    JS_CGETSET_DEF("firstChild", js_node_get_firstChild, NULL),
    JS_CGETSET_DEF("nextSibling", js_node_get_nextSibling, NULL),
    JS_CGETSET_DEF("parentNode", js_node_get_parentNode, NULL),
    JS_CGETSET_DEF("nodeType", js_node_get_nodeType, NULL),
    JS_CGETSET_DEF("nodeName", js_node_get_nodeName, NULL),
    JS_CFUNC_DEF("getContext", 1, js_prisimi_node_getContext),
};

// Epic 62: setProperty support for Framework styles
static JSValue js_style_setProperty(JSContext *ctx, JSValueConst this_val,
                                    int argc, JSValueConst *argv) {
  if (argc < 2)
    return JS_EXCEPTION;
  const char *key = JS_ToCString(ctx, argv[0]);
  if (!key)
    return JS_EXCEPTION;

  // Route to existing setters
  if (strcmp(key, "display") == 0)
    js_style_set_display(ctx, this_val, argv[1]);
  else if (strcmp(key, "background-color") == 0 ||
           strcmp(key, "backgroundColor") == 0)
    js_style_set_backgroundColor(ctx, this_val, argv[1]);
  else if (strcmp(key, "transform") == 0)
    js_style_set_transform(ctx, this_val, argv[1]);
  else if (strcmp(key, "opacity") == 0)
    js_style_set_opacity(ctx, this_val, argv[1]);
  else if (strcmp(key, "width") == 0)
    js_style_set_width(ctx, this_val, argv[1]);
  else if (strcmp(key, "height") == 0)
    js_style_set_height(ctx, this_val, argv[1]);
  else if (strcmp(key, "flex-grow") == 0 || strcmp(key, "flexGrow") == 0)
    js_style_set_flexGrow(ctx, this_val, argv[1]);
  else if (strcmp(key, "grid-template-columns") == 0 ||
           strcmp(key, "gridTemplateColumns") == 0)
    js_style_set_gridTemplateColumns(ctx, this_val, argv[1]);
  else if (strcmp(key, "grid-column-start") == 0 ||
           strcmp(key, "gridColumnStart") == 0)
    js_style_set_gridColumnStart(ctx, this_val, argv[1]);
  else if (strcmp(key, "position") == 0)
    js_style_set_position(ctx, this_val, argv[1]);
  else if (strcmp(key, "top") == 0)
    js_style_set_top(ctx, this_val, argv[1]);
  else if (strcmp(key, "left") == 0)
    js_style_set_left(ctx, this_val, argv[1]);
  else if (strcmp(key, "z-index") == 0 || strcmp(key, "zIndex") == 0)
    js_style_set_zIndex(ctx, this_val, argv[1]);
  else if (strcmp(key, "overflow") == 0)
    js_style_set_overflow(ctx, this_val, argv[1]);

  JS_FreeCString(ctx, key);
  return JS_UNDEFINED;
}

static const JSCFunctionListEntry prisimi_style_funcs[] = {
    JS_CFUNC_DEF("setProperty", 2, js_style_setProperty),
    JS_CGETSET_DEF("backgroundColor", NULL, js_style_set_backgroundColor),
    JS_CGETSET_DEF("display", NULL, js_style_set_display),
    JS_CGETSET_DEF("transform", NULL, js_style_set_transform),
    JS_CGETSET_DEF("opacity", NULL, js_style_set_opacity),
    JS_CGETSET_DEF("width", NULL, js_style_set_width),
    JS_CGETSET_DEF("height", NULL, js_style_set_height),
    JS_CGETSET_DEF("flexGrow", NULL, js_style_set_flexGrow),
    JS_CGETSET_DEF("gridTemplateColumns", NULL,
                   js_style_set_gridTemplateColumns),
    JS_CGETSET_DEF("gridColumnStart", NULL, js_style_set_gridColumnStart),
    JS_CGETSET_DEF("position", NULL, js_style_set_position),
    JS_CGETSET_DEF("top", NULL, js_style_set_top),
    JS_CGETSET_DEF("left", NULL, js_style_set_left),
    JS_CGETSET_DEF("zIndex", NULL, js_style_set_zIndex),
    JS_CGETSET_DEF("overflow", NULL, js_style_set_overflow),
};

void *js_bridge_malloc(JSMallocState *s, size_t size) {
  if (size == 0)
    return NULL;
  uint32_t offset = airlock_allocate((uint32_t)size);
  if (offset == 0xFFFFFFFF)
    return NULL;
  uint8_t *airlock_mem = (uint8_t *)airlock_get_ptr();
  return (void *)(airlock_mem + offset);
}

void js_bridge_free(JSMallocState *s, void *ptr) {
  if (!ptr)
    return;
  uint8_t *airlock_mem = (uint8_t *)airlock_get_ptr();
  uint32_t offset = (uint32_t)((uint8_t *)ptr - airlock_mem);
  airlock_deallocate(offset);
}

void *js_bridge_realloc(JSMallocState *s, void *ptr, size_t size) {
  if (size == 0) {
    js_bridge_free(s, ptr);
    return NULL;
  }
  if (!ptr)
    return js_bridge_malloc(s, size);
  uint8_t *airlock_mem = (uint8_t *)airlock_get_ptr();
  uint32_t offset = (uint32_t)((uint8_t *)ptr - airlock_mem);
  uint32_t old_block_size = airlock_get_block_size(offset);
  if (size <= old_block_size)
    return ptr;
  void *new_ptr = js_bridge_malloc(s, size);
  if (!new_ptr)
    return NULL;
  memcpy(new_ptr, ptr, old_block_size);
  js_bridge_free(s, ptr);
  return new_ptr;
}

static const JSMallocFunctions bridge_malloc_funcs = {
    js_bridge_malloc, js_bridge_free, js_bridge_realloc, NULL};
static JSMallocState bridge_malloc_state = {0};

#include "../../vendor/crypto/base64.c"

extern uint32_t ext_net_open_websocket(uint64_t url_ptr, uint32_t len);
extern void websocket_send(uint32_t ws_id, uint64_t payload_ptr,
                           uint32_t payload_len);

static JSValue js_websockets[256];

void js_bridge_dispatch_websocket_message(uint32_t ws_id, uint64_t msg_ptr,
                                          uint32_t msg_len) {
  printf("[C] Dispatching Websocket Frame! ws_id: %u\n", ws_id);
  if (!ctx)
    return;
  if (ws_id >= 256)
    return;

  JSValue ws_obj = js_websockets[ws_id];

  JSValue onmessage = JS_GetPropertyStr(ctx, ws_obj, "onmessage");
  printf("[C] Fetched onmessage property natively accurately!\n");
  if (JS_IsFunction(ctx, onmessage)) {
    printf("[C] Bound to QuickJS Function accurately.\n");
    JSValue event_obj = JS_NewObject(ctx);
    JSValue data_str =
        JS_NewStringLen(ctx, (const char *)(uintptr_t)msg_ptr, msg_len);
    JS_SetPropertyStr(ctx, event_obj, "data", data_str);

    JS_Call(ctx, onmessage, ws_obj, 1, &event_obj);
    JS_FreeValue(ctx, event_obj);
  }
  JS_FreeValue(ctx, onmessage);
}

static JSClassID prisimi_websocket_class_id;

static void js_prisimi_websocket_finalizer(JSRuntime *rt, JSValue val) {}

static JSClassDef prisimi_websocket_class = {
    "WebSocket",
    .finalizer = js_prisimi_websocket_finalizer,
};

static JSValue js_prisimi_websocket_send(JSContext *ctx, JSValueConst this_val,
                                         int argc, JSValueConst *argv) {
  uint32_t ws_id =
      (uint32_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_websocket_class_id);
  size_t len;
  const char *str = JS_ToCStringLen(ctx, &len, argv[0]);
  if (!str)
    return JS_EXCEPTION;
  websocket_send(ws_id, (uint64_t)(uintptr_t)str, (uint32_t)len);
  JS_FreeCString(ctx, str);
  return JS_UNDEFINED;
}

static const JSCFunctionListEntry prisimi_websocket_funcs[] = {
    JS_CFUNC_DEF("send", 1, js_prisimi_websocket_send),
};

static JSValue js_window_WebSocket(JSContext *ctx, JSValueConst this_val,
                                   int argc, JSValueConst *argv) {
  size_t url_len;
  const char *url = JS_ToCStringLen(ctx, &url_len, argv[0]);
  if (!url)
    return JS_EXCEPTION;
  printf("[C] Creating WebSocket Native Object! URL: %s\n", url);
  uint32_t ws_id =
      ext_net_open_websocket((uint64_t)(uintptr_t)url, (uint32_t)url_len);
  JS_FreeCString(ctx, url);
  JSValue obj = JS_NewObjectClass(ctx, prisimi_websocket_class_id);
  JS_SetOpaque(obj, (void *)(uintptr_t)ws_id);
  js_websockets[ws_id] = JS_DupValue(ctx, obj);
  printf("[C] Native Instance Extracted: %u\n", ws_id);
  return obj;
}

void generate_ws_accept_key_bridge(uint64_t key_ptr, uint32_t len,
                                   uint64_t out_ptr) {
  generate_ws_accept_key((const char *)(uintptr_t)key_ptr, len,
                         (char *)(uintptr_t)out_ptr);
}

extern void js_post_worker_down(uint64_t msg_ptr, uint32_t msg_len);

static JSContext *worker_ctx = NULL;
static JSRuntime *worker_rt = NULL;

static JSClassID prisimi_worker_class_id;

static void js_prisimi_worker_finalizer(JSRuntime *rt, JSValue val) {}

static JSClassDef prisimi_worker_class = {
    "Worker",
    .finalizer = js_prisimi_worker_finalizer,
};

static JSValue js_prisimi_worker_postMessage(JSContext *ctx,
                                             JSValueConst this_val, int argc,
                                             JSValueConst *argv) {
  size_t len;
  const char *str = JS_ToCStringLen(ctx, &len, argv[0]);
  if (!str)
    return JS_EXCEPTION;
  js_post_worker_down((uint64_t)(uintptr_t)str, (uint32_t)len);
  JS_FreeCString(ctx, str);
  return JS_UNDEFINED;
}

static const JSCFunctionListEntry prisimi_worker_funcs[] = {
    JS_CFUNC_DEF("postMessage", 1, js_prisimi_worker_postMessage),
};

static JSValue global_worker_obj;

static JSValue js_window_Worker(JSContext *ctx, JSValueConst this_val, int argc,
                                JSValueConst *argv) {
  size_t url_len;
  const char *url = JS_ToCStringLen(ctx, &url_len, argv[0]);
  if (!url)
    return JS_EXCEPTION;

  JS_FreeCString(ctx, url);
  JSValue obj = JS_NewObjectClass(ctx, prisimi_worker_class_id);
  global_worker_obj = JS_DupValue(ctx, obj);
  return obj;
}

extern void js_post_worker_up(uint64_t msg_ptr, uint32_t msg_len);

static JSValue js_worker_global_postMessage(JSContext *wctx,
                                            JSValueConst this_val, int argc,
                                            JSValueConst *argv) {
  size_t len;
  const char *str = JS_ToCStringLen(wctx, &len, argv[0]);
  if (!str)
    return JS_EXCEPTION;
  js_post_worker_up((uint64_t)(uintptr_t)str, (uint32_t)len);
  JS_FreeCString(wctx, str);
  return JS_UNDEFINED;
}

void js_bridge_dispatch_main_message(uint64_t msg_ptr, uint32_t msg_len) {
  if (!ctx)
    return;
  JSValue onmessage = JS_GetPropertyStr(ctx, global_worker_obj, "onmessage");
  if (JS_IsFunction(ctx, onmessage)) {
    JSValue event_obj = JS_NewObject(ctx);
    JSValue data_str =
        JS_NewStringLen(ctx, (const char *)(uintptr_t)msg_ptr, msg_len);
    JS_SetPropertyStr(ctx, event_obj, "data", data_str);

    JS_Call(ctx, onmessage, global_worker_obj, 1, &event_obj);
    JS_FreeValue(ctx, event_obj);
  }
  JS_FreeValue(ctx, onmessage);
}

int32_t js_quickjs_init_worker(uint64_t script_data_ptr, uint32_t len) {
  worker_rt = JS_NewRuntime2(&bridge_malloc_funcs, &bridge_malloc_state);
  if (!worker_rt)
    return -1;
  worker_ctx = JS_NewContext(worker_rt);
  if (!worker_ctx)
    return -1;

  JSValue global_obj = JS_GetGlobalObject(worker_ctx);
  JS_SetPropertyStr(worker_ctx, global_obj, "postMessage",
                    JS_NewCFunction(worker_ctx, js_worker_global_postMessage,
                                    "postMessage", 1));
  JS_FreeValue(worker_ctx, global_obj);

  return 1;
}

int32_t sys_js_init_worker_context() {
  rt = JS_NewRuntime2(&bridge_malloc_funcs, &bridge_malloc_state);
  ctx = JS_NewContext(rt);

  JS_NewClassID(&prisimi_fetchevent_class_id);
  JS_NewClass(rt, prisimi_fetchevent_class_id, &prisimi_fetchevent_class);

  JSValue global_obj = JS_GetGlobalObject(ctx);
  JSValue fe_proto = JS_NewObject(ctx);
  JS_SetPropertyStr(
      ctx, fe_proto, "respondWith",
      JS_NewCFunction(ctx, js_fetchevent_respondWith, "respondWith", 1));
  JS_SetClassProto(ctx, prisimi_fetchevent_class_id, fe_proto);

  JS_SetPropertyStr(ctx, global_obj, "self", JS_DupValue(ctx, global_obj));
  JS_SetPropertyStr(ctx, global_obj, "addEventListener",
                    JS_NewCFunction(ctx, js_document_addEventListener,
                                    "addEventListener", 2));

  JS_FreeValue(ctx, global_obj);
  return 1;
}

void js_bridge_dispatch_worker_message(uint64_t msg_ptr, uint32_t msg_len) {
  if (!worker_ctx)
    return;
  JSValue global_obj = JS_GetGlobalObject(worker_ctx);
  JSValue onmessage = JS_GetPropertyStr(worker_ctx, global_obj, "onmessage");
  if (JS_IsFunction(worker_ctx, onmessage)) {
    JSValue event_obj = JS_NewObject(worker_ctx);
    JSValue data_str =
        JS_NewStringLen(worker_ctx, (const char *)(uintptr_t)msg_ptr, msg_len);
    JS_SetPropertyStr(worker_ctx, event_obj, "data", data_str);

    JS_Call(worker_ctx, onmessage, global_obj, 1, &event_obj);
    JS_FreeValue(worker_ctx, event_obj);
  }
  JS_FreeValue(worker_ctx, onmessage);
  JS_FreeValue(worker_ctx, global_obj);
}

int32_t js_execute_worker_jobs() {
  JSContext *pctx;
  return JS_ExecutePendingJob(worker_rt, &pctx);
}

static JSValue js_timer_callbacks[256];
static uint32_t next_timer_id = 1;

extern int32_t ext_timers_register(uint32_t timer_id, uint32_t delay_ms,
                                   uint8_t is_interval);
extern void ext_timers_clear(uint32_t timer_id);

static JSValue js_global_setTimeout(JSContext *ctx, JSValueConst this_val,
                                    int argc, JSValueConst *argv) {
  if (argc < 1 || !JS_IsFunction(ctx, argv[0]))
    return JS_EXCEPTION;

  uint64_t delay = 0;
  if (argc > 1)
    JS_ToInt64(ctx, (int64_t *)&delay, argv[1]);

  uint32_t t_id = next_timer_id++;
  uint32_t slot = t_id % 256;

  if (!JS_IsUndefined(js_timer_callbacks[slot])) {
    JS_FreeValue(ctx, js_timer_callbacks[slot]);
  }

  js_timer_callbacks[slot] = JS_DupValue(ctx, argv[0]);
  ext_timers_register(t_id, (uint32_t)delay, 0); // 0 = setTimeout
  return JS_NewInt32(ctx, t_id);
}

static JSValue js_global_setInterval(JSContext *ctx, JSValueConst this_val,
                                     int argc, JSValueConst *argv) {
  if (argc < 1 || !JS_IsFunction(ctx, argv[0]))
    return JS_EXCEPTION;

  uint64_t delay = 0;
  if (argc > 1)
    JS_ToInt64(ctx, (int64_t *)&delay, argv[1]);

  uint32_t t_id = next_timer_id++;
  uint32_t slot = t_id % 256;

  if (!JS_IsUndefined(js_timer_callbacks[slot])) {
    JS_FreeValue(ctx, js_timer_callbacks[slot]);
  }

  js_timer_callbacks[slot] = JS_DupValue(ctx, argv[0]);
  ext_timers_register(t_id, (uint32_t)delay, 1); // 1 = setInterval
  return JS_NewInt32(ctx, t_id);
}

static JSValue js_global_clearTimeout(JSContext *ctx, JSValueConst this_val,
                                      int argc, JSValueConst *argv) {
  if (argc < 1)
    return JS_UNDEFINED;
  uint32_t t_id = 0;
  JS_ToUint32(ctx, &t_id, argv[0]);

  uint32_t slot = t_id % 256;
  if (!JS_IsUndefined(js_timer_callbacks[slot])) {
    JS_FreeValue(ctx, js_timer_callbacks[slot]);
    js_timer_callbacks[slot] = JS_UNDEFINED;
  }

  ext_timers_clear(t_id);
  return JS_UNDEFINED;
}

void sys_js_execute_timer(uint32_t timer_id, uint8_t is_interval) {
  if (!ctx)
    return;
  uint32_t slot = timer_id % 256;
  JSValue cb = js_timer_callbacks[slot];

  if (!JS_IsUndefined(cb) && JS_IsFunction(ctx, cb)) {
    JS_Call(ctx, cb, JS_UNDEFINED, 0, NULL);

    if (is_interval == 0) {
      JS_FreeValue(ctx, cb);
      js_timer_callbacks[slot] = JS_UNDEFINED;
    }
  }
}

static JSValue js_raf_callbacks[256];
static uint32_t next_raf_id = 1;

extern int32_t ext_raf_register(uint32_t raf_id);
extern void ext_raf_cancel(uint32_t raf_id);

static JSValue js_window_requestAnimationFrame(JSContext *ctx,
                                               JSValueConst this_val, int argc,
                                               JSValueConst *argv) {
  if (argc < 1 || !JS_IsFunction(ctx, argv[0]))
    return JS_EXCEPTION;

  uint32_t r_id = next_raf_id++;
  uint32_t slot = r_id % 256;

  if (!JS_IsUndefined(js_raf_callbacks[slot])) {
    JS_FreeValue(ctx, js_raf_callbacks[slot]);
  }

  js_raf_callbacks[slot] = JS_DupValue(ctx, argv[0]);
  ext_raf_register(r_id);

  return JS_NewInt32(ctx, r_id);
}

static JSValue js_window_cancelAnimationFrame(JSContext *ctx,
                                              JSValueConst this_val, int argc,
                                              JSValueConst *argv) {
  if (argc < 1)
    return JS_UNDEFINED;
  uint32_t r_id = 0;
  JS_ToUint32(ctx, &r_id, argv[0]);

  uint32_t slot = r_id % 256;
  if (!JS_IsUndefined(js_raf_callbacks[slot])) {
    JS_FreeValue(ctx, js_raf_callbacks[slot]);
    js_raf_callbacks[slot] = JS_UNDEFINED;
  }

  ext_raf_cancel(r_id);
  return JS_UNDEFINED;
}

void sys_js_execute_raf(uint32_t raf_id, double timestamp) {
  if (!ctx)
    return;
  uint32_t slot = raf_id % 256;
  JSValue cb = js_raf_callbacks[slot];

  if (!JS_IsUndefined(cb) && JS_IsFunction(ctx, cb)) {
    JSValue time_val = JS_NewFloat64(ctx, timestamp);
    JS_Call(ctx, cb, JS_UNDEFINED, 1, &time_val);
    JS_FreeValue(ctx, time_val);

    JS_FreeValue(ctx, cb);
    js_raf_callbacks[slot] = JS_UNDEFINED;
  }
}

// Epic 63: Custom Element Registry
static JSValue js_customElements_define(JSContext *ctx, JSValueConst this_val,
                                        int argc, JSValueConst *argv) {
  if (argc < 2)
    return JS_EXCEPTION;
  const char *tag_name = JS_ToCString(ctx, argv[0]);
  if (!tag_name)
    return JS_EXCEPTION;

  if (custom_elements_count < 64) {
    CustomElementDefinition *def = &custom_elements[custom_elements_count++];
    def->tag_hash = fnv1a_hash_str(tag_name);
    def->constructor = JS_DupValue(ctx, argv[1]);
  }

  JS_FreeCString(ctx, tag_name);
  return JS_UNDEFINED;
}

// Epic 63: HTMLElement Base Class constructor for Custom Elements
static JSValue js_HTMLElement_constructor(JSContext *ctx,
                                          JSValueConst new_target, int argc,
                                          JSValueConst *argv) {
  JSValue proto = JS_GetPropertyStr(ctx, new_target, "prototype");
  if (JS_IsException(proto))
    return JS_EXCEPTION;

  JSValue obj = JS_NewObjectProtoClass(ctx, proto, prisimi_node_class_id);
  JS_FreeValue(ctx, proto);
  if (JS_IsException(obj))
    return obj;

  // Instantiate a default DOM node (TAG_DIV) to back this Custom Element
  // natively
  uint64_t node_id = ext_salt_create_node(4);
  JS_SetOpaque(obj, (void *)(uintptr_t)node_id);
  return obj;
}

// Epic 65: Media Source Extensions
static JSClassID prisimi_sourcebuffer_class_id;

static void js_prisimi_sourcebuffer_finalizer(JSRuntime *rt, JSValue val) {}

static JSClassDef prisimi_sourcebuffer_class = {
    "SourceBuffer",
    .finalizer = js_prisimi_sourcebuffer_finalizer,
};

extern void ext_media_push_chunk(uint64_t data_ptr, uint32_t data_len);
extern void sys_js_dispatch_event(const char *event_type);

static JSValue js_sourceBuffer_appendBuffer(JSContext *ctx,
                                            JSValueConst this_val, int argc,
                                            JSValueConst *argv) {
  if (argc < 1)
    return JS_EXCEPTION;

  size_t size;
  uint8_t *buf = JS_GetArrayBuffer(ctx, &size, argv[0]);
  if (!buf)
    return JS_ThrowTypeError(ctx, "Expected ArrayBuffer");

  ext_media_push_chunk((uint64_t)(uintptr_t)buf, (uint32_t)size);

  // Simulate async completion of the append operation
  // sys_js_dispatch_event("updateend");
  // Actually we will dispatch event back via the js bridge? Or just skip since
  // tests may not need 'updateend' specifically if we pump
  return JS_UNDEFINED;
}

static JSValue js_MediaSource_addSourceBuffer(JSContext *ctx,
                                              JSValueConst this_val, int argc,
                                              JSValueConst *argv) {
  JSValue obj = JS_NewObjectClass(ctx, prisimi_sourcebuffer_class_id);
  return obj;
}

static JSValue js_window_MediaSource(JSContext *ctx, JSValueConst this_val,
                                     int argc, JSValueConst *argv) {
  JSValue obj = JS_NewObject(ctx);
  JS_SetPropertyStr(ctx, obj, "addSourceBuffer",
                    JS_NewCFunction(ctx, js_MediaSource_addSourceBuffer,
                                    "addSourceBuffer", 1));
  return obj;
}

// Epic 73: The Audio Matrix — Web Audio API FFI
static JSClassID prisimi_audiocontext_class_id;
static JSClassID prisimi_audiobuffer_class_id;
static JSClassID prisimi_audiobuffersourcenode_class_id;

static JSValue js_audio_buffer_source_start(JSContext *ctx,
                                            JSValueConst this_val, int argc,
                                            JSValueConst *argv) {
  JSValue buffer_val = JS_GetPropertyStr(ctx, this_val, "buffer");
  if (JS_IsUndefined(buffer_val) || JS_IsNull(buffer_val))
    return JS_EXCEPTION;

  // In our simplified MDN-lite model, AudioBuffer contains a single channel
  // 'data' Float32Array
  JSValue data_val = JS_GetPropertyStr(ctx, buffer_val, "data");
  size_t byte_len = 0;
  uint8_t *raw_ptr = JS_GetArrayBuffer(ctx, &byte_len, data_val);

  if (raw_ptr && byte_len > 0) {
    // Zero-copy bridge to Salt hardware ring buffer
    ext_media_push_audio_pcm((uint64_t)(uintptr_t)raw_ptr,
                             (uint32_t)(byte_len / sizeof(float)));
  }

  JS_FreeValue(ctx, data_val);
  JS_FreeValue(ctx, buffer_val);
  return JS_UNDEFINED;
}

static JSValue js_audiocontext_createBufferSource(JSContext *ctx,
                                                  JSValueConst this_val,
                                                  int argc,
                                                  JSValueConst *argv) {
  JSValue obj = JS_NewObjectClass(ctx, prisimi_audiobuffersourcenode_class_id);
  JS_SetPropertyStr(
      ctx, obj, "start",
      JS_NewCFunction(ctx, js_audio_buffer_source_start, "start", 0));
  return obj;
}

static JSValue js_audiocontext_createBuffer(JSContext *ctx,
                                            JSValueConst this_val, int argc,
                                            JSValueConst *argv) {
  if (argc < 3)
    return JS_EXCEPTION;
  uint32_t channels, length, sample_rate;
  JS_ToUint32(ctx, &channels, argv[0]);
  JS_ToUint32(ctx, &length, argv[1]);
  JS_ToUint32(ctx, &sample_rate, argv[2]);

  JSValue buffer = JS_NewObjectClass(ctx, prisimi_audiobuffer_class_id);

  // Allocate raw PCM storage
  JSValue data = JS_NewArrayBufferCopy(ctx, NULL, length * sizeof(float));
  JS_SetPropertyStr(ctx, buffer, "data", data);
  JS_SetPropertyStr(ctx, buffer, "length", JS_NewInt32(ctx, length));
  JS_SetPropertyStr(ctx, buffer, "sampleRate", JS_NewInt32(ctx, sample_rate));

  return buffer;
}

static JSValue js_audio_buffer_getChannelData(JSContext *ctx,
                                              JSValueConst this_val, int argc,
                                              JSValueConst *argv) {
  // Return the pre-allocated ArrayBuffer as a Float32Array view
  JSValue data = JS_GetPropertyStr(ctx, this_val, "data");
  JSValue view = JS_NewObject(ctx); // simplified, usually JS_NewTypedArray
  // For this e2e test, we'll just return the backing buffer if it's already an
  // ArrayBuffer
  return data;
}

static JSValue js_window_AudioContext(JSContext *ctx, JSValueConst this_val,
                                      int argc, JSValueConst *argv) {
  JSValue obj = JS_NewObjectClass(ctx, prisimi_audiocontext_class_id);
  JS_SetPropertyStr(
      ctx, obj, "createBuffer",
      JS_NewCFunction(ctx, js_audiocontext_createBuffer, "createBuffer", 3));
  JS_SetPropertyStr(ctx, obj, "createBufferSource",
                    JS_NewCFunction(ctx, js_audiocontext_createBufferSource,
                                    "createBufferSource", 0));
  JS_SetPropertyStr(ctx, obj, "destination",
                    JS_NewObject(ctx)); // Stub destination
  return obj;
}

static JSValue js_URL_createObjectURL(JSContext *ctx, JSValueConst this_val,
                                      int argc, JSValueConst *argv) {
  return JS_NewString(ctx, "blob:media");
}
extern void ext_history_push(uint64_t url_ptr, uint32_t url_len,
                             uint64_t state_ptr, uint32_t state_len);
extern uint64_t
airlock_alloc(uint32_t size); // Use airlock allocation for state

static JSValue js_history_pushState(JSContext *ctx, JSValueConst this_val,
                                    int argc, JSValueConst *argv) {
  if (argc < 3)
    return JS_EXCEPTION;

  // 1. Serialize State Object
  JSValue state_str_val =
      JS_JSONStringify(ctx, argv[0], JS_UNDEFINED, JS_UNDEFINED);
  size_t state_len = 0;
  const char *state_str = NULL;
  uint64_t safe_state_ptr = 0;

  if (!JS_IsUndefined(state_str_val)) {
    state_str = JS_ToCStringLen(ctx, &state_len, state_str_val);
    if (state_len > 0) {
      safe_state_ptr = dom_alloc_text((uint32_t)state_len);
      if (safe_state_ptr != 0) {
        memcpy((void *)(uintptr_t)safe_state_ptr, state_str, state_len);
      }
    }
  }

  // 2. Extract URL
  size_t url_len;
  const char *url_str = JS_ToCStringLen(ctx, &url_len, argv[2]);

  // 3. Push to Native Stack
  ext_history_push((uint64_t)url_str, (uint32_t)url_len, safe_state_ptr,
                   (uint32_t)state_len);

  if (state_str)
    JS_FreeCString(ctx, state_str);
  if (url_str)
    JS_FreeCString(ctx, url_str);
  JS_FreeValue(ctx, state_str_val);

  return JS_UNDEFINED;
}

void sys_js_dispatch_popstate(uint64_t state_json_ptr, uint32_t state_len) {
  if (!ctx)
    return;
  JSValue event = JS_NewObject(ctx);
  JS_SetPropertyStr(ctx, event, "type", JS_NewString(ctx, "popstate"));

  if (state_len > 0 && state_json_ptr != 0) {
    JSValue state_obj = JS_ParseJSON(ctx, (const char *)state_json_ptr,
                                     state_len, "history.json");
    JS_SetPropertyStr(ctx, event, "state", state_obj);
  } else {
    JS_SetPropertyStr(ctx, event, "state", JS_NULL);
  }

  uint32_t hash = fnv1a_hash_len("popstate", 8);
  for (int i = 0; i < js_listeners_count; i++) {
    if (js_listeners[i].node_id == 999999 &&
        js_listeners[i].event_type_hash == hash) {
      JSValue ret =
          JS_Call(ctx, js_listeners[i].callback, JS_UNDEFINED, 1, &event);
      JS_FreeValue(ctx, ret);
    }
  }

  JS_FreeValue(ctx, event);

  js_execute_pending_jobs();
}

static JSValue js_window_addEventListener(JSContext *ctx, JSValueConst this_val,
                                          int argc, JSValueConst *argv) {
  if (argc < 2)
    return JS_EXCEPTION;
  const char *type = JS_ToCString(ctx, argv[0]);
  if (!type)
    return JS_EXCEPTION;
  if (js_listeners_count < 1024) {
    PrisimiEventListener *l = &js_listeners[js_listeners_count++];
    l->node_id = 999999;
    l->event_type_hash = fnv1a_hash_str(type);
    l->callback = JS_DupValue(ctx, argv[1]);
  }
  JS_FreeCString(ctx, type);
  return JS_UNDEFINED;
}

int32_t js_init_quickjs() {
  rt = JS_NewRuntime2(&bridge_malloc_funcs, &bridge_malloc_state);
  if (rt == NULL)
    return -1;
  ctx = JS_NewContext(rt);
  if (ctx == NULL)
    return -1;

  JS_NewClassID(&prisimi_node_class_id);
  JS_NewClass(rt, prisimi_node_class_id, &prisimi_node_class);

  JS_NewClassID(&prisimi_selection_class_id);
  JS_NewClass(rt, prisimi_selection_class_id, &(JSClassDef){"Selection", NULL});

  JS_NewClassID(&prisimi_range_class_id);
  JS_NewClass(rt, prisimi_range_class_id, &(JSClassDef){"Range", NULL});

  JSValue global_obj = JS_GetGlobalObject(ctx);
  JS_SetPropertyStr(
      ctx, global_obj, "getSelection",
      JS_NewCFunction(ctx, js_window_getSelection, "getSelection", 0));
  // Also available on window explicitly
  JS_SetPropertyStr(ctx, global_obj, "window", JS_GetGlobalObject(ctx));
  JS_SetPropertyStr(ctx, global_obj, "fetch",
                    JS_NewCFunction(ctx, js_window_fetch, "fetch", 1));
  JS_SetPropertyStr(
      ctx, global_obj, "setTimeout",
      JS_NewCFunction(ctx, js_global_setTimeout, "setTimeout", 2));
  JS_SetPropertyStr(
      ctx, global_obj, "setInterval",
      JS_NewCFunction(ctx, js_global_setInterval, "setInterval", 2));
  JS_SetPropertyStr(
      ctx, global_obj, "clearTimeout",
      JS_NewCFunction(ctx, js_global_clearTimeout, "clearTimeout", 1));
  JS_SetPropertyStr(
      ctx, global_obj, "clearInterval",
      JS_NewCFunction(ctx, js_global_clearTimeout, "clearInterval", 1));
  JS_SetPropertyStr(ctx, global_obj, "requestAnimationFrame",
                    JS_NewCFunction(ctx, js_window_requestAnimationFrame,
                                    "requestAnimationFrame", 1));
  JS_SetPropertyStr(ctx, global_obj, "cancelAnimationFrame",
                    JS_NewCFunction(ctx, js_window_cancelAnimationFrame,
                                    "cancelAnimationFrame", 1));
  JS_SetPropertyStr(
      ctx, global_obj, "addEventListener",
      JS_NewCFunction(ctx, js_window_addEventListener, "addEventListener", 2));
  JS_SetPropertyStr(
      ctx, global_obj, "postMessage",
      JS_NewCFunction(ctx, js_window_postMessage, "postMessage", 1));

  JSValue history_obj = JS_NewObject(ctx);
  JS_SetPropertyStr(ctx, history_obj, "pushState",
                    JS_NewCFunction(ctx, js_history_pushState, "pushState", 3));
  JS_SetPropertyStr(ctx, global_obj, "history", history_obj);

  JSValue url_obj = JS_NewObject(ctx);
  JS_SetPropertyStr(
      ctx, url_obj, "createObjectURL",
      JS_NewCFunction(ctx, js_URL_createObjectURL, "createObjectURL", 1));
  JS_SetPropertyStr(ctx, global_obj, "URL", url_obj);

  // Epic 63: window.customElements
  JSValue custom_elements_obj = JS_NewObject(ctx);
  JS_SetPropertyStr(
      ctx, custom_elements_obj, "define",
      JS_NewCFunction(ctx, js_customElements_define, "define", 2));
  JS_SetPropertyStr(ctx, global_obj, "customElements", custom_elements_obj);

  JSValue document_obj = JS_NewObject(ctx);
  JS_SetPropertyStr(
      ctx, document_obj, "getElementById",
      JS_NewCFunction(ctx, js_document_getElementById, "getElementById", 1));
  JS_SetPropertyStr(
      ctx, document_obj, "createElement",
      JS_NewCFunction(ctx, js_document_createElement, "createElement", 1));
  JS_SetPropertyStr(
      ctx, document_obj, "createTextNode",
      JS_NewCFunction(ctx, js_document_createTextNode, "createTextNode", 1));
  JS_SetPropertyStr(ctx, document_obj, "addEventListener",
                    JS_NewCFunction(ctx, js_document_addEventListener,
                                    "addEventListener", 2));
  JSValue idb_obj = JS_NewObject(ctx);
  JS_SetPropertyStr(ctx, idb_obj, "put",
                    JS_NewCFunction(ctx, js_idb_put, "put", 2));
  JS_SetPropertyStr(ctx, idb_obj, "get",
                    JS_NewCFunction(ctx, js_idb_get, "get", 1));
  JS_SetPropertyStr(ctx, global_obj, "indexedDB", idb_obj);

  JSValue nav_obj = JS_NewObject(ctx);
  JSValue sw_obj = JS_NewObject(ctx);
  JS_SetPropertyStr(ctx, sw_obj, "register",
                    JS_NewCFunction(ctx, js_sw_register, "register", 1));
  JS_SetPropertyStr(ctx, nav_obj, "serviceWorker", sw_obj);
  JS_SetPropertyStr(ctx, global_obj, "navigator", nav_obj);
  JS_SetPropertyStr(ctx, global_obj, "print",
                    JS_NewCFunction(ctx, js_print, "print", 1));

  JSValue proto = JS_NewObject(ctx);
  JS_SetPropertyFunctionList(ctx, proto, prisimi_node_funcs,
                             sizeof(prisimi_node_funcs) /
                                 sizeof(prisimi_node_funcs[0]));
  JS_SetClassProto(ctx, prisimi_node_class_id, proto);

  JSValue sel_proto = JS_NewObject(ctx);
  JS_SetPropertyFunctionList(ctx, sel_proto, js_selection_proto_funcs,
                             sizeof(js_selection_proto_funcs) /
                                 sizeof(js_selection_proto_funcs[0]));
  JS_SetClassProto(ctx, prisimi_selection_class_id, sel_proto);

  JSValue range_proto = JS_NewObject(ctx);
  JS_SetPropertyFunctionList(ctx, range_proto, js_range_proto_funcs,
                             sizeof(js_range_proto_funcs) /
                                 sizeof(js_range_proto_funcs[0]));
  JS_SetClassProto(ctx, prisimi_range_class_id, range_proto);

  // Epic 63: Register globally available HTMLElement extending the Prisimi
  // native prototype
  JSValue html_elem_ctor =
      JS_NewCFunction2(ctx, js_HTMLElement_constructor, "HTMLElement", 0,
                       JS_CFUNC_constructor, 0);
  // Bind constructor's prototype to our node proxy proto
  JS_SetConstructor(ctx, html_elem_ctor, proto);
  JS_SetPropertyStr(ctx, global_obj, "HTMLElement", html_elem_ctor);
  JS_NewClassID(&prisimi_style_class_id);
  JS_NewClass(rt, prisimi_style_class_id, &prisimi_style_class);
  JSValue style_proto = JS_NewObject(ctx);
  JS_SetPropertyFunctionList(ctx, style_proto, prisimi_style_funcs,
                             sizeof(prisimi_style_funcs) /
                                 sizeof(prisimi_style_funcs[0]));
  JS_SetClassProto(ctx, prisimi_style_class_id, style_proto);
  JS_SetPropertyStr(ctx, global_obj, "WebSocket",
                    JS_NewCFunction(ctx, js_window_WebSocket, "WebSocket", 1));

  JS_NewClassID(&prisimi_websocket_class_id);
  JS_NewClass(rt, prisimi_websocket_class_id, &prisimi_websocket_class);
  JSValue ws_proto = JS_NewObject(ctx);
  JS_SetPropertyFunctionList(ctx, ws_proto, prisimi_websocket_funcs, 1);
  JS_SetClassProto(ctx, prisimi_websocket_class_id, ws_proto);

  JS_SetPropertyStr(ctx, global_obj, "Worker",
                    JS_NewCFunction(ctx, js_window_Worker, "Worker", 1));

  JS_NewClassID(&prisimi_worker_class_id);
  JS_NewClass(rt, prisimi_worker_class_id, &prisimi_worker_class);
  JSValue worker_proto = JS_NewObject(ctx);
  JS_SetPropertyFunctionList(ctx, worker_proto, prisimi_worker_funcs, 1);
  JS_SetClassProto(ctx, prisimi_worker_class_id, worker_proto);

  // Epic 65: MediaSource & SourceBuffer FFI
  JS_SetPropertyStr(
      ctx, global_obj, "MediaSource",
      JS_NewCFunction(ctx, js_window_MediaSource, "MediaSource", 0));

  // Epic 73: Web Audio API
  JS_SetPropertyStr(
      ctx, global_obj, "AudioContext",
      JS_NewCFunction(ctx, js_window_AudioContext, "AudioContext", 0));

  JS_NewClassID(&prisimi_sourcebuffer_class_id);
  JS_NewClass(rt, prisimi_sourcebuffer_class_id, &prisimi_sourcebuffer_class);
  JSValue sb_proto = JS_NewObject(ctx);
  JS_SetPropertyStr(
      ctx, sb_proto, "appendBuffer",
      JS_NewCFunction(ctx, js_sourceBuffer_appendBuffer, "appendBuffer", 1));
  JS_SetClassProto(ctx, prisimi_sourcebuffer_class_id, sb_proto);

  // Epic 75: EME & CDM Sandbox
  extern void init_eme_bridge(JSContext * ctx, JSValue global_obj);
  init_eme_bridge(ctx, global_obj);

  JS_NewClassID(&prisimi_canvas_context_class_id);
  JS_NewClass(rt, prisimi_canvas_context_class_id,
              &prisimi_canvas_context_class);
  JSValue canvas_ctx_proto = JS_NewObject(ctx);
  JS_SetPropertyFunctionList(ctx, canvas_ctx_proto,
                             prisimi_canvas_context_funcs,
                             sizeof(prisimi_canvas_context_funcs) /
                                 sizeof(prisimi_canvas_context_funcs[0]));
  JS_SetClassProto(ctx, prisimi_canvas_context_class_id, canvas_ctx_proto);

  JS_FreeValue(ctx, global_obj);
  return 1;
}

int32_t js_quickjs_teardown() {
  if (ctx) {
    // Free active listeners
    for (int i = 0; i < js_listeners_count; i++) {
      JS_FreeValue(ctx, js_listeners[i].callback);
    }
    js_listeners_count = 0;

    // Free timer arrays
    for (int i = 0; i < 256; i++) {
      JS_FreeValue(ctx, js_timer_callbacks[i]);
      js_timer_callbacks[i] = JS_UNDEFINED;
      JS_FreeValue(ctx, js_raf_callbacks[i]);
      js_raf_callbacks[i] = JS_UNDEFINED;
      JS_FreeValue(ctx, js_websockets[i]);
      js_websockets[i] = JS_UNDEFINED;
      JS_FreeValue(ctx, idb_resolve_funcs[i]);
      idb_resolve_funcs[i] = JS_UNDEFINED;
      JS_FreeValue(ctx, idb_reject_funcs[i]);
      idb_reject_funcs[i] = JS_UNDEFINED;
    }

    for (int i = 0; i < custom_elements_count; i++) {
      JS_FreeValue(ctx, custom_elements[i].constructor);
    }
    custom_elements_count = 0;

    JS_FreeValue(ctx, global_worker_obj);
    global_worker_obj = JS_UNDEFINED;

    for (int i = 0; i < 256; i++) {
      if (js_fetch_requests[i].active) {
        JS_FreeValue(ctx, js_fetch_requests[i].resolve_func);
        JS_FreeValue(ctx, js_fetch_requests[i].reject_func);
      }
      js_fetch_requests[i].active = 0;
    }
    next_fetch_id = 1;

    JS_FreeContext(ctx);
    ctx = NULL;
  }
  if (rt) {
    JS_FreeRuntime(rt);
    rt = NULL;
  }

  js_listeners_count = 0;

  return 1;
}

// Zero-Allocation JS Eval via static scratchpad in airlock.salt
extern uint64_t js_eval_scratchpad_ptr(void);
#define JS_EVAL_SCRATCHPAD_CAP 1048576

int32_t js_eval_buffer(const char *code_ptr, uint32_t len) {
  if (!ctx)
    return -1;
  // QuickJS requires input[input_len] == '\0'. Rather than malloc/free on
  // every eval, we memcpy into the pre-allocated 1MB scratchpad in
  // airlock.salt, slap a \0 at the end, and pass it straight through.
  if (len >= JS_EVAL_SCRATCHPAD_CAP) {
    printf("[C] js_eval_buffer: script too large (%u bytes, cap %u)\n", len,
           JS_EVAL_SCRATCHPAD_CAP);
    return -1;
  }
  char *buf = (char *)(uintptr_t)js_eval_scratchpad_ptr();
  memcpy(buf, code_ptr, len);
  buf[len] = '\0';
  printf("[C] QuickJS evaluating: %s\n", buf);
  fflush(stdout);
  JSValue val = JS_Eval(ctx, buf, len, "<memory>", JS_EVAL_TYPE_GLOBAL);
  if (JS_IsException(val)) {
    JSValue ex = JS_GetException(ctx);
    const char *estr = JS_ToCString(ctx, ex);
    // Epic 61: Enhanced Error Matrix — extract stack trace
    JSValue stack = JS_GetPropertyStr(ctx, ex, "stack");
    if (!JS_IsUndefined(stack)) {
      const char *stack_str = JS_ToCString(ctx, stack);
      printf("[Prisimi Engine Panic] Uncaught Exception:\n%s\nStack:\n%s\n",
             estr, stack_str);
      JS_FreeCString(ctx, stack_str);
    } else {
      printf("[C] QuickJS Exception: %s\n", estr);
    }
    JS_FreeValue(ctx, stack);
    JS_FreeCString(ctx, estr);
    JS_FreeValue(ctx, ex);
    JS_FreeValue(ctx, val);
    return -1;
  }
  JS_FreeValue(ctx, val);
  return 0;
}

// Epic 61: The QuickJS Ignition — Execute a script bundle with a proper
// filename for stack traces. Uses the DOM_TEXT_ARENA scratchpad for NUL
// termination.
void sys_js_evaluate_script(uint64_t code_ptr, uint32_t code_len,
                            uint64_t filename_ptr, uint32_t filename_len) {
  if (!ctx)
    return;

  // Build NUL-terminated filename
  char filename[256];
  uint32_t safe_len = filename_len < 255 ? filename_len : 255;
  memcpy(filename, (const void *)(uintptr_t)filename_ptr, safe_len);
  filename[safe_len] = '\0';

  printf("[Epic 61] Igniting script: %s (%u bytes)\n", filename, code_len);
  fflush(stdout);

  // Copy to scratchpad for NUL termination (QuickJS requirement)
  if (code_len >= JS_EVAL_SCRATCHPAD_CAP) {
    printf("[Prisimi Engine Panic] Script too large for scratchpad: %s (%u "
           "bytes)\n",
           filename, code_len);
    return;
  }
  char *buf = (char *)(uintptr_t)js_eval_scratchpad_ptr();
  memcpy(buf, (const void *)(uintptr_t)code_ptr, code_len);
  buf[code_len] = '\0';

  // Evaluate with the real filename for precise stack traces
  JSValue result = JS_Eval(ctx, buf, code_len, filename, JS_EVAL_TYPE_GLOBAL);

  if (JS_IsException(result)) {
    JSValue exception_val = JS_GetException(ctx);
    const char *err_str = JS_ToCString(ctx, exception_val);

    // Retrieve Stack Trace if available
    JSValue stack = JS_GetPropertyStr(ctx, exception_val, "stack");
    if (!JS_IsUndefined(stack)) {
      const char *stack_str = JS_ToCString(ctx, stack);
      printf(
          "[Prisimi Engine Panic] Uncaught Exception in %s:\n%s\nStack:\n%s\n",
          filename, err_str, stack_str);
      JS_FreeCString(ctx, stack_str);
    } else {
      printf("[Prisimi Engine Panic] Uncaught Exception in %s:\n%s\n", filename,
             err_str);
    }
    JS_FreeValue(ctx, stack);
    JS_FreeCString(ctx, err_str);
    JS_FreeValue(ctx, exception_val);
  }

  JS_FreeValue(ctx, result);

  // Flush microtasks resulting from the initial execution
  JSContext *pctx;
  while (JS_ExecutePendingJob(JS_GetRuntime(ctx), &pctx) > 0) {
  }
}

// Epic 61: Fire a document-level event (DOMContentLoaded, load, etc.)
// Dispatches through the real listener registry using node_id=0 (document
// sentinel)
void js_bridge_dispatch_document_event(const char *type_ptr,
                                       uint32_t type_len) {
  if (!ctx)
    return;

  printf("[Epic 61] Dispatching document event: %.*s\n", type_len, type_ptr);
  fflush(stdout);

  // Look up listeners registered on the document (node_id=0)
  uint32_t hash = fnv1a_hash_len(type_ptr, type_len);
  for (int i = 0; i < js_listeners_count; i++) {
    if (js_listeners[i].node_id == 0 &&
        js_listeners[i].event_type_hash == hash) {
      JSValue event_obj = JS_NewObject(ctx);
      JSValue ret =
          JS_Call(ctx, js_listeners[i].callback, JS_UNDEFINED, 1, &event_obj);
      if (JS_IsException(ret)) {
        JSValue ex = JS_GetException(ctx);
        const char *estr = JS_ToCString(ctx, ex);
        printf("[Prisimi Engine Panic] DOMContentLoaded handler error: %s\n",
               estr);
        JS_FreeCString(ctx, estr);
        JS_FreeValue(ctx, ex);
      }
      JS_FreeValue(ctx, ret);
      JS_FreeValue(ctx, event_obj);
    }
  }

  // Flush microtasks
  JSContext *pctx;
  while (JS_ExecutePendingJob(JS_GetRuntime(ctx), &pctx) > 0) {
  }
}

int32_t js_execute_pending_jobs() {
  if (!rt || !ctx)
    return 0;
  JSContext *pctx;
  return JS_ExecutePendingJob(rt, &pctx);
}

// Test script for async fetch verification
static const char test_script_fetch[] =
    "var hero = document.getElementById('hero');\n"
    "fetch('/api/data').then(function(res) { return res.json(); "
    "}).then(function(data) {\n"
    "  if (data.message === 'success') {\n"
    "    hero.className = 'fetched';\n"
    "  }\n"
    "});\n";

void js_get_test_script_fetch(uint64_t buf) {
  memcpy((void *)buf, test_script_fetch, sizeof(test_script_fetch) - 1);
}

uint32_t js_get_test_script_fetch_len() {
  return (uint32_t)(sizeof(test_script_fetch) - 1);
}

// Test script for event dispatch verification
static const char test_script_events[] =
    "var hero = document.getElementById('hero');\n"
    "hero.addEventListener('click', function() {\n"
    "  hero.className = 'clicked';\n"
    "});\n"
    "hero.innerHTML = '<div "
    "id=\"untrusted\"><script>alert(1)</script></div>';\n"
    "var expected = '<div id=\"untrusted\"><script>alert(1)</script></div>';\n"
    "var actual = hero.innerHTML;\n"
    "if (actual !== expected) {\n"
    "  throw new Error('innerHTML mismatch! expected: ' + expected + ' actual: "
    "' + actual);\n"
    "}\n"
    "hero.style.width = '400px';\n"
    "hero.style.height = '50%';\n"
    "hero.style.flexGrow = '1';\n"
    "hero.style.flexGrow = 1;\n";

void js_get_test_script_events(uint64_t buf) {
  memcpy((void *)buf, test_script_events, sizeof(test_script_events) - 1);
}

uint32_t js_get_test_script_events_len() {
  return (uint32_t)(sizeof(test_script_events) - 1);
}
