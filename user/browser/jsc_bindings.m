#import <Foundation/Foundation.h>
#import <JavaScriptCore/JavaScriptCore.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// Google Unblock: Base64 data URI decoder
extern int32_t is_data_uri(const uint8_t *src, uint32_t len);
extern int32_t is_data_image_uri(const uint8_t *src, uint32_t len);
extern int32_t decode_data_uri(const uint8_t *src, uint32_t src_len,
                               uint8_t *dst, uint32_t dst_max);

// Google Unblock: DOM mutator externs for script fetch pipeline
extern void dom_set_script_src(uint32_t node_idx, uint64_t src_ptr,
                               uint32_t src_len);
extern uint64_t queue_script_fetch(uint64_t src_ptr, uint32_t src_len);
extern void ext_salt_invalidate_layout(uint32_t node_id);
extern void ext_salt_invalidate_all_layout(void);
extern void js_lex_html_chunk(uint64_t root_id, uint64_t ptr, uint32_t len,
                              uint8_t can_execute);

extern uint64_t ext_salt_create_node(uint32_t tag);
extern void ext_dom_free_node(uint32_t node_id);
extern uint32_t dom_get_free_list_count();
extern uint64_t dom_get_text_ptr(uint32_t idx);
extern uint32_t dom_get_text_len(uint32_t idx);
extern uint32_t dom_get_tag(uint32_t idx);
extern uint32_t dom_get_generation(uint32_t idx);
extern uint64_t resolve_node_by_id(uint64_t ptr, uint32_t len);
extern uint32_t dom_get_active_focus();
extern uint32_t ext_timers_add_timeout(uint32_t delay_ms, uint8_t is_interval);
extern void ext_timers_clear(uint32_t timer_id);
extern uint32_t ext_timers_add_raf();
extern uint8_t ext_observers_register(uint32_t node_id, uint64_t callback_ptr,
                                      uint8_t obs_type);
extern void user__browser__dom__sys_node_set_width(uint32_t node_idx,
                                                   int32_t width);

// --- DOM Manipulation Externs (Epic 80A Port) ---
extern void js_dom_append_child(uint32_t parent_idx, uint32_t child_idx);
extern void ext_dom_remove_child(uint32_t parent_idx, uint32_t child_idx);
extern void ext_dom_set_text_content(uint32_t node_idx, uint64_t str_ptr,
                                     uint32_t str_len);
extern void ext_dom_insert_before(uint32_t parent_idx, uint32_t new_idx,
                                  uint32_t ref_idx);
extern uint64_t dom_alloc_text(uint32_t len);
extern void dom_set_id(uint32_t idx, uint64_t id_ptr, uint32_t id_len);
extern void set_class(int32_t node_id, uint64_t ptr, uint32_t len);
extern uint64_t js_get_class_ptr(int32_t node_id);
extern uint32_t js_get_class_len(int32_t node_id);
extern uint64_t js_dom_get_parent(uint64_t node_id);
extern uint64_t dom_get_first_child(uint32_t idx);
extern uint64_t dom_get_next_sibling(uint32_t idx);
extern void ext_dom_set_class_name(uint32_t node_id, uint64_t str_ptr,
                                   uint32_t str_len);
extern uint64_t dom_get_class_ptr(uint32_t idx);
extern uint32_t dom_get_class_len(uint32_t idx);

JSValueRef jsc_xhr_stub_method(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef *exception);

// --- Style Externs (Epic 80A Port) ---
extern void js_set_style_bg_color(uint64_t node_id, uint8_t r, uint8_t g,
                                  uint8_t b);
extern void dom_set_style_display(uint32_t idx, uint8_t val);
extern void dom_set_style_width(uint32_t idx, int32_t val);
extern void dom_set_style_height(uint32_t idx, int32_t val);
extern void dom_set_style_position(uint32_t idx, uint8_t val);
extern void dom_set_style_top(uint32_t idx, int32_t val);
extern void dom_set_style_left(uint32_t idx, int32_t val);
extern void dom_set_style_z_index(uint32_t idx, int32_t val);
extern void dom_set_style_overflow(uint32_t idx, uint8_t val);
extern void js_set_style_opacity(uint64_t node_id, float val);

// --- Globals ---
extern JSObjectRef create_js_node_wrapper(JSContextRef ctx, uint64_t node_id);

#include <dispatch/dispatch.h>
#include <stdatomic.h>

// --- GCD Timer Flags ---
// 0 = active, 1 = cancelled
static _Atomic(uint8_t) timer_cancelled[65536];
static _Atomic(uint32_t) global_timer_id = 1;

// --- FNV-1a Hash (same as QuickJS bridge) ---
static uint32_t fnv1a_hash_str(const char *str) {
  uint32_t hash = 2166136261u;
  while (*str) {
    hash ^= (uint8_t)*str++;
    hash *= 16777619u;
  }
  return hash;
}

// --- Event Listener Registry (JSC side) ---
typedef struct {
  uint64_t node_id;
  uint32_t event_type_hash;
  JSObjectRef callback; // Protected via JSValueProtect
} JSCEventListener;

static JSCEventListener jsc_listeners[1024];
static int jsc_listeners_count = 0;

// console.log / print
JSValueRef jsc_print(JSContextRef ctx, JSObjectRef function,
                     JSObjectRef thisObject, size_t argc,
                     const JSValueRef argv[], JSValueRef *exception) {
  if (argc > 0) {
    JSStringRef str = JSValueToStringCopy(ctx, argv[0], exception);
    size_t len = JSStringGetMaximumUTF8CStringSize(str);
    char *buf = (char *)malloc(len);
    if (buf) {
      JSStringGetUTF8CString(str, buf, len);
      printf("%s\n", buf);
      free(buf);
    }
    JSStringRelease(str);
  }
  return JSValueMakeUndefined(ctx);
}

// setTimeout(callback, delay)
JSValueRef JSC_SetTimeout(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argumentCount, const JSValueRef arguments[], JSValueRef* exception) {
    if (argumentCount < 1 || !JSValueIsObject(ctx, arguments[0])) return JSValueMakeUndefined(ctx);
    
    JSObjectRef callback = JSValueToObject(ctx, arguments[0], exception);
    double delay_ms = (argumentCount > 1 && JSValueIsNumber(ctx, arguments[1])) ? JSValueToNumber(ctx, arguments[1], exception) : 0.0;
    
    uint32_t timer_id = atomic_fetch_add(&global_timer_id, 1);
    if (timer_id < 65536) atomic_store(&timer_cancelled[timer_id], 0);
    
    JSValueProtect(ctx, callback);
    
    dispatch_time_t dt = dispatch_time(DISPATCH_TIME_NOW, (int64_t)(delay_ms * NSEC_PER_MSEC));
    
    dispatch_after(dt, dispatch_get_main_queue(), ^{
        if (timer_id < 65536 && atomic_load(&timer_cancelled[timer_id]) == 1) {
            JSValueUnprotect(ctx, callback);
            return;
        }
        JSObjectCallAsFunction(ctx, callback, NULL, 0, NULL, NULL);
        JSValueUnprotect(ctx, callback);
    });
    
    return JSValueMakeNumber(ctx, timer_id);
}

JSValueRef JSC_ClearTimeout(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argumentCount, const JSValueRef arguments[], JSValueRef* exception) {
    if (argumentCount < 1 || !JSValueIsNumber(ctx, arguments[0])) return JSValueMakeUndefined(ctx);
    
    uint32_t timer_id = (uint32_t)JSValueToNumber(ctx, arguments[0], exception);
    if (timer_id < 65536) {
        atomic_store(&timer_cancelled[timer_id], 1);
    }
    
    return JSValueMakeUndefined(ctx);
}

// setInterval(callback, delay)
static void JSC_DispatchInterval(JSContextRef ctx, JSObjectRef callback, double delay_ms, uint32_t timer_id) {
    dispatch_time_t dt = dispatch_time(DISPATCH_TIME_NOW, (int64_t)(delay_ms * NSEC_PER_MSEC));
    dispatch_after(dt, dispatch_get_main_queue(), ^{
        if (timer_id < 65536 && atomic_load(&timer_cancelled[timer_id]) == 1) {
            JSValueUnprotect(ctx, callback);
            return;
        }
        JSObjectCallAsFunction(ctx, callback, NULL, 0, NULL, NULL);
        JSC_DispatchInterval(ctx, callback, delay_ms, timer_id);
    });
}

JSValueRef JSC_SetInterval(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argumentCount, const JSValueRef arguments[], JSValueRef* exception) {
    if (argumentCount < 1 || !JSValueIsObject(ctx, arguments[0])) return JSValueMakeUndefined(ctx);
    
    JSObjectRef callback = JSValueToObject(ctx, arguments[0], exception);
    double delay_ms = (argumentCount > 1 && JSValueIsNumber(ctx, arguments[1])) ? JSValueToNumber(ctx, arguments[1], exception) : 0.0;
    
    uint32_t timer_id = atomic_fetch_add(&global_timer_id, 1);
    if (timer_id < 65536) atomic_store(&timer_cancelled[timer_id], 0);
    
    JSValueProtect(ctx, callback);
    JSC_DispatchInterval(ctx, callback, delay_ms, timer_id);
    
    return JSValueMakeNumber(ctx, timer_id);
}

// requestAnimationFrame(callback)
JSValueRef JSC_RequestAnimationFrame(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argumentCount, const JSValueRef arguments[], JSValueRef* exception) {
    if (argumentCount < 1 || !JSValueIsObject(ctx, arguments[0])) return JSValueMakeUndefined(ctx);
    
    JSObjectRef callback = JSValueToObject(ctx, arguments[0], exception);
    
    uint32_t raf_id = atomic_fetch_add(&global_timer_id, 1);
    if (raf_id < 65536) atomic_store(&timer_cancelled[raf_id], 0);
    
    JSValueProtect(ctx, callback);
    
    // ~16ms (60fps) GCD alignment
    dispatch_time_t dt = dispatch_time(DISPATCH_TIME_NOW, (int64_t)(16 * NSEC_PER_MSEC));
    dispatch_after(dt, dispatch_get_main_queue(), ^{
        if (raf_id < 65536 && atomic_load(&timer_cancelled[raf_id]) == 1) {
            JSValueUnprotect(ctx, callback);
            return;
        }
        JSValueRef args[] = { JSValueMakeNumber(ctx, 0.0) };
        JSObjectCallAsFunction(ctx, callback, NULL, 1, args, NULL);
        JSValueUnprotect(ctx, callback);
    });
    
    return JSValueMakeNumber(ctx, raf_id);
}

static char *jsc_value_to_cstring(JSContextRef ctx, JSValueRef val,
                                  size_t *out_len);

typedef struct {
  uint32_t tag_hash;
  JSObjectRef constructor;
} JSCCustomElementDef;
static JSCCustomElementDef jsc_custom_elements[64];
static int jsc_custom_elements_count = 0;

// Epic 84: Upgrade Node Stack — supports nested constructor calls during CE
// upgrade. Enterprise SPAs (e.g. YouTube) may synchronously createElement
// inside a constructor before super() resolves. A single variable would be
// overwritten; a stack gives LIFO safety.
#define UPGRADE_STACK_DEPTH 32
static uint64_t UPGRADE_NODE_STACK[UPGRADE_STACK_DEPTH];
static uint8_t UPGRADE_STACK_PTR = 0;

static void upgrade_stack_push(uint64_t node_id) {
  if (UPGRADE_STACK_PTR < UPGRADE_STACK_DEPTH) {
    UPGRADE_NODE_STACK[UPGRADE_STACK_PTR++] = node_id;
  }
}

static uint64_t upgrade_stack_pop(void) {
  if (UPGRADE_STACK_PTR > 0) {
    return UPGRADE_NODE_STACK[--UPGRADE_STACK_PTR];
  }
  return 0;
}

JSValueRef jsc_document_createElement(JSContextRef ctx, JSObjectRef function,
                                      JSObjectRef thisObject, size_t argc,
                                      const JSValueRef argv[],
                                      JSValueRef *exception) {
  if (argc < 1)
    return JSValueMakeNull(ctx);

  size_t tag_len;
  char *tag = jsc_value_to_cstring(ctx, argv[0], &tag_len);
  if (!tag)
    return JSValueMakeNull(ctx);

  for (int i = 0; tag[i]; i++) {
    if (tag[i] >= 'a' && tag[i] <= 'z')
      tag[i] -= 32;
  }

  // Google Unblock: Expanded tag recognition for dynamic DOM construction
  uint32_t tag_id = 4; // Default to DIV
  if (strcmp(tag, "DIV") == 0)
    tag_id = 4;
  else if (strcmp(tag, "SPAN") == 0)
    tag_id = 5;
  else if (strcmp(tag, "P") == 0)
    tag_id = 6;
  else if (strcmp(tag, "A") == 0)
    tag_id = 7;
  else if (strcmp(tag, "IMG") == 0)
    tag_id = 8;
  else if (strcmp(tag, "H1") == 0)
    tag_id = 9;
  else if (strcmp(tag, "TABLE") == 0)
    tag_id = 12;
  else if (strcmp(tag, "TR") == 0)
    tag_id = 13;
  else if (strcmp(tag, "TD") == 0)
    tag_id = 14;
  else if (strcmp(tag, "B") == 0)
    tag_id = 15;
  else if (strcmp(tag, "I") == 0)
    tag_id = 16;
  else if (strcmp(tag, "INPUT") == 0)
    tag_id = 18;
  else if (strcmp(tag, "TEXTAREA") == 0)
    tag_id = 19;
  else if (strcmp(tag, "BUTTON") == 0)
    tag_id = 20;
  else if (strcmp(tag, "FORM") == 0)
    tag_id = 21;
  else if (strcmp(tag, "VIDEO") == 0)
    tag_id = 25;
  else if (strcmp(tag, "IFRAME") == 0)
    tag_id = 26;
  else if (strcmp(tag, "CANVAS") == 0)
    tag_id = 27;
  else if (strcmp(tag, "STYLE") == 0)
    tag_id = 98;
  else if (strcmp(tag, "SCRIPT") == 0)
    tag_id = 99;

  extern void ext_dom_set_custom_tag(uint32_t node_idx, uint32_t hash,
                                     uint64_t ptr, uint32_t len);

  // Check if it's a custom element via the registry first
  // Use the lowercase tag for hashing since JS defines custom elements in
  // lower-case
  char lc_tag[256] = {0};
  for (int i = 0; i < tag_len && i < 255; i++) {
    lc_tag[i] = tag[i] >= 'A' && tag[i] <= 'Z' ? tag[i] + 32 : tag[i];
  }
  uint32_t ce_hash = fnv1a_hash_str(lc_tag);

  JSObjectRef constructor = NULL;
  for (int i = 0; i < jsc_custom_elements_count; i++) {
    if (jsc_custom_elements[i].tag_hash == ce_hash) {
      constructor = jsc_custom_elements[i].constructor;
      tag_id = 96; // TAG_CUSTOM_ELEMENT
      break;
    }
  }

  if (tag_id == 4 && strchr(tag, '-')) {
    tag_id = 96; // Hyphenated tag without constructor, still TAG_CUSTOM_ELEMENT
  }

  uint64_t node_id = ext_salt_create_node(tag_id);
  if (node_id == 0) {
    free(tag);
    return JSValueMakeNull(ctx);
  }

  if (tag_id == 96) {
    // Must store custom tag! We can allocate a string via malloc to persist it?
    // No, we cannot easily persist malloc'd strings to the Rust ring without
    // using the text arena! For createElement, we'll store a hardcoded 0
    // ptr/len. It will still work via hash.
    ext_dom_set_custom_tag((uint32_t)(node_id & 0xFFFF), ce_hash, 0, 0);

    if (constructor) {
      upgrade_stack_push(node_id);
      JSValueRef ex = NULL;
      JSObjectRef instance =
          JSObjectCallAsConstructor(ctx, constructor, 0, NULL, &ex);
      // Drain unconsumed token if constructor was malformed
      if (UPGRADE_STACK_PTR > 0 &&
          UPGRADE_NODE_STACK[UPGRADE_STACK_PTR - 1] == node_id) {
        UPGRADE_STACK_PTR--;
      }
      free(tag);

      // Mark it upgraded natively
      extern void dom_set_node_upgraded(uint32_t idx, uint8_t upgraded);
      dom_set_node_upgraded((uint32_t)(node_id & 0xFFFF), 1);

      return instance ? instance : JSValueMakeNull(ctx);
    }
  }

  free(tag);
  return create_js_node_wrapper(ctx, node_id);
}

// document.getElementById(id)
static char jsc_document_cookie[8192] = "";

JSValueRef jsc_document_get_cookie(JSContextRef ctx, JSObjectRef object, JSStringRef pn, JSValueRef *ex) {
  return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(jsc_document_cookie));
}

bool jsc_document_set_cookie(JSContextRef ctx, JSObjectRef object, JSStringRef pn, JSValueRef value, JSValueRef *ex) {
  size_t len;
  char *str = jsc_value_to_cstring(ctx, value, &len);
  if (str) {
      // Append or replace cookie (very basic implementation)
      // Real browsers parse key=value and handle expires/path/domain
      if (strlen(jsc_document_cookie) + len + 2 < 8192) {
          if (strlen(jsc_document_cookie) > 0) strcat(jsc_document_cookie, "; ");
          strcat(jsc_document_cookie, str);
      }
      free(str);
  }
  return true;
}

JSValueRef jsc_navigator_sendBeacon(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef *exception) {
  return JSValueMakeBoolean(ctx, true);
}

JSObjectRef jsc_TextEncoder_constructor(JSContextRef ctx, JSObjectRef constructor, size_t argc, const JSValueRef argv[], JSValueRef *exception) {
  JSObjectRef te = JSObjectMake(ctx, NULL, NULL);
  JSStringRef encStr = JSStringCreateWithUTF8CString("encode");
  JSObjectSetProperty(ctx, te, encStr, JSObjectMakeFunctionWithCallback(ctx, encStr, jsc_xhr_stub_method), kJSPropertyAttributeNone, NULL);
  JSStringRelease(encStr);
  return te;
}

JSObjectRef jsc_TextDecoder_constructor(JSContextRef ctx, JSObjectRef constructor, size_t argc, const JSValueRef argv[], JSValueRef *exception) {
  JSObjectRef td = JSObjectMake(ctx, NULL, NULL);
  JSStringRef decStr = JSStringCreateWithUTF8CString("decode");
  JSObjectSetProperty(ctx, td, decStr, JSObjectMakeFunctionWithCallback(ctx, decStr, jsc_xhr_stub_method), kJSPropertyAttributeNone, NULL);
  JSStringRelease(decStr);
  return td;
}

JSObjectRef jsc_URLSearchParams_constructor(JSContextRef ctx, JSObjectRef constructor, size_t argc, const JSValueRef argv[], JSValueRef *exception) {
  JSObjectRef usp = JSObjectMake(ctx, NULL, NULL);
  const char* methods[] = {"get", "set", "has", "append", "delete", "entries", "forEach", "keys", "values"};
  for (int i = 0; i < 9; i++) {
    JSStringRef mStr = JSStringCreateWithUTF8CString(methods[i]);
    JSObjectSetProperty(ctx, usp, mStr, JSObjectMakeFunctionWithCallback(ctx, mStr, jsc_xhr_stub_method), kJSPropertyAttributeNone, NULL);
    JSStringRelease(mStr);
  }
  return usp;
}

static char jsc_doc_title[256] = "Prisimi Browser";

JSValueRef jsc_document_get_title(JSContextRef ctx, JSObjectRef object, JSStringRef pn, JSValueRef *ex) {
  return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(jsc_doc_title));
}

bool jsc_document_set_title(JSContextRef ctx, JSObjectRef object, JSStringRef pn, JSValueRef value, JSValueRef *ex) {
  size_t len;
  char *str = jsc_value_to_cstring(ctx, value, &len);
  if (str) {
    strncpy(jsc_doc_title, str, 255);
    jsc_doc_title[255] = '\0';
    free(str);
    // In a real browser we'd update the window title here
  }
  return true;
}

JSValueRef jsc_document_get_activeElement(JSContextRef ctx, JSObjectRef object, JSStringRef pn, JSValueRef *ex) {
  extern uint32_t dom_get_active_focus(void);
  uint32_t active = dom_get_active_focus();
  if (active == 0) return JSValueMakeNull(ctx);
  extern JSObjectRef create_js_node_wrapper(JSContextRef ctx, uint64_t node_id);
  return create_js_node_wrapper(ctx, active);
}

JSValueRef jsc_document_get_readyState(JSContextRef ctx, JSObjectRef object, JSStringRef pn, JSValueRef *ex) {
  return JSValueMakeString(ctx, JSStringCreateWithUTF8CString("complete"));
}

JSValueRef jsc_document_get_referrer(JSContextRef ctx, JSObjectRef object, JSStringRef pn, JSValueRef *ex) {
  return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(""));
}

JSValueRef jsc_window_get_devicePixelRatio(JSContextRef ctx, JSObjectRef object, JSStringRef pn, JSValueRef *ex) {
  return JSValueMakeNumber(ctx, 2.0);
}

JSValueRef jsc_document_getElementById(JSContextRef ctx, JSObjectRef function,
                                       JSObjectRef thisObject, size_t argc,
                                       const JSValueRef argv[],
                                       JSValueRef *exception) {
  if (argc < 1)
    return JSValueMakeNull(ctx);

  JSStringRef idStr = JSValueToStringCopy(ctx, argv[0], exception);
  size_t len = JSStringGetMaximumUTF8CStringSize(idStr);
  char *buf = (char *)malloc(len);
  if (!buf) {
    JSStringRelease(idStr);
    return JSValueMakeNull(ctx);
  }
  JSStringGetUTF8CString(idStr, buf, len);

  uint64_t node_id =
      resolve_node_by_id((uint64_t)(uintptr_t)buf, (uint32_t)strlen(buf));

  free(buf);
  JSStringRelease(idStr);

  if (node_id == 0)
    return JSValueMakeNull(ctx);
  return create_js_node_wrapper(ctx, node_id);
}

extern uint64_t sys_dom_query_selector(uint32_t root_node_id,
                                       uint64_t selector_str_ptr,
                                       uint32_t selector_len);

JSValueRef jsc_document_querySelector(JSContextRef ctx, JSObjectRef function,
                                      JSObjectRef thisObject, size_t argc,
                                      const JSValueRef argv[],
                                      JSValueRef *exception) {
  if (argc < 1)
    return JSValueMakeNull(ctx);
  size_t sel_len;
  char *sel = jsc_value_to_cstring(ctx, argv[0], &sel_len);
  if (!sel)
    return JSValueMakeNull(ctx);

  // We pass 1 (document.body) as the root. Actually, standard querySelector is
  // from document, which contains html, head, body. For Prisimi, body is
  // node_id 1.
  uint64_t node_id = sys_dom_query_selector(1, (uint64_t)(uintptr_t)sel,
                                            (uint32_t)strlen(sel));
  free(sel);

  if (node_id == 0)
    return JSValueMakeNull(ctx);
  return create_js_node_wrapper(ctx, node_id);
}

JSValueRef jsc_document_querySelectorAll(JSContextRef ctx, JSObjectRef function,
                                         JSObjectRef thisObject, size_t argc,
                                         const JSValueRef argv[],
                                         JSValueRef *exception) {
  return JSObjectMakeArray(ctx, 0, NULL, exception);
}

// sys_observers_register(node_index, callback, type)
JSValueRef jsc_observers_register(JSContextRef ctx, JSObjectRef function,
                                  JSObjectRef thisObject, size_t argc,
                                  const JSValueRef argv[],
                                  JSValueRef *exception) {
  if (argc < 3)
    return JSValueMakeUndefined(ctx);

  uint32_t node_idx = (uint32_t)JSValueToNumber(ctx, argv[0], exception);
  JSObjectRef callback = JSValueToObject(ctx, argv[1], exception);
  uint32_t obs_type = (uint32_t)JSValueToNumber(ctx, argv[2], exception);

  uint8_t success = ext_observers_register(
      node_idx, (uint64_t)(uintptr_t)callback, (uint8_t)obs_type);
  if (success) {
    JSValueProtect(ctx, callback);
  }

  return JSValueMakeUndefined(ctx);
}

// sys_node_set_width(index, width)
JSValueRef jsc_node_set_width(JSContextRef ctx, JSObjectRef function,
                              JSObjectRef thisObject, size_t argc,
                              const JSValueRef argv[], JSValueRef *exception) {
  if (argc < 2)
    return JSValueMakeUndefined(ctx);
  uint32_t idx = (uint32_t)JSValueToNumber(ctx, argv[0], exception);
  int32_t w = (int32_t)JSValueToNumber(ctx, argv[1], exception);
  user__browser__dom__sys_node_set_width(idx, w);
  return JSValueMakeUndefined(ctx);
}

// getFreeNodeCount()
JSValueRef jsc_get_free_node_count(JSContextRef ctx, JSObjectRef function,
                                   JSObjectRef thisObject, size_t argc,
                                   const JSValueRef argv[],
                                   JSValueRef *exception) {
  return JSValueMakeNumber(ctx, (double)dom_get_free_list_count());
}

// --- DOM Property Accessors ---

JSValueRef get_node_text_content(JSContextRef ctx, JSObjectRef object,
                                 JSStringRef propertyName,
                                 JSValueRef *exception) {
  uint64_t node_id_packed = (uint64_t)(uintptr_t)JSObjectGetPrivate(object);
  uint32_t node_idx = (uint32_t)(node_id_packed & 0xFFFF);

  uint64_t text_ptr = dom_get_text_ptr(node_idx);
  uint32_t text_len = dom_get_text_len(node_idx);

  if (text_ptr == 0)
    return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(""));

  char *buf = (char *)malloc(text_len + 1);
  if (!buf)
    return JSValueMakeNull(ctx);
  memcpy(buf, (void *)(uintptr_t)text_ptr, text_len);
  buf[text_len] = '\0';

  JSStringRef jsStr = JSStringCreateWithUTF8CString(buf);
  JSValueRef res = JSValueMakeString(ctx, jsStr);
  JSStringRelease(jsStr);
  free(buf);
  return res;
}

bool set_node_text_content(JSContextRef ctx, JSObjectRef object,
                           JSStringRef propertyName, JSValueRef value,
                           JSValueRef *exception) {
  // Phase 2 placeholder: text mutation requires Salt bridge
  return true;
}

JSValueRef get_node_type(JSContextRef ctx, JSObjectRef object,
                         JSStringRef propertyName, JSValueRef *exception) {
  uint64_t node_id_packed = (uint64_t)(uintptr_t)JSObjectGetPrivate(object);
  uint32_t node_idx = (uint32_t)(node_id_packed & 0xFFFF);
  uint32_t tag = dom_get_tag(node_idx);
  return JSValueMakeNumber(ctx, (tag == 0) ? 3 : 1);
}

JSValueRef get_node_tag_name(JSContextRef ctx, JSObjectRef object,
                             JSStringRef propertyName, JSValueRef *exception) {
  uint64_t node_id_packed = (uint64_t)(uintptr_t)JSObjectGetPrivate(object);
  uint32_t node_idx = (uint32_t)(node_id_packed & 0xFFFF);
  uint32_t tag = dom_get_tag(node_idx);

  const char *name = "DIV";
  if (tag == fnv1a_hash_str("test-widget")) {
    name = "TEST-WIDGET";
  } else {
    switch (tag) {
    case 1:
      name = "HTML";
      break;
    case 2:
      name = "HEAD";
      break;
    case 3:
      name = "BODY";
      break;
    case 4:
      name = "DIV";
      break;
    case 5:
      name = "SPAN";
      break;
    case 6:
      name = "P";
      break;
    case 99:
      name = "SCRIPT";
      break;
    }
  }
  JSStringRef jsStr = JSStringCreateWithUTF8CString(name);
  JSValueRef res = JSValueMakeString(ctx, jsStr);
  JSStringRelease(jsStr);
  return res;
}

// performance.now()
JSValueRef jsc_performance_now(JSContextRef ctx, JSObjectRef function,
                               JSObjectRef thisObject, size_t argc,
                               const JSValueRef argv[], JSValueRef *exception) {
  extern uint64_t salt_clock_now();
  return JSValueMakeNumber(ctx, (double)salt_clock_now() / 1000000.0);
}

// gc()
JSValueRef jsc_gc(JSContextRef ctx, JSObjectRef function,
                  JSObjectRef thisObject, size_t argc, const JSValueRef argv[],
                  JSValueRef *exception) {
  JSGarbageCollect(ctx);
  return JSValueMakeUndefined(ctx);
}

// =====================================================================
// Google Unblock: document.write / document.writeln
// Google recursively bootstraps its UI via document.write('<script ...>').
// This intercepts the payload string, persists it in the DOM text arena,
// and feeds it to the HTML lexer under document.body as insertion root.
// =====================================================================
JSValueRef jsc_document_write(JSContextRef ctx, JSObjectRef function,
                              JSObjectRef thisObject, size_t argc,
                              const JSValueRef argv[], JSValueRef *exception) {
  if (argc < 1)
    return JSValueMakeUndefined(ctx);

  JSStringRef str = JSValueToStringCopy(ctx, argv[0], exception);
  if (!str)
    return JSValueMakeUndefined(ctx);

  size_t maxBuffer = JSStringGetMaximumUTF8CStringSize(str);
  char *buffer = (char *)malloc(maxBuffer);
  if (!buffer) {
    JSStringRelease(str);
    return JSValueMakeUndefined(ctx);
  }
  size_t actualLen = JSStringGetUTF8CString(str, buffer, maxBuffer);
  JSStringRelease(str);

  if (actualLen > 1) { // JSC includes null terminator in count
    uint32_t html_len = (uint32_t)(actualLen - 1);

    // Persist into DOM text arena so the lexer can read it safely
    uint64_t safe_ptr = dom_alloc_text(html_len);
    if (safe_ptr != 0) {
      memcpy((void *)(uintptr_t)safe_ptr, buffer, html_len);
    } else {
      safe_ptr = (uint64_t)(uintptr_t)buffer; // Fallback to malloc'd buffer
    }

    // Insert under document.body (node_id 1 by Prisimi convention)
    // can_execute=1 so nested <script> tags are queued for execution
    js_lex_html_chunk(1, safe_ptr, html_len, 1);

    // Mark entire tree dirty for re-layout
    ext_salt_invalidate_all_layout();
  }

  free(buffer);
  return JSValueMakeUndefined(ctx);
}

// --- Initialization ---

// =====================================================================
// Epic 80A: DOM Method Implementations (JSC port of QuickJS bridge)
// Each of these is a JSStaticFunction on the Node class.
// =====================================================================

// Helper: extract node index from JSObjectRef private data
static uint32_t jsc_get_node_idx(JSObjectRef obj) {
  uint64_t packed = (uint64_t)(uintptr_t)JSObjectGetPrivate(obj);
  return (uint32_t)(packed & 0xFFFF);
}

// Helper: extract string from JSValueRef, returns malloc'd C string + length.
// Caller MUST free the returned pointer.
static char *jsc_value_to_cstring(JSContextRef ctx, JSValueRef val,
                                  size_t *out_len) {
  JSStringRef str = JSValueToStringCopy(ctx, val, NULL);
  if (!str) {
    *out_len = 0;
    return NULL;
  }
  size_t maxLen = JSStringGetMaximumUTF8CStringSize(str);
  char *buf = (char *)malloc(maxLen);
  size_t actualLen = JSStringGetUTF8CString(str, buf, maxLen);
  JSStringRelease(str);
  *out_len = actualLen > 0 ? actualLen - 1
                           : 0; // JSC includes null terminator in count
  return buf;
}

extern void ext_events_register(uint32_t node_id, uint32_t type_hash,
                                uint64_t callback_ptr, uint8_t use_capture);
extern void ext_events_remove(uint32_t node_id, uint32_t type_hash,
                              uint64_t callback_ptr, uint8_t use_capture);

// ---- addEventListener(type, callback, options) ----
JSValueRef jsc_node_addEventListener(JSContextRef ctx, JSObjectRef function,
                                     JSObjectRef thisObject, size_t argc,
                                     const JSValueRef argv[],
                                     JSValueRef *exception) {
  if (argc < 2)
    return JSValueMakeUndefined(ctx);
  uint32_t node_idx = jsc_get_node_idx(thisObject);

  size_t type_len;
  char *type = jsc_value_to_cstring(ctx, argv[0], &type_len);
  if (!type)
    return JSValueMakeUndefined(ctx);
  uint32_t hash = fnv1a_hash_str(type);
  free(type);

  JSObjectRef callback = JSValueToObject(ctx, argv[1], exception);
  if (!callback)
    return JSValueMakeUndefined(ctx);

  uint8_t use_capture = 0;
  if (argc > 2) {
    if (JSValueIsBoolean(ctx, argv[2])) {
      use_capture = JSValueToBoolean(ctx, argv[2]) ? 1 : 0;
    } else if (JSValueIsObject(ctx, argv[2])) {
      JSStringRef cap_str = JSStringCreateWithUTF8CString("capture");
      JSValueRef cap_val = JSObjectGetProperty(
          ctx, JSValueToObject(ctx, argv[2], NULL), cap_str, NULL);
      if (JSValueIsBoolean(ctx, cap_val)) {
        use_capture = JSValueToBoolean(ctx, cap_val) ? 1 : 0;
      }
      JSStringRelease(cap_str);
    }
  }

  JSValueProtect(ctx, callback);
  ext_events_register(node_idx, hash, (uint64_t)callback, use_capture);
  return JSValueMakeUndefined(ctx);
}

// ---- removeEventListener(type, callback, options) ----
JSValueRef jsc_node_removeEventListener(JSContextRef ctx, JSObjectRef function,
                                        JSObjectRef thisObject, size_t argc,
                                        const JSValueRef argv[],
                                        JSValueRef *exception) {
  if (argc < 2)
    return JSValueMakeUndefined(ctx);
  uint32_t node_idx = jsc_get_node_idx(thisObject);

  size_t type_len;
  char *type = jsc_value_to_cstring(ctx, argv[0], &type_len);
  if (!type)
    return JSValueMakeUndefined(ctx);
  uint32_t hash = fnv1a_hash_str(type);
  free(type);

  JSObjectRef callback = JSValueToObject(ctx, argv[1], exception);
  if (!callback)
    return JSValueMakeUndefined(ctx);

  uint8_t use_capture = 0;
  if (argc > 2) {
    if (JSValueIsBoolean(ctx, argv[2])) {
      use_capture = JSValueToBoolean(ctx, argv[2]) ? 1 : 0;
    } else if (JSValueIsObject(ctx, argv[2])) {
      JSStringRef cap_str = JSStringCreateWithUTF8CString("capture");
      JSValueRef cap_val = JSObjectGetProperty(
          ctx, JSValueToObject(ctx, argv[2], NULL), cap_str, NULL);
      if (JSValueIsBoolean(ctx, cap_val)) {
        use_capture = JSValueToBoolean(ctx, cap_val) ? 1 : 0;
      }
      JSStringRelease(cap_str);
    }
  }

  ext_events_remove(node_idx, hash, (uint64_t)callback, use_capture);
  JSValueUnprotect(ctx, callback);
  return JSValueMakeUndefined(ctx);
}

// ---- dispatchEvent(event) ----
JSValueRef jsc_node_dispatchEvent(JSContextRef ctx, JSObjectRef function,
                                  JSObjectRef thisObject, size_t argc,
                                  const JSValueRef argv[],
                                  JSValueRef *exception) {
  if (argc < 1)
    return JSValueMakeBoolean(ctx, false);
  uint32_t node_idx = jsc_get_node_idx(thisObject);

  // Extract event.type string
  JSObjectRef event_obj = JSValueToObject(ctx, argv[0], exception);
  if (!event_obj)
    return JSValueMakeBoolean(ctx, false);

  JSStringRef typeProp = JSStringCreateWithUTF8CString("type");
  JSValueRef typeVal = JSObjectGetProperty(ctx, event_obj, typeProp, NULL);
  JSStringRelease(typeProp);

  size_t type_len;
  char *type = jsc_value_to_cstring(ctx, typeVal, &type_len);
  if (!type)
    return JSValueMakeBoolean(ctx, false);
  uint32_t hash = fnv1a_hash_str(type);
  free(type);

  // Fire all matching listeners
  for (int i = 0; i < jsc_listeners_count; i++) {
    if (jsc_listeners[i].node_id == (uint64_t)node_idx &&
        jsc_listeners[i].event_type_hash == hash) {
      JSValueRef args[1] = {argv[0]};
      JSValueRef ex = NULL;
      JSObjectCallAsFunction(ctx, jsc_listeners[i].callback, thisObject, 1,
                             args, &ex);
      if (ex) {
        extern void sys_jsc_dump_exception(JSContextRef ctx,
                                           JSValueRef exception);
        sys_jsc_dump_exception(ctx, ex);
      }
    }
  }
  return JSValueMakeBoolean(ctx, true);
}

// ---- setAttribute(key, value) ----
JSValueRef jsc_node_setAttribute(JSContextRef ctx, JSObjectRef function,
                                 JSObjectRef thisObject, size_t argc,
                                 const JSValueRef argv[],
                                 JSValueRef *exception) {
  if (argc < 2)
    return JSValueMakeUndefined(ctx);
  uint32_t n_idx = jsc_get_node_idx(thisObject);
  if (n_idx == 0 || n_idx >= 65536)
    return JSValueMakeUndefined(ctx);

  size_t key_len, val_len;
  char *key = jsc_value_to_cstring(ctx, argv[0], &key_len);
  char *val = jsc_value_to_cstring(ctx, argv[1], &val_len);
  if (!key || !val) {
    if (key)
      free(key);
    if (val)
      free(val);
    return JSValueMakeUndefined(ctx);
  }

  // Allocate val into DOM arena for persistence across GC
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
  } else if (key_len == 3 && memcmp(key, "src", 3) == 0) {
    // Google Unblock: Track src on <script> for dynamic fetch
    uint32_t node_tag = dom_get_tag(n_idx);
    if (node_tag == 99) { // TAG_SCRIPT
      dom_set_script_src(n_idx, safe_val, (uint32_t)val_len);
    }
    // For <img>, handle data:image/ URIs
    if (node_tag == 8) { // TAG_IMG
      if (is_data_image_uri((const uint8_t *)(uintptr_t)safe_val,
                            (uint32_t)val_len)) {
        // Decode inline Base64 and mark as loaded
        static uint8_t data_uri_decode_buf[1048576]; // 1MB scratch
        int32_t decoded_len = decode_data_uri(
            (const uint8_t *)(uintptr_t)safe_val, (uint32_t)val_len,
            data_uri_decode_buf, sizeof(data_uri_decode_buf));
        if (decoded_len > 0) {
          extern uint8_t user__browser__dom__DOM_NODE_FETCH_STATE[];
          user__browser__dom__DOM_NODE_FETCH_STATE[n_idx] = 3; // DECODED
          // Persist decoded bytes in arena
          uint64_t img_ptr = dom_alloc_text((uint32_t)decoded_len);
          if (img_ptr != 0) {
            memcpy((void *)(uintptr_t)img_ptr, data_uri_decode_buf,
                   (size_t)decoded_len);
          }
        }
      }
    }
  }
  // For generic data-* attributes, store in the generic attr SoA arrays
  // (referenced by dom.salt ATTR_NODE_ID / ATTR_KEY_PTR / ATTR_VAL_PTR)
  // Store key+val in arena for retrieval by getAttribute
  {
    extern uint32_t user__browser__dom__ATTR_COUNT;
    extern uint64_t user__browser__dom__ATTR_NODE_ID[];
    extern uint64_t user__browser__dom__ATTR_KEY_PTR[];
    extern uint32_t user__browser__dom__ATTR_KEY_LEN[];
    extern uint64_t user__browser__dom__ATTR_VAL_PTR[];
    extern uint32_t user__browser__dom__ATTR_VAL_LEN[];

    // Allocate key in arena too
    uint64_t safe_key = dom_alloc_text((uint32_t)key_len);
    if (safe_key != 0) {
      memcpy((void *)(uintptr_t)safe_key, key, key_len);
    }

    uint32_t idx = user__browser__dom__ATTR_COUNT;
    // Check for existing attr with same node+key, update in place
    for (uint32_t i = 0; i < idx; i++) {
      if (user__browser__dom__ATTR_NODE_ID[i] == (uint64_t)n_idx &&
          user__browser__dom__ATTR_KEY_LEN[i] == (uint32_t)key_len) {
        char *existing_key =
            (char *)(uintptr_t)user__browser__dom__ATTR_KEY_PTR[i];
        if (existing_key && memcmp(existing_key, key, key_len) == 0) {
          user__browser__dom__ATTR_VAL_PTR[i] = safe_val;
          user__browser__dom__ATTR_VAL_LEN[i] = (uint32_t)val_len;
          free(key);
          free(val);
          return JSValueMakeUndefined(ctx);
        }
      }
    }

    if (idx < 262144) {
      user__browser__dom__ATTR_NODE_ID[idx] = (uint64_t)n_idx;
      user__browser__dom__ATTR_KEY_PTR[idx] = safe_key;
      user__browser__dom__ATTR_KEY_LEN[idx] = (uint32_t)key_len;
      user__browser__dom__ATTR_VAL_PTR[idx] = safe_val;
      user__browser__dom__ATTR_VAL_LEN[idx] = (uint32_t)val_len;
      user__browser__dom__ATTR_COUNT = idx + 1;
    }
  }

  free(key);
  free(val);
  return JSValueMakeUndefined(ctx);
}

// ---- getAttribute(key) ----
JSValueRef jsc_node_getAttribute(JSContextRef ctx, JSObjectRef function,
                                 JSObjectRef thisObject, size_t argc,
                                 const JSValueRef argv[],
                                 JSValueRef *exception) {
  if (argc < 1)
    return JSValueMakeNull(ctx);
  uint32_t n_idx = jsc_get_node_idx(thisObject);

  size_t key_len;
  char *key = jsc_value_to_cstring(ctx, argv[0], &key_len);
  if (!key)
    return JSValueMakeNull(ctx);

  // Search generic attributes
  extern uint32_t user__browser__dom__ATTR_COUNT;
  extern uint64_t user__browser__dom__ATTR_NODE_ID[];
  extern uint64_t user__browser__dom__ATTR_KEY_PTR[];
  extern uint32_t user__browser__dom__ATTR_KEY_LEN[];
  extern uint64_t user__browser__dom__ATTR_VAL_PTR[];
  extern uint32_t user__browser__dom__ATTR_VAL_LEN[];

  for (uint32_t i = 0; i < user__browser__dom__ATTR_COUNT; i++) {
    if (user__browser__dom__ATTR_NODE_ID[i] == (uint64_t)n_idx &&
        user__browser__dom__ATTR_KEY_LEN[i] == (uint32_t)key_len) {
      char *existing_key =
          (char *)(uintptr_t)user__browser__dom__ATTR_KEY_PTR[i];
      if (existing_key && memcmp(existing_key, key, key_len) == 0) {
        char *val = (char *)(uintptr_t)user__browser__dom__ATTR_VAL_PTR[i];
        uint32_t val_len = user__browser__dom__ATTR_VAL_LEN[i];
        free(key);
        // Create JSC string from arena pointer
        char *buf = (char *)malloc(val_len + 1);
        memcpy(buf, val, val_len);
        buf[val_len] = '\0';
        JSStringRef jsStr = JSStringCreateWithUTF8CString(buf);
        JSValueRef result = JSValueMakeString(ctx, jsStr);
        JSStringRelease(jsStr);
        free(buf);
        return result;
      }
    }
  }

  free(key);
  return JSValueMakeNull(ctx);
}

// ---- appendChild(child) ----
// Google Unblock: Hardened with script-fetch detection.
// When a <script> with a src attribute is dynamically appended,
// queue the URL for network fetch + execution.
JSValueRef jsc_node_appendChild(JSContextRef ctx, JSObjectRef function,
                                JSObjectRef thisObject, size_t argc,
                                const JSValueRef argv[],
                                JSValueRef *exception) {
  if (argc < 1)
    return JSValueMakeUndefined(ctx);
  uint32_t parent_idx = jsc_get_node_idx(thisObject);

  JSObjectRef child_obj = JSValueToObject(ctx, argv[0], exception);
  if (!child_obj)
    return JSValueMakeUndefined(ctx);
  uint32_t child_idx = jsc_get_node_idx(child_obj);

  if (parent_idx == 0 || child_idx == 0)
    return JSValueMakeUndefined(ctx);
  js_dom_append_child(parent_idx, child_idx);

  // Google Unblock: Detect dynamic <script src="..."> insertion
  uint32_t child_tag = dom_get_tag(child_idx);
  if (child_tag == 99) { // TAG_SCRIPT
    extern uint64_t dom_get_script_src_ptr(uint32_t idx);
    extern uint32_t dom_get_script_src_len(uint32_t idx);
    uint64_t src_ptr = dom_get_script_src_ptr(child_idx);
    uint32_t src_len = dom_get_script_src_len(child_idx);
    if (src_ptr != 0 && src_len > 0) {
      // Queue network fetch for this script
      uint64_t fetch_id = queue_script_fetch(src_ptr, src_len);
      if (fetch_id != 0) {
        extern void sys_ipc_send_r2m_command_with_payload(
            uint32_t cmd_type, uint64_t arg1, uint64_t p_ptr, uint32_t p_len);
        // Set bit 60 to mark as script fetch (matches lexer.salt convention)
        uint64_t script_bit = ((uint64_t)1) << 60;
        uint64_t multiplexed_id = script_bit | fetch_id;
        sys_ipc_send_r2m_command_with_payload(12 /* CMD_FETCH_REQUEST */,
                                              multiplexed_id, src_ptr, src_len);
      }
    }
  }

  // Trigger layout invalidation for the parent subtree
  ext_salt_invalidate_layout(parent_idx);

  return argv[0]; // appendChild returns the appended child
}

// ---- removeChild(child) ----
JSValueRef jsc_node_removeChild(JSContextRef ctx, JSObjectRef function,
                                JSObjectRef thisObject, size_t argc,
                                const JSValueRef argv[],
                                JSValueRef *exception) {
  if (argc < 1)
    return JSValueMakeUndefined(ctx);
  uint32_t parent_idx = jsc_get_node_idx(thisObject);

  JSObjectRef child_obj = JSValueToObject(ctx, argv[0], exception);
  if (!child_obj)
    return JSValueMakeUndefined(ctx);
  uint32_t child_idx = jsc_get_node_idx(child_obj);

  if (parent_idx == 0 || child_idx == 0)
    return JSValueMakeUndefined(ctx);
  ext_dom_remove_child(parent_idx, child_idx);
  return argv[0];
}

// ---- innerHTML setter (via textContent for now — full HTML parse in Phase 2)
// ----
bool set_node_innerHTML(JSContextRef ctx, JSObjectRef object,
                        JSStringRef propertyName, JSValueRef value,
                        JSValueRef *exception) {
  uint32_t n_idx = jsc_get_node_idx(object);
  if (n_idx == 0)
    return false;

  size_t len;
  char *str = jsc_value_to_cstring(ctx, value, &len);
  if (!str)
    return false;

  // Allocate in arena and set as text content (simplified — full HTML parse
  // TBD)
  uint64_t safe_ptr = dom_alloc_text((uint32_t)len);
  if (safe_ptr != 0) {
    memcpy((void *)(uintptr_t)safe_ptr, str, len);
  }
  ext_dom_set_text_content(n_idx, safe_ptr, (uint32_t)len);

  free(str);
  return true;
}

JSValueRef get_node_innerHTML(JSContextRef ctx, JSObjectRef object,
                              JSStringRef propertyName, JSValueRef *exception) {
  // Delegate to textContent for now
  return get_node_text_content(ctx, object, propertyName, exception);
}

// ---- parentNode getter ----
JSValueRef get_node_ownerDocument(JSContextRef ctx, JSObjectRef object, JSStringRef propertyName, JSValueRef* exception) {
  extern JSObjectRef get_cached_document_wrapper(void);
  return get_cached_document_wrapper();
}

JSValueRef get_node_parentNode(JSContextRef ctx, JSObjectRef object,
                               JSStringRef propertyName,
                               JSValueRef *exception) {
  uint64_t packed = (uint64_t)(uintptr_t)JSObjectGetPrivate(object);
  uint64_t parent_id = js_dom_get_parent(packed);
  if (parent_id == 0)
    return JSValueMakeNull(ctx);
  return create_js_node_wrapper(ctx, parent_id);
}

// ---- childNodes getter (returns a simple array) ----
JSValueRef get_node_parentElement(JSContextRef ctx, JSObjectRef object, JSStringRef propertyName, JSValueRef* exception) {
  uint64_t packed = (uint64_t)(uintptr_t)JSObjectGetPrivate(object);
  uint64_t parent_id = js_dom_get_parent(packed);
  if (parent_id == 0 || parent_id == 999999) return JSValueMakeNull(ctx);
  extern JSObjectRef create_js_node_wrapper(JSContextRef ctx, uint64_t node_id);
  return create_js_node_wrapper(ctx, parent_id);
}

JSValueRef get_node_childNodes(JSContextRef ctx, JSObjectRef object,
                               JSStringRef propertyName,
                               JSValueRef *exception) {
  uint32_t n_idx = jsc_get_node_idx(object);

  // Build JavaScript array of child wrappers
  JSObjectRef arr = JSObjectMakeArray(ctx, 0, NULL, exception);
  uint64_t child_id = dom_get_first_child(n_idx);
  uint32_t i = 0;
  while (child_id != 0 && i < 4096) {
    uint32_t child_idx = (uint32_t)(child_id & 0xFFFF);
    JSValueRef wrapper = create_js_node_wrapper(ctx, child_id);
    JSObjectSetPropertyAtIndex(ctx, arr, i, wrapper, NULL);
    child_id = dom_get_next_sibling(child_idx);
    i++;
  }
  return arr;
}

// ---- style getter (returns object with setters) ----

// Style class: holds a node_idx in private data
static JSClassRef dom_style_class = NULL;

static JSValueRef jsc_style_setProperty(JSContextRef ctx, JSObjectRef function,
                                        JSObjectRef thisObject, size_t argc,
                                        const JSValueRef argv[],
                                        JSValueRef *exception) {
  if (argc < 2)
    return JSValueMakeUndefined(ctx);
  uint32_t n_idx = jsc_get_node_idx(thisObject);

  size_t key_len, val_len;
  char *key = jsc_value_to_cstring(ctx, argv[0], &key_len);
  char *val = jsc_value_to_cstring(ctx, argv[1], &val_len);

  if (key && val) {
    if (strcmp(key, "width") == 0) {
      dom_set_style_width(n_idx, atoi(val));
    } else if (strcmp(key, "height") == 0) {
      dom_set_style_height(n_idx, atoi(val));
    }
  }

  if (key)
    free(key);
  if (val)
    free(val);
  return JSValueMakeUndefined(ctx);
}

static void parse_color(const char *val, uint8_t *r, uint8_t *g, uint8_t *b) {
  if (val[0] == '#') {
    if (strlen(val) == 7) {
      sscanf(val + 1, "%02hhx%02hhx%02hhx", r, g, b);
    } else if (strlen(val) == 4) {
      uint8_t r1, g1, b1;
      sscanf(val + 1, "%1hhx%1hhx%1hhx", &r1, &g1, &b1);
      *r = (r1 << 4) | r1;
      *g = (g1 << 4) | g1;
      *b = (b1 << 4) | b1;
    }
  } else if (strcmp(val, "red") == 0) { *r=255; *g=0; *b=0; }
  else if (strcmp(val, "green") == 0) { *r=0; *g=255; *b=0; }
  else if (strcmp(val, "blue") == 0) { *r=0; *g=0; *b=255; }
  else if (strcmp(val, "white") == 0) { *r=255; *g=255; *b=255; }
  else if (strcmp(val, "black") == 0) { *r=0; *g=0; *b=0; }
  else { *r=0; *g=0; *b=0; } // Default
}

static bool jsc_style_set_backgroundColor(JSContextRef ctx, JSObjectRef object,
                                          JSStringRef propertyName,
                                          JSValueRef value,
                                          JSValueRef *exception) {
  uint32_t n_idx = jsc_get_node_idx(object);
  size_t len;
  char *str = jsc_value_to_cstring(ctx, value, &len);
  if (str) {
    uint8_t r, g, b;
    parse_color(str, &r, &g, &b);
    extern void dom_set_style_bg(uint32_t idx, uint8_t r, uint8_t g, uint8_t b);
    dom_set_style_bg(n_idx, r, g, b);
    free(str);
  }
  return true;
}

static bool jsc_style_set_width(JSContextRef ctx, JSObjectRef object,
                                JSStringRef propertyName, JSValueRef value,
                                JSValueRef *exception) {
  uint32_t n_idx = jsc_get_node_idx(object);
  size_t len;
  char *str = jsc_value_to_cstring(ctx, value, &len);
  if (str) {
    dom_set_style_width(n_idx, atoi(str));
    free(str);
  }
  return true;
}

static bool jsc_style_set_height(JSContextRef ctx, JSObjectRef object,
                                 JSStringRef propertyName, JSValueRef value,
                                 JSValueRef *exception) {
  uint32_t n_idx = jsc_get_node_idx(object);
  size_t len;
  char *str = jsc_value_to_cstring(ctx, value, &len);
  if (str) {
    dom_set_style_height(n_idx, atoi(str));
    free(str);
  }
  return true;
}

static bool jsc_style_set_display(JSContextRef ctx, JSObjectRef object,
                                  JSStringRef propertyName, JSValueRef value,
                                  JSValueRef *exception) {
  uint32_t n_idx = jsc_get_node_idx(object);
  size_t len;
  char *str = jsc_value_to_cstring(ctx, value, &len);
  if (str) {
    uint8_t display_val = 0; // block
    if (strcmp(str, "flex") == 0)
      display_val = 1;
    else if (strcmp(str, "none") == 0)
      display_val = 2;
    else if (strcmp(str, "inline") == 0)
      display_val = 3;
    else if (strcmp(str, "grid") == 0)
      display_val = 4;
    dom_set_style_display(n_idx, display_val);
    free(str);
  }
  return true;
}

static bool jsc_style_set_opacity(JSContextRef ctx, JSObjectRef object,
                                  JSStringRef propertyName, JSValueRef value,
                                  JSValueRef *exception) {
  uint64_t packed = (uint64_t)(uintptr_t)JSObjectGetPrivate(object);
  double val = JSValueToNumber(ctx, value, exception);
  js_set_style_opacity(packed, (float)val);
  return true;
}

static bool jsc_style_set_position(JSContextRef ctx, JSObjectRef object,
                                   JSStringRef propertyName, JSValueRef value,
                                   JSValueRef *exception) {
  uint32_t n_idx = jsc_get_node_idx(object);
  size_t len;
  char *str = jsc_value_to_cstring(ctx, value, &len);
  if (str) {
    uint8_t pos_val = 0; // static
    if (strcmp(str, "relative") == 0)
      pos_val = 1;
    else if (strcmp(str, "absolute") == 0)
      pos_val = 2;
    else if (strcmp(str, "fixed") == 0)
      pos_val = 3;
    else if (strcmp(str, "sticky") == 0)
      pos_val = 4;
    dom_set_style_position(n_idx, pos_val);
    free(str);
  }
  return true;
}

static bool jsc_style_set_top(JSContextRef ctx, JSObjectRef object,
                              JSStringRef propertyName, JSValueRef value,
                              JSValueRef *exception) {
  uint32_t n_idx = jsc_get_node_idx(object);
  size_t len;
  char *str = jsc_value_to_cstring(ctx, value, &len);
  if (str) {
    dom_set_style_top(n_idx, atoi(str));
    free(str);
  }
  return true;
}

static bool jsc_style_set_left(JSContextRef ctx, JSObjectRef object,
                               JSStringRef propertyName, JSValueRef value,
                               JSValueRef *exception) {
  uint32_t n_idx = jsc_get_node_idx(object);
  size_t len;
  char *str = jsc_value_to_cstring(ctx, value, &len);
  if (str) {
    dom_set_style_left(n_idx, atoi(str));
    free(str);
  }
  return true;
}

static bool jsc_style_set_zIndex(JSContextRef ctx, JSObjectRef object,
                                 JSStringRef propertyName, JSValueRef value,
                                 JSValueRef *exception) {
  uint32_t n_idx = jsc_get_node_idx(object);
  size_t len;
  char *str = jsc_value_to_cstring(ctx, value, &len);
  if (str) {
    dom_set_style_z_index(n_idx, atoi(str));
    free(str);
  }
  return true;
}

static bool jsc_style_set_overflow(JSContextRef ctx, JSObjectRef object,
                                   JSStringRef propertyName, JSValueRef value,
                                   JSValueRef *exception) {
  uint32_t n_idx = jsc_get_node_idx(object);
  size_t len;
  char *str = jsc_value_to_cstring(ctx, value, &len);
  if (str) {
    uint8_t ov_val = 0; // visible
    if (strcmp(str, "hidden") == 0)
      ov_val = 1;
    else if (strcmp(str, "scroll") == 0)
      ov_val = 2;
    else if (strcmp(str, "auto") == 0)
      ov_val = 3;
    dom_set_style_overflow(n_idx, ov_val);
    free(str);
  }
  return true;
}

// Forward declarations for Wave 4 style setters (defined later)
static bool jsc_style_set_transform(JSContextRef ctx, JSObjectRef object,
                                    JSStringRef pn, JSValueRef value,
                                    JSValueRef *ex);
static bool jsc_style_set_flexGrow(JSContextRef ctx, JSObjectRef object,
                                   JSStringRef pn, JSValueRef value,
                                   JSValueRef *ex);
static bool jsc_style_set_gridTemplateColumns(JSContextRef ctx,
                                              JSObjectRef object,
                                              JSStringRef pn, JSValueRef value,
                                              JSValueRef *ex);
static bool jsc_style_set_gridColumnStart(JSContextRef ctx, JSObjectRef object,
                                          JSStringRef pn, JSValueRef value,
                                          JSValueRef *ex);

extern int32_t user__browser__dom__STYLE_MARGIN_TOP[65536];
extern int32_t user__browser__dom__STYLE_MARGIN_RIGHT[65536];
extern int32_t user__browser__dom__STYLE_MARGIN_BOTTOM[65536];
extern int32_t user__browser__dom__STYLE_MARGIN_LEFT[65536];
extern int32_t user__browser__dom__STYLE_PADDING_TOP[65536];
extern int32_t user__browser__dom__STYLE_PADDING_RIGHT[65536];
extern int32_t user__browser__dom__STYLE_PADDING_BOTTOM[65536];
extern int32_t user__browser__dom__STYLE_PADDING_LEFT[65536];
extern int32_t user__browser__dom__STYLE_BORDER_WIDTH_TOP[65536];
extern int32_t user__browser__dom__STYLE_BORDER_WIDTH_RIGHT[65536];
extern int32_t user__browser__dom__STYLE_BORDER_WIDTH_BOTTOM[65536];
extern int32_t user__browser__dom__STYLE_BORDER_WIDTH_LEFT[65536];

extern int32_t user__browser__dom__STYLE_MIN_WIDTH[65536];
extern int32_t user__browser__dom__STYLE_MAX_WIDTH[65536];
extern int32_t user__browser__dom__STYLE_MIN_HEIGHT[65536];
extern int32_t user__browser__dom__STYLE_MAX_HEIGHT[65536];
extern uint8_t user__browser__dom__STYLE_BOX_SIZING[65536];
extern uint8_t user__browser__dom__STYLE_VISIBILITY[65536];
extern float   user__browser__dom__STYLE_LINE_HEIGHT[65536];
extern uint8_t user__browser__dom__STYLE_WHITE_SPACE[65536];
extern uint8_t user__browser__dom__STYLE_FLEX_WRAP[65536];
extern uint8_t user__browser__dom__STYLE_TEXT_ALIGN[65536];
extern uint8_t user__browser__dom__STYLE_VERTICAL_ALIGN[65536];
extern float   user__browser__dom__STYLE_FONT_SIZE[65536];

static int32_t parse_px(const char *val) {
  if (strcmp(val, "auto") == 0) return -2;
  return atoi(val);
}

static void parse_shorthand(const char *val, int32_t *t, int32_t *r, int32_t *b, int32_t *l) {
  char buf[256];
  strncpy(buf, val, 255);
  buf[255] = '\0';
  char *tokens[4];
  int count = 0;
  char *tok = strtok(buf, " \t\r\n");
  while (tok && count < 4) {
    tokens[count++] = tok;
    tok = strtok(NULL, " \t\r\n");
  }
  if (count == 1) {
    *t = *r = *b = *l = parse_px(tokens[0]);
  } else if (count == 2) {
    *t = *b = parse_px(tokens[0]);
    *r = *l = parse_px(tokens[1]);
  } else if (count == 3) {
    *t = parse_px(tokens[0]);
    *r = *l = parse_px(tokens[1]);
    *b = parse_px(tokens[2]);
  } else if (count == 4) {
    *t = parse_px(tokens[0]);
    *r = parse_px(tokens[1]);
    *b = parse_px(tokens[2]);
    *l = parse_px(tokens[3]);
  }
}

static bool jsc_style_catchAllSetter(JSContextRef ctx, JSObjectRef object, JSStringRef propertyName, JSValueRef value, JSValueRef* exception) {
  uint32_t n_idx = (uint32_t)(uintptr_t)JSObjectGetPrivate(object);
  if (n_idx == 0 || n_idx == 0xFFFFFFFF) return false;

  size_t len = JSStringGetMaximumUTF8CStringSize(propertyName);
  char *pn = malloc(len);
  if (!pn) return false;
  JSStringGetUTF8CString(propertyName, pn, len);

  JSStringRef jsStr = JSValueToStringCopy(ctx, value, NULL);
  if (!jsStr) {
    free(pn);
    return false;
  }
  size_t vlen = JSStringGetMaximumUTF8CStringSize(jsStr);
  char *val = malloc(vlen);
  if (!val) {
    free(pn);
    JSStringRelease(jsStr);
    return false;
  }
  JSStringGetUTF8CString(jsStr, val, vlen);
  JSStringRelease(jsStr);

  int32_t px = parse_px(val);
  bool layout_dirty = false;
  bool paint_dirty = false;
  
  if (strcmp(pn, "margin") == 0) {
      int32_t t=0, r=0, b=0, l=0;
      parse_shorthand(val, &t, &r, &b, &l);
      user__browser__dom__STYLE_MARGIN_TOP[n_idx] = t;
      user__browser__dom__STYLE_MARGIN_RIGHT[n_idx] = r;
      user__browser__dom__STYLE_MARGIN_BOTTOM[n_idx] = b;
      user__browser__dom__STYLE_MARGIN_LEFT[n_idx] = l;
      layout_dirty = true;
  } else if (strcmp(pn, "marginTop") == 0) {
      user__browser__dom__STYLE_MARGIN_TOP[n_idx] = px;
      layout_dirty = true;
  } else if (strcmp(pn, "marginBottom") == 0) {
      user__browser__dom__STYLE_MARGIN_BOTTOM[n_idx] = px;
      layout_dirty = true;
  } else if (strcmp(pn, "marginLeft") == 0) {
      user__browser__dom__STYLE_MARGIN_LEFT[n_idx] = px;
      layout_dirty = true;
  } else if (strcmp(pn, "marginRight") == 0) {
      user__browser__dom__STYLE_MARGIN_RIGHT[n_idx] = px;
      layout_dirty = true;
  } else if (strcmp(pn, "padding") == 0) {
      int32_t t=0, r=0, b=0, l=0;
      parse_shorthand(val, &t, &r, &b, &l);
      user__browser__dom__STYLE_PADDING_TOP[n_idx] = t;
      user__browser__dom__STYLE_PADDING_RIGHT[n_idx] = r;
      user__browser__dom__STYLE_PADDING_BOTTOM[n_idx] = b;
      user__browser__dom__STYLE_PADDING_LEFT[n_idx] = l;
      layout_dirty = true;
  } else if (strcmp(pn, "paddingTop") == 0) {
      user__browser__dom__STYLE_PADDING_TOP[n_idx] = px;
      layout_dirty = true;
  } else if (strcmp(pn, "paddingBottom") == 0) {
      user__browser__dom__STYLE_PADDING_BOTTOM[n_idx] = px;
      layout_dirty = true;
  } else if (strcmp(pn, "paddingLeft") == 0) {
      user__browser__dom__STYLE_PADDING_LEFT[n_idx] = px;
      layout_dirty = true;
  } else if (strcmp(pn, "paddingRight") == 0) {
      user__browser__dom__STYLE_PADDING_RIGHT[n_idx] = px;
      layout_dirty = true;
  } else if (strcmp(pn, "border") == 0 || strcmp(pn, "borderWidth") == 0) {
      user__browser__dom__STYLE_BORDER_WIDTH_TOP[n_idx] = px > 0 ? px : 1; 
      user__browser__dom__STYLE_BORDER_WIDTH_RIGHT[n_idx] = px > 0 ? px : 1;
      user__browser__dom__STYLE_BORDER_WIDTH_BOTTOM[n_idx] = px > 0 ? px : 1;
      user__browser__dom__STYLE_BORDER_WIDTH_LEFT[n_idx] = px > 0 ? px : 1;
      
      // Try to find a color in the shorthand
      const char *hash = strchr(val, '#');
      if (hash) {
          uint8_t r, g, b;
          parse_color(hash, &r, &g, &b);
          extern void dom_set_style_border_color(uint32_t idx, uint8_t r, uint8_t g, uint8_t b);
          dom_set_style_border_color(n_idx, r, g, b);
          paint_dirty = true;
      }
      layout_dirty = true;
  } else if (strcmp(pn, "color") == 0) {
      uint8_t r, g, b;
      parse_color(val, &r, &g, &b);
      extern void dom_set_style_color(uint32_t idx, uint8_t r, uint8_t g, uint8_t b);
      dom_set_style_color(n_idx, r, g, b);
      paint_dirty = true;
  } else if (strcmp(pn, "fontSize") == 0) {
      user__browser__dom__STYLE_FONT_SIZE[n_idx] = (float)px;
      layout_dirty = true;
  } else if (strcmp(pn, "textAlign") == 0) {
      if (strcmp(val, "center") == 0) user__browser__dom__STYLE_TEXT_ALIGN[n_idx] = 2;
      else if (strcmp(val, "right") == 0) user__browser__dom__STYLE_TEXT_ALIGN[n_idx] = 1;
      else user__browser__dom__STYLE_TEXT_ALIGN[n_idx] = 0;
      layout_dirty = true; // Align now affects layout (Epic 106)
      paint_dirty = true;
  } else if (strcmp(pn, "boxSizing") == 0) {
      if (strcmp(val, "border-box") == 0) user__browser__dom__STYLE_BOX_SIZING[n_idx] = 1;
      else user__browser__dom__STYLE_BOX_SIZING[n_idx] = 0;
      layout_dirty = true;
  } else if (strcmp(pn, "minWidth") == 0) {
      user__browser__dom__STYLE_MIN_WIDTH[n_idx] = px;
      layout_dirty = true;
  } else if (strcmp(pn, "maxWidth") == 0) {
      user__browser__dom__STYLE_MAX_WIDTH[n_idx] = px;
      layout_dirty = true;
  } else if (strcmp(pn, "minHeight") == 0) {
      user__browser__dom__STYLE_MIN_HEIGHT[n_idx] = px;
      layout_dirty = true;
  } else if (strcmp(pn, "maxHeight") == 0) {
      user__browser__dom__STYLE_MAX_HEIGHT[n_idx] = px;
      layout_dirty = true;
  } else if (strcmp(pn, "visibility") == 0) {
      if (strcmp(val, "hidden") == 0) user__browser__dom__STYLE_VISIBILITY[n_idx] = 1;
      else user__browser__dom__STYLE_VISIBILITY[n_idx] = 0;
      paint_dirty = true;
  } else if (strcmp(pn, "lineHeight") == 0) {
      user__browser__dom__STYLE_LINE_HEIGHT[n_idx] = (float)atof(val);
      layout_dirty = true;
  } else if (strcmp(pn, "whiteSpace") == 0) {
      if (strcmp(val, "nowrap") == 0) user__browser__dom__STYLE_WHITE_SPACE[n_idx] = 1;
      else user__browser__dom__STYLE_WHITE_SPACE[n_idx] = 0;
      layout_dirty = true;
  } else if (strcmp(pn, "flexWrap") == 0) {
      if (strcmp(val, "wrap") == 0) user__browser__dom__STYLE_FLEX_WRAP[n_idx] = 1;
      else user__browser__dom__STYLE_FLEX_WRAP[n_idx] = 0;
      layout_dirty = true;
  } else if (strcmp(pn, "verticalAlign") == 0) {
      if (strcmp(val, "middle") == 0) user__browser__dom__STYLE_VERTICAL_ALIGN[n_idx] = 1;
      else if (strcmp(val, "top") == 0) user__browser__dom__STYLE_VERTICAL_ALIGN[n_idx] = 2;
      else if (strcmp(val, "bottom") == 0) user__browser__dom__STYLE_VERTICAL_ALIGN[n_idx] = 3;
      else user__browser__dom__STYLE_VERTICAL_ALIGN[n_idx] = 0;
      layout_dirty = true;
  }

  if (layout_dirty) {
      extern void ext_salt_invalidate_layout(uint32_t node_id);
      ext_salt_invalidate_layout(n_idx);
  } else if (paint_dirty) {
      extern void invalidate_paint(uint32_t node_id);
      invalidate_paint(n_idx);
  }

  free(pn);
  free(val);
  return false; // Let JSC set it purely as JS property so reading it back works identically across everything.
}
void init_style_class(JSContextRef ctx) {
  if (dom_style_class)
    return;

  JSClassDefinition styleDef = kJSClassDefinitionEmpty;
  styleDef.className = "CSSStyleDeclaration";
  styleDef.setProperty = jsc_style_catchAllSetter;

  static JSStaticValue styleValues[] = {
      {"backgroundColor", NULL, jsc_style_set_backgroundColor,
       kJSPropertyAttributeNone},
      {"width", NULL, jsc_style_set_width, kJSPropertyAttributeNone},
      {"height", NULL, jsc_style_set_height, kJSPropertyAttributeNone},
      {"display", NULL, jsc_style_set_display, kJSPropertyAttributeNone},
      {"opacity", NULL, jsc_style_set_opacity, kJSPropertyAttributeNone},
      {"position", NULL, jsc_style_set_position, kJSPropertyAttributeNone},
      {"top", NULL, jsc_style_set_top, kJSPropertyAttributeNone},
      {"left", NULL, jsc_style_set_left, kJSPropertyAttributeNone},
      {"zIndex", NULL, jsc_style_set_zIndex, kJSPropertyAttributeNone},
      {"overflow", NULL, jsc_style_set_overflow, kJSPropertyAttributeNone},
      {"transform", NULL, jsc_style_set_transform, kJSPropertyAttributeNone},
      {"flexGrow", NULL, jsc_style_set_flexGrow, kJSPropertyAttributeNone},
      {"gridTemplateColumns", NULL, jsc_style_set_gridTemplateColumns,
       kJSPropertyAttributeNone},
      {"gridColumnStart", NULL, jsc_style_set_gridColumnStart,
       kJSPropertyAttributeNone},
      {0, 0, 0, 0}};
  styleDef.staticValues = styleValues;

  static JSStaticFunction styleFuncs[] = {
      {"setProperty", jsc_style_setProperty, kJSPropertyAttributeNone},
      {0, 0, 0}};
  styleDef.staticFunctions = styleFuncs;

  dom_style_class = JSClassCreate(&styleDef);
}

JSValueRef get_node_children(JSContextRef ctx, JSObjectRef object, JSStringRef propertyName, JSValueRef* exception) {
  uint32_t n_idx = jsc_get_node_idx(object);
  extern uint32_t dom_get_child_count(uint32_t idx);
  extern uint32_t dom_get_child_idx(uint32_t parent_idx, uint32_t child_idx);
  extern uint32_t dom_get_tag(uint32_t idx);
  extern uint64_t node_id_from_idx(uint32_t idx);

  uint32_t count = dom_get_child_count(n_idx);
  JSValueRef results[256];
  uint32_t found = 0;
  for (uint32_t i = 0; i < count && found < 256; i++) {
    uint32_t c_idx = dom_get_child_idx(n_idx, i);
    if (dom_get_tag(c_idx) != 0) { // Not TAG_TEXT
      results[found++] = create_js_node_wrapper(ctx, node_id_from_idx(c_idx));
    }
  }
  return JSObjectMakeArray(ctx, found, results, exception);
}

JSValueRef get_node_firstElementChild(JSContextRef ctx, JSObjectRef object, JSStringRef propertyName, JSValueRef* exception) {
  uint32_t n_idx = jsc_get_node_idx(object);
  extern uint32_t dom_get_child_count(uint32_t idx);
  extern uint32_t dom_get_child_idx(uint32_t parent_idx, uint32_t child_idx);
  extern uint32_t dom_get_tag(uint32_t idx);
  extern uint64_t node_id_from_idx(uint32_t idx);

  uint32_t count = dom_get_child_count(n_idx);
  for (uint32_t i = 0; i < count; i++) {
    uint32_t c_idx = dom_get_child_idx(n_idx, i);
    if (dom_get_tag(c_idx) != 0) {
      return create_js_node_wrapper(ctx, node_id_from_idx(c_idx));
    }
  }
  return JSValueMakeNull(ctx);
}

JSValueRef get_node_lastElementChild(JSContextRef ctx, JSObjectRef object, JSStringRef propertyName, JSValueRef* exception) {
  uint32_t n_idx = jsc_get_node_idx(object);
  extern uint32_t dom_get_child_count(uint32_t idx);
  extern uint32_t dom_get_child_idx(uint32_t parent_idx, uint32_t child_idx);
  extern uint32_t dom_get_tag(uint32_t idx);
  extern uint64_t node_id_from_idx(uint32_t idx);

  uint32_t count = dom_get_child_count(n_idx);
  if (count == 0) return JSValueMakeNull(ctx);
  for (int32_t i = (int32_t)count - 1; i >= 0; i--) {
    uint32_t c_idx = dom_get_child_idx(n_idx, (uint32_t)i);
    if (dom_get_tag(c_idx) != 0) {
      return create_js_node_wrapper(ctx, node_id_from_idx(c_idx));
    }
  }
  return JSValueMakeNull(ctx);
}

JSValueRef get_node_style(JSContextRef ctx, JSObjectRef object,
                          JSStringRef propertyName, JSValueRef *exception) {
  uint32_t n_idx = jsc_get_node_idx(object);
  init_style_class(ctx);
  // Return a style proxy with same private data as the node (so setters know
  // the node_idx)
  JSObjectRef style_obj =
      JSObjectMake(ctx, dom_style_class, JSObjectGetPrivate(object));
  return style_obj;
}

// ---- classList getter (returns object with add/remove/contains/toggle) ----

static JSValueRef jsc_classList_add(JSContextRef ctx, JSObjectRef function,
                                    JSObjectRef thisObject, size_t argc,
                                    const JSValueRef argv[],
                                    JSValueRef *exception) {
  if (argc < 1)
    return JSValueMakeUndefined(ctx);
  uint32_t n_idx = jsc_get_node_idx(thisObject);

  // Get current class string
  uint64_t cls_ptr = dom_get_class_ptr(n_idx);
  uint32_t cls_len = dom_get_class_len(n_idx);

  size_t add_len;
  char *add_str = jsc_value_to_cstring(ctx, argv[0], &add_len);
  if (!add_str)
    return JSValueMakeUndefined(ctx);

  // Check if already present
  if (cls_ptr != 0 && cls_len > 0) {
    char *cls = (char *)(uintptr_t)cls_ptr;
    // Simple word search
    char *found = strstr(cls, add_str);
    if (found) {
      // Verify it's a whole word
      int before_ok = (found == cls || *(found - 1) == ' ');
      int after_ok = (found[add_len] == '\0' || found[add_len] == ' ');
      if (before_ok && after_ok) {
        free(add_str);
        return JSValueMakeUndefined(ctx); // Already present
      }
    }
  }

  // Append to class string
  uint32_t new_len = cls_len + (cls_len > 0 ? 1 : 0) + (uint32_t)add_len;
  uint64_t new_ptr = dom_alloc_text(new_len);
  if (new_ptr != 0) {
    char *dst = (char *)(uintptr_t)new_ptr;
    if (cls_len > 0 && cls_ptr != 0) {
      memcpy(dst, (void *)(uintptr_t)cls_ptr, cls_len);
      dst[cls_len] = ' ';
      memcpy(dst + cls_len + 1, add_str, add_len);
    } else {
      memcpy(dst, add_str, add_len);
    }
    set_class(n_idx, new_ptr, new_len);
  }

  free(add_str);
  return JSValueMakeUndefined(ctx);
}

static JSValueRef jsc_classList_remove(JSContextRef ctx, JSObjectRef function,
                                       JSObjectRef thisObject, size_t argc,
                                       const JSValueRef argv[],
                                       JSValueRef *exception) {
  if (argc < 1)
    return JSValueMakeUndefined(ctx);
  uint32_t n_idx = jsc_get_node_idx(thisObject);

  uint64_t cls_ptr = dom_get_class_ptr(n_idx);
  uint32_t cls_len = dom_get_class_len(n_idx);
  if (cls_ptr == 0 || cls_len == 0)
    return JSValueMakeUndefined(ctx);

  size_t rem_len;
  char *rem_str = jsc_value_to_cstring(ctx, argv[0], &rem_len);
  if (!rem_str)
    return JSValueMakeUndefined(ctx);

  // Rebuild class string without the removed class
  char *src = (char *)(uintptr_t)cls_ptr;
  uint64_t new_ptr = dom_alloc_text(cls_len);
  char *dst = (char *)(uintptr_t)new_ptr;
  uint32_t dst_len = 0;

  char *token = src;
  uint32_t i = 0;
  while (i <= cls_len) {
    if (i == cls_len || src[i] == ' ') {
      uint32_t tok_len = (uint32_t)(&src[i] - token);
      if (tok_len != (uint32_t)rem_len ||
          memcmp(token, rem_str, rem_len) != 0) {
        if (dst_len > 0)
          dst[dst_len++] = ' ';
        memcpy(dst + dst_len, token, tok_len);
        dst_len += tok_len;
      }
      token = &src[i + 1];
    }
    i++;
  }

  set_class(n_idx, new_ptr, dst_len);
  free(rem_str);
  return JSValueMakeUndefined(ctx);
}

static JSValueRef jsc_classList_contains(JSContextRef ctx, JSObjectRef function,
                                         JSObjectRef thisObject, size_t argc,
                                         const JSValueRef argv[],
                                         JSValueRef *exception) {
  if (argc < 1)
    return JSValueMakeBoolean(ctx, false);
  uint32_t n_idx = jsc_get_node_idx(thisObject);

  uint64_t cls_ptr = dom_get_class_ptr(n_idx);
  uint32_t cls_len = dom_get_class_len(n_idx);
  if (cls_ptr == 0 || cls_len == 0)
    return JSValueMakeBoolean(ctx, false);

  size_t check_len;
  char *check_str = jsc_value_to_cstring(ctx, argv[0], &check_len);
  if (!check_str)
    return JSValueMakeBoolean(ctx, false);

  char *cls = (char *)(uintptr_t)cls_ptr;
  // Create null-terminated copy for strstr
  char *cls_copy = (char *)malloc(cls_len + 1);
  if (!cls_copy) {
    free(check_str);
    return JSValueMakeBoolean(ctx, false);
  }
  memcpy(cls_copy, cls, cls_len);
  cls_copy[cls_len] = '\0';

  bool found = false;
  char *match = strstr(cls_copy, check_str);
  while (match) {
    int before_ok = (match == cls_copy || *(match - 1) == ' ');
    int after_ok = (match[check_len] == '\0' || match[check_len] == ' ');
    if (before_ok && after_ok) {
      found = true;
      break;
    }
    match = strstr(match + 1, check_str);
  }

  free(cls_copy);
  free(check_str);
  return JSValueMakeBoolean(ctx, found);
}

static JSValueRef jsc_classList_toggle(JSContextRef ctx, JSObjectRef function,
                                       JSObjectRef thisObject, size_t argc,
                                       const JSValueRef argv[],
                                       JSValueRef *exception) {
  JSValueRef has =
      jsc_classList_contains(ctx, function, thisObject, argc, argv, exception);
  if (JSValueToBoolean(ctx, has)) {
    jsc_classList_remove(ctx, function, thisObject, argc, argv, exception);
    return JSValueMakeBoolean(ctx, false);
  } else {
    jsc_classList_add(ctx, function, thisObject, argc, argv, exception);
    return JSValueMakeBoolean(ctx, true);
  }
}

static JSClassRef dom_classlist_class = NULL;

void init_classlist_class(JSContextRef ctx) {
  if (dom_classlist_class)
    return;

  JSClassDefinition clDef = kJSClassDefinitionEmpty;
  clDef.className = "DOMTokenList";

  static JSStaticFunction clFuncs[] = {
      {"add", jsc_classList_add, kJSPropertyAttributeNone},
      {"remove", jsc_classList_remove, kJSPropertyAttributeNone},
      {"contains", jsc_classList_contains, kJSPropertyAttributeNone},
      {"toggle", jsc_classList_toggle, kJSPropertyAttributeNone},
      {0, 0, 0}};
  clDef.staticFunctions = clFuncs;

  dom_classlist_class = JSClassCreate(&clDef);
}

JSValueRef get_node_classList(JSContextRef ctx, JSObjectRef object,
                              JSStringRef propertyName, JSValueRef *exception) {
  uint32_t n_idx = jsc_get_node_idx(object);
  init_classlist_class(ctx);
  JSObjectRef cl_obj =
      JSObjectMake(ctx, dom_classlist_class, JSObjectGetPrivate(object));
  return cl_obj;
}

// ---- firstChild getter ----
JSValueRef get_node_firstChild(JSContextRef ctx, JSObjectRef object,
                               JSStringRef propertyName,
                               JSValueRef *exception) {
  uint32_t n_idx = jsc_get_node_idx(object);
  uint64_t child_id = dom_get_first_child(n_idx);
  if (child_id == 0)
    return JSValueMakeNull(ctx);
  return create_js_node_wrapper(ctx, child_id);
}

// ---- nextSibling getter ----
JSValueRef get_node_nextSibling(JSContextRef ctx, JSObjectRef object,
                                JSStringRef propertyName,
                                JSValueRef *exception) {
  uint32_t n_idx = jsc_get_node_idx(object);
  uint64_t sibling_id = dom_get_next_sibling(n_idx);
  if (sibling_id == 0)
    return JSValueMakeNull(ctx);
  return create_js_node_wrapper(ctx, sibling_id);
}

// ---- id getter/setter ----
extern void dom_set_id(uint32_t idx, uint64_t id_ptr, uint32_t id_len);
extern uint64_t dom_get_id_ptr(uint32_t idx);
extern uint32_t dom_get_id_len(uint32_t idx);

JSValueRef get_node_id(JSContextRef ctx, JSObjectRef object,
                       JSStringRef propertyName, JSValueRef *exception) {
  uint32_t n_idx = jsc_get_node_idx(object);
  uint64_t ptr = dom_get_id_ptr(n_idx);
  uint32_t len = dom_get_id_len(n_idx);
  if (ptr == 0 || len == 0)
    return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(""));
  char *buf = (char *)malloc(len + 1);
  memcpy(buf, (void *)(uintptr_t)ptr, len);
  buf[len] = '\0';
  JSStringRef jsStr = JSStringCreateWithUTF8CString(buf);
  JSValueRef res = JSValueMakeString(ctx, jsStr);
  JSStringRelease(jsStr);
  free(buf);
  return res;
}

bool set_node_id(JSContextRef ctx, JSObjectRef object, JSStringRef propertyName,
                 JSValueRef value, JSValueRef *exception) {
  uint32_t n_idx = jsc_get_node_idx(object);
  size_t len;
  char *str = jsc_value_to_cstring(ctx, value, &len);
  if (!str)
    return false;
  uint64_t safe_ptr = dom_alloc_text((uint32_t)len);
  if (safe_ptr != 0) {
    memcpy((void *)(uintptr_t)safe_ptr, str, len);
  }
  dom_set_id(n_idx, safe_ptr, (uint32_t)len);
  free(str);
  return true;
}

// ---- className getter/setter ----
JSValueRef get_node_className(JSContextRef ctx, JSObjectRef object,
                              JSStringRef propertyName, JSValueRef *exception) {
  uint32_t n_idx = jsc_get_node_idx(object);
  uint64_t ptr = dom_get_class_ptr(n_idx);
  uint32_t len = dom_get_class_len(n_idx);
  if (ptr == 0 || len == 0)
    return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(""));
  char *buf = (char *)malloc(len + 1);
  memcpy(buf, (void *)(uintptr_t)ptr, len);
  buf[len] = '\0';
  JSStringRef jsStr = JSStringCreateWithUTF8CString(buf);
  JSValueRef res = JSValueMakeString(ctx, jsStr);
  JSStringRelease(jsStr);
  free(buf);
  return res;
}

bool set_node_className(JSContextRef ctx, JSObjectRef object,
                        JSStringRef propertyName, JSValueRef value,
                        JSValueRef *exception) {
  uint32_t n_idx = jsc_get_node_idx(object);
  size_t len;
  char *str = jsc_value_to_cstring(ctx, value, &len);
  if (!str)
    return false;
  uint64_t safe_ptr = dom_alloc_text((uint32_t)len);
  if (safe_ptr != 0) {
    memcpy((void *)(uintptr_t)safe_ptr, str, len);
  }
  set_class(n_idx, safe_ptr, (uint32_t)len);
  free(str);
  return true;
}

// ---- Event constructor (JSObjectCallAsConstructorCallback signature) ----
JSObjectRef jsc_Event_constructor(JSContextRef ctx, JSObjectRef constructor,
                                  size_t argc, const JSValueRef argv[],
                                  JSValueRef *exception) {
  JSObjectRef event = JSObjectMake(ctx, NULL, NULL);
  if (argc >= 1) {
    JSStringRef typeProp = JSStringCreateWithUTF8CString("type");
    JSObjectSetProperty(ctx, event, typeProp, argv[0], kJSPropertyAttributeNone,
                        NULL);
    JSStringRelease(typeProp);
  }
  return event;
}

// ---- document.createTextNode(text) ----
JSValueRef jsc_document_createTextNode(JSContextRef ctx, JSObjectRef function,
                                       JSObjectRef thisObject, size_t argc,
                                       const JSValueRef argv[],
                                       JSValueRef *exception) {
  uint64_t node_id = ext_salt_create_node(0); // TAG_TEXT = 0
  if (node_id == 0)
    return JSValueMakeNull(ctx);

  if (argc >= 1) {
    size_t len;
    char *str = jsc_value_to_cstring(ctx, argv[0], &len);
    if (str && len > 0) {
      uint32_t n_idx = (uint32_t)(node_id & 0xFFFF);
      uint64_t safe_ptr = dom_alloc_text((uint32_t)len);
      if (safe_ptr != 0) {
        memcpy((void *)(uintptr_t)safe_ptr, str, len);
      }
      ext_dom_set_text_content(n_idx, safe_ptr, (uint32_t)len);
    }
    if (str)
      free(str);
  }

  return create_js_node_wrapper(ctx, node_id);
}

// =====================================================================
// Epic 80A Wave 4: Remaining DOM Methods
// =====================================================================

// ---- insertBefore(newNode, referenceNode) ----
JSValueRef jsc_node_insertBefore(JSContextRef ctx, JSObjectRef function,
                                 JSObjectRef thisObject, size_t argc,
                                 const JSValueRef argv[],
                                 JSValueRef *exception) {
  if (argc < 2)
    return JSValueMakeUndefined(ctx);
  uint32_t parent_idx = jsc_get_node_idx(thisObject);
  JSObjectRef new_obj = JSValueToObject(ctx, argv[0], exception);
  if (!new_obj)
    return JSValueMakeUndefined(ctx);
  uint32_t new_idx = jsc_get_node_idx(new_obj);
  uint32_t ref_idx = 0;
  if (!JSValueIsNull(ctx, argv[1])) {
    JSObjectRef ref_obj = JSValueToObject(ctx, argv[1], NULL);
    if (ref_obj)
      ref_idx = jsc_get_node_idx(ref_obj);
  }
  if (ref_idx == 0) {
    js_dom_append_child(parent_idx, new_idx);
  } else {
    ext_dom_insert_before(parent_idx, new_idx, ref_idx);
  }
  return argv[0];
}

// ---- replaceChild(newChild, oldChild) ----
JSValueRef jsc_node_replaceChild(JSContextRef ctx, JSObjectRef function,
                                 JSObjectRef thisObject, size_t argc,
                                 const JSValueRef argv[],
                                 JSValueRef *exception) {
  if (argc < 2)
    return JSValueMakeUndefined(ctx);
  uint32_t parent_idx = jsc_get_node_idx(thisObject);
  JSObjectRef new_obj = JSValueToObject(ctx, argv[0], exception);
  JSObjectRef old_obj = JSValueToObject(ctx, argv[1], exception);
  if (!new_obj || !old_obj)
    return JSValueMakeUndefined(ctx);
  uint32_t new_idx = jsc_get_node_idx(new_obj);
  uint32_t old_idx = jsc_get_node_idx(old_obj);
  ext_dom_insert_before(parent_idx, new_idx, old_idx);
  ext_dom_remove_child(parent_idx, old_idx);
  return argv[1];
}

// ---- removeAttribute(key) ----
JSValueRef jsc_node_removeAttribute(JSContextRef ctx, JSObjectRef function,
                                    JSObjectRef thisObject, size_t argc,
                                    const JSValueRef argv[],
                                    JSValueRef *exception) {
  if (argc < 1)
    return JSValueMakeUndefined(ctx);
  uint32_t n_idx = jsc_get_node_idx(thisObject);
  size_t key_len;
  char *key = jsc_value_to_cstring(ctx, argv[0], &key_len);
  if (!key)
    return JSValueMakeUndefined(ctx);
  if (key_len == 2 && key[0] == 'i' && key[1] == 'd')
    dom_set_id(n_idx, 0, 0);
  else if (key_len == 5 && memcmp(key, "class", 5) == 0)
    set_class(n_idx, 0, 0);
  free(key);
  return JSValueMakeUndefined(ctx);
}

// ---- click() ----
JSValueRef jsc_node_click(JSContextRef ctx, JSObjectRef function,
                          JSObjectRef thisObject, size_t argc,
                          const JSValueRef argv[], JSValueRef *exception) {
  uint32_t node_idx = jsc_get_node_idx(thisObject);
  if (node_idx > 0) {
    extern void sys_jsc_dispatch_event(uint32_t target_node_idx,
                                       uint32_t type_hash, float client_x,
                                       float client_y);
    sys_jsc_dispatch_event(node_idx, fnv1a_hash_str("click"), 0.0f, 0.0f);
  }
  return JSValueMakeUndefined(ctx);
}

JSValueRef jsc_node_submit(JSContextRef ctx, JSObjectRef function,
                           JSObjectRef thisObject, size_t argc,
                           const JSValueRef argv[], JSValueRef *exception) {
  uint32_t node_idx = jsc_get_node_idx(thisObject);
  if (node_idx > 0) {
    // 1. Extract form action and method
    extern uint64_t dom_get_attr_ptr(uint32_t node_idx, const char *key);
    extern uint32_t dom_get_attr_len(uint32_t node_idx, const char *key);
    
    uint64_t action_ptr = dom_get_attr_ptr(node_idx, "action");
    uint32_t action_len = dom_get_attr_len(node_idx, "action");
    
    char action[1024] = "/search";
    if (action_ptr && action_len > 0) {
      if (action_len > 1023) action_len = 1023;
      memcpy(action, (void*)(uintptr_t)action_ptr, action_len);
      action[action_len] = '\0';
    }

    // 2. Iterate children to find inputs (very simple for POC)
    // Build search query: ?q=val&...
    char query[4096] = {0};
    strcat(query, action);
    strcat(query, "?");

    extern uint32_t dom_get_child_count(uint32_t idx);
    extern uint32_t dom_get_child_idx(uint32_t parent_idx, uint32_t child_idx);
    extern uint32_t dom_get_tag(uint32_t idx);
    extern uint64_t dom_get_value_ptr(uint32_t idx);
    extern uint32_t dom_get_value_len(uint32_t idx);

    uint32_t count = dom_get_child_count(node_idx);
    for (uint32_t i = 0; i < count; i++) {
        uint32_t c_idx = dom_get_child_idx(node_idx, i);
        if (dom_get_tag(c_idx) == 18) { // TAG_INPUT
            uint64_t name_ptr = dom_get_attr_ptr(c_idx, "name");
            uint32_t name_len = dom_get_attr_len(c_idx, "name");
            if (name_ptr) {
                strncat(query, (char*)(uintptr_t)name_ptr, name_len);
                strcat(query, "=");
                uint64_t val_ptr = dom_get_value_ptr(c_idx);
                uint32_t val_len = dom_get_value_len(c_idx);
                if (val_ptr) {
                    strncat(query, (char*)(uintptr_t)val_ptr, val_len);
                }
                strcat(query, "&");
            }
        }
    }

    // 3. Navigate
    extern void sys_browser_navigate(uint64_t ptr, uint32_t len);
    sys_browser_navigate((uint64_t)(uintptr_t)query, (uint32_t)strlen(query));
  }
  return JSValueMakeUndefined(ctx);
}

// ---- attachShadow(init) ----
extern void ext_dom_set_shadow_root(uint32_t host_idx, uint32_t shadow_idx);
JSValueRef jsc_node_attachShadow(JSContextRef ctx, JSObjectRef function,
                                 JSObjectRef thisObject, size_t argc,
                                 const JSValueRef argv[],
                                 JSValueRef *exception) {
  uint32_t host_idx = jsc_get_node_idx(thisObject);
  uint64_t shadow_id = ext_salt_create_node(4);
  if (shadow_id == 0)
    return JSValueMakeNull(ctx);
  uint32_t shadow_idx = (uint32_t)(shadow_id & 0xFFFF);
  ext_dom_set_shadow_root(host_idx, shadow_idx);
  return create_js_node_wrapper(ctx, shadow_id);
}

// ---- getContext('2d') stub ----
extern void dom_init_canvas(uint32_t node_id, uint32_t width, uint32_t height);
JSValueRef jsc_node_getContext(JSContextRef ctx, JSObjectRef function,
                               JSObjectRef thisObject, size_t argc,
                               const JSValueRef argv[], JSValueRef *exception) {
  uint32_t n_idx = jsc_get_node_idx(thisObject);
  dom_init_canvas(n_idx, 300, 150);
  JSObjectRef ctx2d = JSObjectMake(ctx, NULL, JSObjectGetPrivate(thisObject));
  return ctx2d;
}

// ---- nodeValue / value / scrollTop ----
JSValueRef get_node_nodeValue(JSContextRef ctx, JSObjectRef object,
                              JSStringRef pn, JSValueRef *ex) {
  return get_node_text_content(ctx, object, pn, ex);
}
bool set_node_nodeValue(JSContextRef ctx, JSObjectRef object, JSStringRef pn,
                        JSValueRef v, JSValueRef *ex) {
  return set_node_text_content(ctx, object, pn, v, ex);
}
JSValueRef get_node_value(JSContextRef ctx, JSObjectRef object, JSStringRef pn,
                          JSValueRef *ex) {
  return get_node_text_content(ctx, object, pn, ex);
}
bool set_node_value(JSContextRef ctx, JSObjectRef object, JSStringRef pn,
                    JSValueRef v, JSValueRef *ex) {
  return set_node_text_content(ctx, object, pn, v, ex);
}

JSValueRef get_node_src(JSContextRef ctx, JSObjectRef object, JSStringRef pn,
                        JSValueRef *ex) {
  // Stub
  return JSValueMakeUndefined(ctx);
}
bool set_node_src(JSContextRef ctx, JSObjectRef object, JSStringRef pn,
                  JSValueRef v, JSValueRef *ex) {
  // We don't save the URL tightly, just acknowledging the bind. E2E test checks
  // output visually.
  return true;
}
extern void dom_set_layout_scroll_y(uint32_t idx, float val);
bool set_node_scrollTop(JSContextRef ctx, JSObjectRef object, JSStringRef pn,
                        JSValueRef value, JSValueRef *ex) {
  uint32_t n_idx = jsc_get_node_idx(object);
  dom_set_layout_scroll_y(n_idx, (float)JSValueToNumber(ctx, value, ex));
  return true;
}
JSValueRef get_node_scrollTop(JSContextRef ctx, JSObjectRef object,
                              JSStringRef pn, JSValueRef *ex) {
  return JSValueMakeNumber(ctx, 0);
}

// ---- setInterval / clearTimeout / clearInterval / cancelAnimationFrame ----


// ---- Style: transform, flexGrow, gridTemplateColumns, gridColumnStart ----
static bool jsc_style_set_transform(JSContextRef ctx, JSObjectRef object,
                                    JSStringRef pn, JSValueRef value,
                                    JSValueRef *ex) {
  return true;
}
static bool jsc_style_set_flexGrow(JSContextRef ctx, JSObjectRef object,
                                   JSStringRef pn, JSValueRef value,
                                   JSValueRef *ex) {
  return true;
}
static bool jsc_style_set_gridTemplateColumns(JSContextRef ctx,
                                              JSObjectRef object,
                                              JSStringRef pn, JSValueRef value,
                                              JSValueRef *ex) {
  return true;
}
static bool jsc_style_set_gridColumnStart(JSContextRef ctx, JSObjectRef object,
                                          JSStringRef pn, JSValueRef value,
                                          JSValueRef *ex) {
  return true;
}

// ---- Custom Elements ----

JSObjectRef get_ce_prototype(JSContextRef ctx, uint32_t tag_hash) {
  for (int i = 0; i < jsc_custom_elements_count; i++) {
    if (jsc_custom_elements[i].tag_hash == tag_hash) {
      JSStringRef protoStr = JSStringCreateWithUTF8CString("prototype");
      JSValueRef protoVal = JSObjectGetProperty(
          ctx, jsc_custom_elements[i].constructor, protoStr, NULL);
      JSStringRelease(protoStr);
      if (JSValueIsObject(ctx, protoVal)) {
        return (JSObjectRef)protoVal;
      }
      break;
    }
  }
  return NULL;
}

extern void ext_ce_register(uint32_t tag_hash, uint64_t constructor_ptr);

JSValueRef jsc_customElements_define(JSContextRef ctx, JSObjectRef function,
                                     JSObjectRef thisObject, size_t argc,
                                     const JSValueRef argv[],
                                     JSValueRef *exception) {
  if (argc < 2)
    return JSValueMakeUndefined(ctx);
  size_t tag_len;
  char *tag = jsc_value_to_cstring(ctx, argv[0], &tag_len);
  if (!tag)
    return JSValueMakeUndefined(ctx);
  JSObjectRef ctor = JSValueToObject(ctx, argv[1], exception);
  if (!ctor) {
    free(tag);
    return JSValueMakeUndefined(ctx);
  }
  if (jsc_custom_elements_count < 64) {
    JSCCustomElementDef *def =
        &jsc_custom_elements[jsc_custom_elements_count++];
    def->tag_hash = fnv1a_hash_str(tag);
    def->constructor = ctor;
    JSValueProtect(ctx, ctor);

    // Phase 1: Register in Native Array
    ext_ce_register(def->tag_hash, (uint64_t)ctor);
  }
  free(tag);
  return JSValueMakeUndefined(ctx);
}

JSObjectRef jsc_HTMLElement_constructor(JSContextRef ctx,
                                        JSObjectRef constructor, size_t argc,
                                        const JSValueRef argv[],
                                        JSValueRef *exception) {
  uint64_t node_id;
  if (UPGRADE_STACK_PTR > 0) {
    node_id = upgrade_stack_pop(); // Bind to existing native DOM node
  } else {
    node_id = ext_salt_create_node(4); // Standalone new HTMLElement() — allocate fresh
  }
  return create_js_node_wrapper(ctx, node_id);
}

extern JSGlobalContextRef global_ctx;

void ext_jsc_invoke_ce_constructor(uint64_t node_id, uint64_t constructor_ptr) {
  if (!global_ctx)
    return;
  JSObjectRef ctor = (JSObjectRef)constructor_ptr;

  upgrade_stack_push(node_id);
  JSObjectCallAsConstructor(global_ctx, ctor, 0, NULL, NULL);
  // If super() didn't consume it (malformed constructor), drain to prevent leak
  if (UPGRADE_STACK_PTR > 0 &&
      UPGRADE_NODE_STACK[UPGRADE_STACK_PTR - 1] == node_id) {
    UPGRADE_STACK_PTR--;
  }

  // Attempt to dispatch connectedCallback if it exists on the prototype
  JSStringRef connectedStr = JSStringCreateWithUTF8CString("connectedCallback");
  JSValueRef wrapper = create_js_node_wrapper(global_ctx, node_id);
  JSObjectRef obj = JSValueToObject(global_ctx, wrapper, NULL);
  if (obj) {
    JSValueRef cb = JSObjectGetProperty(global_ctx, obj, connectedStr, NULL);
    if (cb && JSValueIsObject(global_ctx, cb)) {
      JSObjectRef cbObj = JSValueToObject(global_ctx, cb, NULL);
      if (JSObjectIsFunction(global_ctx, cbObj)) {
        JSObjectCallAsFunction(global_ctx, cbObj, obj, 0, NULL, NULL);
      }
    }
  }
  JSStringRelease(connectedStr);
}

// ---- Fetch API (stub — returns object, full Promise in Phase 2) ----
extern void ext_net_queue_fetch(uint64_t fetch_id, uint64_t url_ptr,
                                uint32_t url_len);
static uint64_t jsc_next_fetch_id = 1;

typedef struct {
  uint8_t active;
  uint32_t fetch_id;
  JSObjectRef resolve_func;
  JSObjectRef reject_func;
} FetchRequestSlot;

static FetchRequestSlot fetch_cache[256];

static uint32_t fetch_cache_stash(JSContextRef ctx, JSObjectRef resolve, JSObjectRef reject) {
  uint32_t fid = jsc_next_fetch_id++;
  for (int i = 0; i < 256; i++) {
    if (!fetch_cache[i].active) {
      fetch_cache[i].active = 1;
      fetch_cache[i].fetch_id = fid;
      fetch_cache[i].resolve_func = resolve;
      fetch_cache[i].reject_func = reject;
      JSValueProtect(ctx, resolve);
      JSValueProtect(ctx, reject);
      return fid;
    }
  }
  return 0;
}

JSValueRef jsc_window_fetch(JSContextRef ctx, JSObjectRef function,
                            JSObjectRef thisObject, size_t argc,
                            const JSValueRef argv[], JSValueRef *exception) {
  if (argc < 1) return JSValueMakeUndefined(ctx);
  size_t url_len;
  char *url = jsc_value_to_cstring(ctx, argv[0], &url_len);
  if (!url) return JSValueMakeUndefined(ctx);

  JSObjectRef resolve_func, reject_func;
  JSObjectRef promise = JSObjectMakeDeferredPromise(ctx, &resolve_func, &reject_func, exception);
  
  uint32_t fid = fetch_cache_stash(ctx, resolve_func, reject_func);
  if (fid != 0) {
    extern void ext_net_queue_fetch(uint64_t fid, uint64_t url_ptr, uint32_t url_len);
    ext_net_queue_fetch((uint64_t)fid, (uint64_t)(uintptr_t)url, (uint32_t)url_len);
  }
  
  free(url);
  return promise;
}

static JSValueRef jsc_response_json(JSContextRef ctx, JSObjectRef function,
                                    JSObjectRef thisObject, size_t argc,
                                    const JSValueRef argv[], JSValueRef *exception) {
  JSStringRef textProp = JSStringCreateWithUTF8CString("_text");
  JSValueRef text_val = JSObjectGetProperty(ctx, thisObject, textProp, exception);
  JSStringRelease(textProp);
  
  if (!text_val) return JSValueMakeUndefined(ctx);
  
  JSStringRef jsStr = JSValueToStringCopy(ctx, text_val, exception);
  JSValueRef parsed = JSValueMakeFromJSONString(ctx, jsStr);
  JSStringRelease(jsStr);
  if (!parsed) parsed = JSValueMakeUndefined(ctx);
  
  JSObjectRef resolve, reject;
  JSObjectRef promise = JSObjectMakeDeferredPromise(ctx, &resolve, &reject, exception);
  JSObjectCallAsFunction(ctx, resolve, NULL, 1, &parsed, exception);
  return promise;
}

static JSValueRef jsc_response_text(JSContextRef ctx, JSObjectRef function,
                                    JSObjectRef thisObject, size_t argc,
                                    const JSValueRef argv[], JSValueRef *exception) {
  JSStringRef textProp = JSStringCreateWithUTF8CString("_text");
  JSValueRef text_val = JSObjectGetProperty(ctx, thisObject, textProp, exception);
  JSStringRelease(textProp);
  
  if (!text_val) return JSValueMakeUndefined(ctx);
  
  JSObjectRef resolve, reject;
  JSObjectRef promise = JSObjectMakeDeferredPromise(ctx, &resolve, &reject, exception);
  JSObjectCallAsFunction(ctx, resolve, NULL, 1, &text_val, exception);
  return promise;
}

void js_resolve_fetch_impl(uint64_t fetch_id, uint64_t buffer_ptr, uint32_t length) {
  extern JSGlobalContextRef global_ctx;
  if (!global_ctx) return;
  JSContextRef ctx = global_ctx;
  
  for (int i = 0; i < 256; i++) {
    if (fetch_cache[i].active && fetch_cache[i].fetch_id == fetch_id) {
      
      JSObjectRef response_obj = JSObjectMake(ctx, NULL, NULL);
      
      char *buf = malloc(length + 1);
      memcpy(buf, (void*)buffer_ptr, length);
      buf[length] = '\0';
      JSStringRef text_str = JSStringCreateWithUTF8CString(buf);
      free(buf);
      
      JSValueRef text_val = JSValueMakeString(ctx, text_str);
      JSStringRelease(text_str);
      
      JSStringRef prop_text = JSStringCreateWithUTF8CString("_text");
      JSObjectSetProperty(ctx, response_obj, prop_text, text_val, 0, NULL);
      JSStringRelease(prop_text);
      
      JSStringRef prop_json = JSStringCreateWithUTF8CString("json");
      JSObjectSetProperty(ctx, response_obj, prop_json, JSObjectMakeFunctionWithCallback(ctx, prop_json, jsc_response_json), 0, NULL);
      JSStringRelease(prop_json);
      
      JSStringRef prop_text_m = JSStringCreateWithUTF8CString("text");
      JSObjectSetProperty(ctx, response_obj, prop_text_m, JSObjectMakeFunctionWithCallback(ctx, prop_text_m, jsc_response_text), 0, NULL);
      JSStringRelease(prop_text_m);
      
      JSObjectCallAsFunction(ctx, fetch_cache[i].resolve_func, NULL, 1, (JSValueRef*)&response_obj, NULL);
      
      JSValueUnprotect(ctx, fetch_cache[i].resolve_func);
      JSValueUnprotect(ctx, fetch_cache[i].reject_func);
      fetch_cache[i].active = 0;
      
      extern void ext_net_complete_fetch(uint64_t fid, uint64_t ptr, uint32_t len);
      ext_net_complete_fetch(fetch_id, buffer_ptr, length);
      
      for (int s = 0; s < 256; s++) {
        extern uint64_t net_get_fetch_id(uint32_t slot);
        if (net_get_fetch_id(s) == fetch_id) {
          extern void ext_net_reclaim_slot(uint32_t slot);
          ext_net_reclaim_slot(s);
          break;
        }
      }
      
      break;
    }
  }
}

// ---- WebSocket, Worker, MediaSource, AudioContext constructors ----
// Stub — real impl in ws.salt, not linked in test builds
void ext_ws_connect(uint64_t url_ptr, uint32_t url_len) __attribute__((weak));
void ext_ws_connect(uint64_t url_ptr, uint32_t url_len) {}
extern void ext_media_push_chunk(uint64_t data_ptr, uint32_t data_len);
extern void ext_media_push_audio_pcm(uint64_t data_ptr, uint32_t sample_count);

JSObjectRef jsc_WebSocket_constructor(JSContextRef ctx, JSObjectRef constructor,
                                      size_t argc, const JSValueRef argv[],
                                      JSValueRef *exception) {
  JSObjectRef ws = JSObjectMake(ctx, NULL, NULL);
  if (argc >= 1) {
    size_t len;
    char *url = jsc_value_to_cstring(ctx, argv[0], &len);
    if (url) {
      if (ext_ws_connect)
        ext_ws_connect((uint64_t)(uintptr_t)url, (uint32_t)len);
      free(url);
    }
  }
  return ws;
}

extern void sys_ipc_send_r2m_command_with_payload(uint32_t cmd_type,
                                                  uint64_t arg1, uint64_t p_ptr,
                                                  uint32_t p_len);
JSObjectRef jsc_Worker_constructor(JSContextRef ctx, JSObjectRef constructor,
                                   size_t argc, const JSValueRef argv[],
                                   JSValueRef *exception) {
  JSObjectRef w = JSObjectMake(ctx, NULL, NULL);
  if (argc >= 1) {
    size_t len;
    char *url = jsc_value_to_cstring(ctx, argv[0], &len);
    if (url) {
      sys_ipc_send_r2m_command_with_payload(5, 0, (uint64_t)(uintptr_t)url,
                                            (uint32_t)len);
      free(url);
    }
  }
  return w;
}

// Removed empty MediaSource/AudioContext stubs - implemented in jsc_media.m

// =============================================================================
// Epic 85: The Persistent Matrix — IndexedDB JSC Bridge
// =============================================================================

// --- Native Storage Externs (from storage.salt) ---
extern int32_t ext_storage_queue_put(uint32_t promise_id, uint64_t key_ptr,
                                     uint32_t key_len, uint64_t val_ptr,
                                     uint32_t val_len);
extern int32_t ext_storage_queue_get(uint32_t promise_id, uint64_t key_ptr,
                                     uint32_t key_len);
extern int32_t ext_storage_init(uint64_t filename_ptr);
extern void pump_storage_queue(void);

// --- IDB Promise Resolution Cache ---
// Stores GC-protected resolve/reject function refs during async disk I/O
typedef struct {
  uint8_t active;
  uint32_t promise_id;
  JSObjectRef resolve_func;
  JSObjectRef reject_func;
} IDBRequestSlot;

static IDBRequestSlot idb_cache[256];
static uint32_t next_idb_promise_id = 1;

static uint32_t idb_cache_stash(JSContextRef ctx, JSObjectRef resolve,
                                JSObjectRef reject) {
  uint32_t pid = next_idb_promise_id++;
  for (int i = 0; i < 256; i++) {
    if (!idb_cache[i].active) {
      idb_cache[i].active = 1;
      idb_cache[i].promise_id = pid;
      idb_cache[i].resolve_func = resolve;
      idb_cache[i].reject_func = reject;
      // CRITICAL: Protect from GC during async disk I/O
      JSValueProtect(ctx, resolve);
      JSValueProtect(ctx, reject);
      return pid;
    }
  }
  return 0; // overflow
}

// --- JSON Helpers ---
// Native JSON.stringify: convert any JS value to a UTF-8 C string
static char *jsc_json_stringify(JSContextRef ctx, JSValueRef val,
                                size_t *out_len) {
  JSObjectRef global = JSContextGetGlobalObject(ctx);
  JSStringRef jsonName = JSStringCreateWithUTF8CString("JSON");
  JSObjectRef jsonObj =
      (JSObjectRef)JSObjectGetProperty(ctx, global, jsonName, NULL);
  JSStringRelease(jsonName);

  JSStringRef stringifyName = JSStringCreateWithUTF8CString("stringify");
  JSObjectRef stringifyFunc =
      (JSObjectRef)JSObjectGetProperty(ctx, jsonObj, stringifyName, NULL);
  JSStringRelease(stringifyName);

  JSValueRef result =
      JSObjectCallAsFunction(ctx, stringifyFunc, jsonObj, 1, &val, NULL);
  if (!result || JSValueIsUndefined(ctx, result)) {
    *out_len = 0;
    return NULL;
  }
  return jsc_value_to_cstring(ctx, result, out_len);
}

// --- Phase 1: The Promise Capability ---

JSValueRef jsc_indexedDB_put(JSContextRef ctx, JSObjectRef function,
                             JSObjectRef thisObject, size_t argc,
                             const JSValueRef argv[], JSValueRef *exception) {
  if (argc < 2)
    return JSValueMakeUndefined(ctx);

  // 1. Create Deferred Promise
  JSObjectRef resolve_func = NULL, reject_func = NULL;
  JSObjectRef promise =
      JSObjectMakeDeferredPromise(ctx, &resolve_func, &reject_func, exception);

  // 2. Extract key (always string)
  size_t key_len;
  char *key = jsc_value_to_cstring(ctx, argv[0], &key_len);
  if (!key) {
    return promise;
  }

  // 3. Serialize value via JSON.stringify for structured clone fallback
  size_t val_len;
  char *val;
  if (JSValueIsString(ctx, argv[1])) {
    // If it's already a string, wrap in quotes for JSON consistency
    val = jsc_json_stringify(ctx, argv[1], &val_len);
  } else {
    val = jsc_json_stringify(ctx, argv[1], &val_len);
  }
  if (!val) {
    val = strdup("null");
    val_len = 4;
  }

  // 4. Stash callbacks (GC-protected)
  uint32_t pid = idb_cache_stash(ctx, resolve_func, reject_func);

  // 5. Dispatch to native Salt storage matrix
  ext_storage_queue_put(pid, (uint64_t)(uintptr_t)key, (uint32_t)key_len,
                        (uint64_t)(uintptr_t)val, (uint32_t)val_len);

  // 6. Pump the storage queue synchronously (mmap writes are instant)
  pump_storage_queue();

  free(key);
  free(val);
  return promise;
}

JSValueRef jsc_indexedDB_get(JSContextRef ctx, JSObjectRef function,
                             JSObjectRef thisObject, size_t argc,
                             const JSValueRef argv[], JSValueRef *exception) {
  if (argc < 1)
    return JSValueMakeUndefined(ctx);

  // 1. Create Deferred Promise
  JSObjectRef resolve_func = NULL, reject_func = NULL;
  JSObjectRef promise =
      JSObjectMakeDeferredPromise(ctx, &resolve_func, &reject_func, exception);

  // 2. Extract key
  size_t key_len;
  char *key = jsc_value_to_cstring(ctx, argv[0], &key_len);
  if (!key) {
    return promise;
  }

  // 3. Stash callbacks (GC-protected)
  uint32_t pid = idb_cache_stash(ctx, resolve_func, reject_func);

  // 4. Dispatch to native Salt storage matrix
  ext_storage_queue_get(pid, (uint64_t)(uintptr_t)key, (uint32_t)key_len);

  // 5. Pump the storage queue synchronously (mmap reads are instant)
  pump_storage_queue();

  free(key);
  return promise;
}

// --- Phase 1b: The Callback Resolver ---
// Called by storage.salt's pump_storage_queue when disk I/O completes

void js_bridge_resolve_idb_promise(uint32_t promise_id, uint64_t val_ptr,
                                   uint32_t val_len) {
  extern JSGlobalContextRef global_ctx;
  if (!global_ctx)
    return;

  for (int i = 0; i < 256; i++) {
    if (idb_cache[i].active && idb_cache[i].promise_id == promise_id) {
      JSValueRef resolved_value;

      if (val_ptr == 0 && val_len == 0) {
        // Write success or not-found: resolve with undefined
        resolved_value = JSValueMakeUndefined(global_ctx);
      } else {
        // Parse stored JSON string back into a JS value
        char *json_buf = (char *)malloc(val_len + 1);
        memcpy(json_buf, (void *)(uintptr_t)val_ptr, val_len);
        json_buf[val_len] = '\0';

        JSStringRef json_str = JSStringCreateWithUTF8CString(json_buf);
        resolved_value = JSValueMakeFromJSONString(global_ctx, json_str);
        JSStringRelease(json_str);
        free(json_buf);

        if (!resolved_value) {
          // Fallback: return as raw string if JSON parse fails
          JSStringRef raw_str = JSStringCreateWithUTF8CString(json_buf);
          resolved_value = JSValueMakeString(global_ctx, raw_str);
          JSStringRelease(raw_str);
        }
      }

      // Execute resolution
      JSObjectCallAsFunction(global_ctx, idb_cache[i].resolve_func, NULL, 1,
                             &resolved_value, NULL);

      // Free the GC lock
      JSValueUnprotect(global_ctx, idb_cache[i].resolve_func);
      JSValueUnprotect(global_ctx, idb_cache[i].reject_func);
      idb_cache[i].active = 0;
      break;
    }
  }
}

// =============================================================================
// Phase 2: W3C Transaction Lifecycle Shell
// =============================================================================

// IDBObjectStore — delegates to flat put/get
JSValueRef jsc_IDBObjectStore_put(JSContextRef ctx, JSObjectRef function,
                                  JSObjectRef thisObject, size_t argc,
                                  const JSValueRef argv[],
                                  JSValueRef *exception) {
  // W3C signature: store.put(value, key) — our flat store uses put(key, value)
  if (argc >= 2) {
    JSValueRef reordered[2] = {argv[1], argv[0]}; // swap key, value
    return jsc_indexedDB_put(ctx, function, thisObject, 2, reordered,
                             exception);
  } else if (argc == 1) {
    // Single arg — use "default" key
    JSStringRef defKey = JSStringCreateWithUTF8CString("_default");
    JSValueRef defKeyVal = JSValueMakeString(ctx, defKey);
    JSStringRelease(defKey);
    JSValueRef reordered[2] = {defKeyVal, argv[0]};
    return jsc_indexedDB_put(ctx, function, thisObject, 2, reordered,
                             exception);
  }
  return JSValueMakeUndefined(ctx);
}

JSValueRef jsc_IDBObjectStore_get(JSContextRef ctx, JSObjectRef function,
                                  JSObjectRef thisObject, size_t argc,
                                  const JSValueRef argv[],
                                  JSValueRef *exception) {
  return jsc_indexedDB_get(ctx, function, thisObject, argc, argv, exception);
}

JSValueRef jsc_IDBObjectStore_createObjectStore(
    JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc,
    const JSValueRef argv[], JSValueRef *exception) {
  // No-op stub — our flat key-value store doesn't need schema
  return JSValueMakeUndefined(ctx);
}

// IDBTransaction.objectStore(name) → returns IDBObjectStore with put/get
JSValueRef jsc_IDBTransaction_objectStore(JSContextRef ctx,
                                          JSObjectRef function,
                                          JSObjectRef thisObject, size_t argc,
                                          const JSValueRef argv[],
                                          JSValueRef *exception) {
  JSObjectRef store = JSObjectMake(ctx, NULL, NULL);

  JSStringRef putName = JSStringCreateWithUTF8CString("put");
  JSObjectSetProperty(
      ctx, store, putName,
      JSObjectMakeFunctionWithCallback(ctx, putName, jsc_IDBObjectStore_put),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(putName);

  JSStringRef getName = JSStringCreateWithUTF8CString("get");
  JSObjectSetProperty(
      ctx, store, getName,
      JSObjectMakeFunctionWithCallback(ctx, getName, jsc_IDBObjectStore_get),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(getName);

  return store;
}

// IDBDatabase.transaction(storeName, mode) → returns IDBTransaction
JSValueRef jsc_IDBDatabase_transaction(JSContextRef ctx, JSObjectRef function,
                                       JSObjectRef thisObject, size_t argc,
                                       const JSValueRef argv[],
                                       JSValueRef *exception) {
  JSObjectRef tx = JSObjectMake(ctx, NULL, NULL);

  JSStringRef osName = JSStringCreateWithUTF8CString("objectStore");
  JSObjectSetProperty(ctx, tx, osName,
                      JSObjectMakeFunctionWithCallback(
                          ctx, osName, jsc_IDBTransaction_objectStore),
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(osName);

  return tx;
}

// IDBDatabase.createObjectStore(name) — no-op
JSValueRef jsc_IDBDatabase_createObjectStore(
    JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc,
    const JSValueRef argv[], JSValueRef *exception) {
  return JSObjectMake(ctx, NULL, NULL); // return a dummy object store
}

// Helper: Create an IDBDatabase mock object
static JSObjectRef create_idb_database(JSContextRef ctx) {
  JSObjectRef db = JSObjectMake(ctx, NULL, NULL);

  JSStringRef txName = JSStringCreateWithUTF8CString("transaction");
  JSObjectSetProperty(ctx, db, txName,
                      JSObjectMakeFunctionWithCallback(
                          ctx, txName, jsc_IDBDatabase_transaction),
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(txName);

  JSStringRef cosName = JSStringCreateWithUTF8CString("createObjectStore");
  JSObjectSetProperty(ctx, db, cosName,
                      JSObjectMakeFunctionWithCallback(
                          ctx, cosName, jsc_IDBDatabase_createObjectStore),
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(cosName);

  // Convenience: flat put/get directly on db
  JSStringRef putName = JSStringCreateWithUTF8CString("put");
  JSObjectSetProperty(
      ctx, db, putName,
      JSObjectMakeFunctionWithCallback(ctx, putName, jsc_indexedDB_put),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(putName);

  JSStringRef getName = JSStringCreateWithUTF8CString("get");
  JSObjectSetProperty(
      ctx, db, getName,
      JSObjectMakeFunctionWithCallback(ctx, getName, jsc_indexedDB_get),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(getName);

  return db;
}

// File-scope pending IDB open queue
static JSObjectRef g_pending_idb_reqs[16];
static JSObjectRef g_pending_idb_events[16];
static uint8_t g_pending_idb_open_count = 0;

// indexedDB.open(name, version) → IDBOpenDBRequest
JSValueRef jsc_IDBFactory_open(JSContextRef ctx, JSObjectRef function,
                               JSObjectRef thisObject, size_t argc,
                               const JSValueRef argv[], JSValueRef *exception) {
  JSObjectRef req = JSObjectMake(ctx, NULL, NULL);
  JSObjectRef db = create_idb_database(ctx);

  // Set req.result = db
  JSStringRef resultName = JSStringCreateWithUTF8CString("result");
  JSObjectSetProperty(ctx, req, resultName, db, kJSPropertyAttributeNone, NULL);
  JSStringRelease(resultName);

  // Build event = { target: { result: db } }
  JSObjectRef event = JSObjectMake(ctx, NULL, NULL);
  JSObjectRef target = JSObjectMake(ctx, NULL, NULL);
  JSStringRef rStr = JSStringCreateWithUTF8CString("result");
  JSObjectSetProperty(ctx, target, rStr, db, kJSPropertyAttributeNone, NULL);
  JSStringRelease(rStr);
  JSStringRef tStr = JSStringCreateWithUTF8CString("target");
  JSObjectSetProperty(ctx, event, tStr, target, kJSPropertyAttributeNone, NULL);
  JSStringRelease(tStr);

  // Protect from GC until async dispatch
  JSValueProtect(ctx, req);
  JSValueProtect(ctx, event);

  // Queue for async dispatch — onsuccess is set AFTER open() returns
  if (g_pending_idb_open_count < 16) {
    g_pending_idb_reqs[g_pending_idb_open_count] = req;
    g_pending_idb_events[g_pending_idb_open_count] = event;
    g_pending_idb_open_count++;
  }

  return req;
}

// Flush pending IDB open requests — fires onsuccess/onupgradeneeded
// Called from the run loop or after script evaluation
void sys_jsc_flush_idb_open_requests(void) {
  extern JSGlobalContextRef global_ctx;
  if (!global_ctx)
    return;
  if (g_pending_idb_open_count == 0)
    return;

  for (uint8_t i = 0; i < g_pending_idb_open_count; i++) {
    JSObjectRef req = g_pending_idb_reqs[i];
    JSObjectRef event = g_pending_idb_events[i];

    // Check for onupgradeneeded first
    JSStringRef upgradeStr = JSStringCreateWithUTF8CString("onupgradeneeded");
    if (JSObjectHasProperty(global_ctx, req, upgradeStr)) {
      JSValueRef cb = JSObjectGetProperty(global_ctx, req, upgradeStr, NULL);
      if (JSValueIsObject(global_ctx, cb) &&
          JSObjectIsFunction(global_ctx, (JSObjectRef)cb)) {
        JSObjectCallAsFunction(global_ctx, (JSObjectRef)cb, req, 1,
                               (JSValueRef[]){event}, NULL);
      }
    }
    JSStringRelease(upgradeStr);

    // Then fire onsuccess
    JSStringRef successStr = JSStringCreateWithUTF8CString("onsuccess");
    if (JSObjectHasProperty(global_ctx, req, successStr)) {
      JSValueRef cb = JSObjectGetProperty(global_ctx, req, successStr, NULL);
      if (JSValueIsObject(global_ctx, cb) &&
          JSObjectIsFunction(global_ctx, (JSObjectRef)cb)) {
        JSObjectCallAsFunction(global_ctx, (JSObjectRef)cb, req, 1,
                               (JSValueRef[]){event}, NULL);
      }
    }
    JSStringRelease(successStr);

    // Unprotect
    JSValueUnprotect(global_ctx, req);
    JSValueUnprotect(global_ctx, event);
  }
  g_pending_idb_open_count = 0;
}

// ---- Selection, History stubs ----
extern uint32_t dom_get_selection_anchor_node();
JSValueRef jsc_getSelection(JSContextRef ctx, JSObjectRef function,
                            JSObjectRef thisObject, size_t argc,
                            const JSValueRef argv[], JSValueRef *exception) {
  return JSObjectMake(ctx, NULL, NULL);
}
JSValueRef jsc_history_pushState(JSContextRef ctx, JSObjectRef function,
                                 JSObjectRef thisObject, size_t argc,
                                 const JSValueRef argv[],
                                 JSValueRef *exception) {
  return JSValueMakeUndefined(ctx);
}
JSValueRef jsc_URL_createObjectURL(JSContextRef ctx, JSObjectRef function,
                                   JSObjectRef thisObject, size_t argc,
                                   const JSValueRef argv[],
                                   JSValueRef *exception) {
  // Return a dummy media URL that DOM property setter will understand
  JSStringRef s = JSStringCreateWithUTF8CString("blob:prisimi/media/1");
  JSValueRef r = JSValueMakeString(ctx, s);
  JSStringRelease(s);
  return r;
}

JSValueRef jsc_sw_register(JSContextRef ctx, JSObjectRef function,
                           JSObjectRef thisObject, size_t argc,
                           const JSValueRef argv[], JSValueRef *exception) {
  return JSObjectMake(ctx, NULL, NULL);
}
JSValueRef jsc_postMessage(JSContextRef ctx, JSObjectRef function,
                           JSObjectRef thisObject, size_t argc,
                           const JSValueRef argv[], JSValueRef *exception) {
  return JSValueMakeUndefined(ctx);
}

// Layout FFI — clean double-returning accessors into Salt SoA arrays
extern double ext_get_layout_x(uint32_t node_idx);
extern double ext_get_layout_y(uint32_t node_idx);
extern double ext_get_layout_w(uint32_t node_idx);
extern double ext_get_layout_h(uint32_t node_idx);
extern int32_t base64_decode(const uint8_t *src, uint32_t src_len, uint8_t *dst, uint32_t dst_max);

static char jsc_current_url[4096] = "https://www.google.com/";

void jsc_set_current_url(const char *url, uint32_t len) {
  uint32_t copy_len = len < 4095 ? len : 4095;
  memcpy(jsc_current_url, url, copy_len);
  jsc_current_url[copy_len] = '\0';
}

static JSValueRef jsc_location_get_href(JSContextRef ctx, JSObjectRef object, JSStringRef pn, JSValueRef *ex) {
  return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(jsc_current_url));
}
static bool jsc_location_set_href(JSContextRef ctx, JSObjectRef object, JSStringRef pn, JSValueRef value, JSValueRef *ex) {
  size_t len = JSStringGetMaximumUTF8CStringSize(JSValueToStringCopy(ctx, value, NULL));
  char *buf = malloc(len);
  JSStringGetUTF8CString(JSValueToStringCopy(ctx, value, NULL), buf, len);
  extern void sys_browser_navigate(uint64_t ptr, uint32_t len);
  sys_browser_navigate((uint64_t)(uintptr_t)buf, (uint32_t)strlen(buf));
  free(buf);
  return true;
}
static JSValueRef jsc_location_get_hostname(JSContextRef ctx, JSObjectRef obj, JSStringRef pn, JSValueRef *ex) {
  const char *p = strstr(jsc_current_url, "://");
  if (!p) return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(""));
  p += 3;
  const char *slash = strchr(p, '/');
  size_t hlen = slash ? (size_t)(slash - p) : strlen(p);
  char host[256];
  if (hlen > 255) hlen = 255;
  memcpy(host, p, hlen);
  host[hlen] = '\0';
  return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(host));
}
static JSValueRef jsc_location_get_protocol(JSContextRef ctx, JSObjectRef obj, JSStringRef pn, JSValueRef *ex) {
  const char *p = strstr(jsc_current_url, "://");
  if (!p) return JSValueMakeString(ctx, JSStringCreateWithUTF8CString("https:"));
  size_t len = (size_t)(p - jsc_current_url);
  char proto[32];
  if (len > 30) len = 30;
  memcpy(proto, jsc_current_url, len);
  proto[len] = ':';
  proto[len+1] = '\0';
  return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(proto));
}
static JSValueRef jsc_location_get_pathname(JSContextRef ctx, JSObjectRef obj, JSStringRef pn, JSValueRef *ex) {
  const char *p = strstr(jsc_current_url, "://");
  if (!p) return JSValueMakeString(ctx, JSStringCreateWithUTF8CString("/"));
  p += 3;
  const char *slash = strchr(p, '/');
  if (!slash) return JSValueMakeString(ctx, JSStringCreateWithUTF8CString("/"));
  const char *q = strchr(slash, '?');
  const char *h = strchr(slash, '#');
  const char *end = q ? q : (h ? h : slash + strlen(slash));
  size_t len = (size_t)(end - slash);
  char path[1024];
  if (len > 1023) len = 1023;
  memcpy(path, slash, len);
  path[len] = '\0';
  return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(path));
}
static JSValueRef jsc_location_get_search(JSContextRef ctx, JSObjectRef obj, JSStringRef pn, JSValueRef *ex) {
  const char *q = strchr(jsc_current_url, '?');
  if (!q) return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(""));
  const char *h = strchr(q, '#');
  size_t len = h ? (size_t)(h - q) : strlen(q);
  char search[4096];
  if (len > 4095) len = 4095;
  memcpy(search, q, len);
  search[len] = '\0';
  return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(search));
}
static JSValueRef jsc_location_get_hash(JSContextRef ctx, JSObjectRef obj, JSStringRef pn, JSValueRef *ex) {
  const char *h = strchr(jsc_current_url, '#');
  if (!h) return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(""));
  return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(h));
}

static JSValueRef jsc_location_set_window(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef *exception) {
  if (argc > 0) {
    return (JSValueRef)jsc_location_set_href(ctx, thisObject, NULL, argv[0], exception);
  }
  return JSValueMakeUndefined(ctx);
}

JSValueRef get_node_offsetWidth(JSContextRef ctx, JSObjectRef object, JSStringRef pn, JSValueRef *ex) {
  uint32_t n_idx = jsc_get_node_idx(object);
  return JSValueMakeNumber(ctx, ext_get_layout_w(n_idx));
}

JSValueRef get_node_offsetHeight(JSContextRef ctx, JSObjectRef object, JSStringRef pn, JSValueRef *ex) {
  uint32_t n_idx = jsc_get_node_idx(object);
  return JSValueMakeNumber(ctx, ext_get_layout_h(n_idx));
}

JSValueRef jsc_node_contains(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef *exception) {
  if (argc < 1) return JSValueMakeBoolean(ctx, false);
  uint32_t n_idx = jsc_get_node_idx(thisObject);
  JSObjectRef other_obj = JSValueToObject(ctx, argv[0], exception);
  if (!other_obj) return JSValueMakeBoolean(ctx, false);
  uint32_t other_idx = jsc_get_node_idx(other_obj);
  
  if (n_idx == 0 || n_idx == 0xFFFFFFFF || other_idx == 0 || other_idx == 0xFFFFFFFF) return JSValueMakeBoolean(ctx, false);
  if (n_idx == other_idx) return JSValueMakeBoolean(ctx, true);

  extern uint32_t dom_get_parent_idx(uint32_t idx);
  uint32_t curr = other_idx;
  while (curr != 0 && curr != 999999) {
    curr = dom_get_parent_idx(curr);
    if (curr == n_idx) return JSValueMakeBoolean(ctx, true);
  }
  return JSValueMakeBoolean(ctx, false);
}

JSValueRef jsc_node_getBoundingClientRect(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef *ex) {
  // Secure node resolution — if private data is null, this is an unbound call
  void *priv = JSObjectGetPrivate(thisObject);
  if (!priv) {
    JSStringRef errMsg = JSStringCreateWithUTF8CString("getBoundingClientRect called on invalid node");
    *ex = JSValueMakeString(ctx, errMsg);
    JSStringRelease(errMsg);
    return JSValueMakeUndefined(ctx);
  }
  uint32_t n_idx = (uint32_t)((uint64_t)(uintptr_t)priv & 0xFFFF);

  // Read layout metrics through clean FFI boundary
  double x = ext_get_layout_x(n_idx);
  double y = ext_get_layout_y(n_idx);
  double w = ext_get_layout_w(n_idx);
  double h = ext_get_layout_h(n_idx);

  // Construct DOMRect-like object with all 8 W3C properties
  JSObjectRef rect = JSObjectMake(ctx, NULL, NULL);
  JSStringRef keys[] = {
      JSStringCreateWithUTF8CString("x"),      JSStringCreateWithUTF8CString("y"),
      JSStringCreateWithUTF8CString("width"),  JSStringCreateWithUTF8CString("height"),
      JSStringCreateWithUTF8CString("top"),    JSStringCreateWithUTF8CString("left"),
      JSStringCreateWithUTF8CString("right"),  JSStringCreateWithUTF8CString("bottom")};
  double vals[] = {x, y, w, h, y, x, x + w, y + h};
  for (int i = 0; i < 8; i++) {
    JSObjectSetProperty(ctx, rect, keys[i], JSValueMakeNumber(ctx, vals[i]), kJSPropertyAttributeReadOnly, NULL);
    JSStringRelease(keys[i]);
  }
  return rect;
}

JSValueRef jsc_node_hasAttribute(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef *ex) {
  if (argc < 1) return JSValueMakeBoolean(ctx, false);
  JSValueRef attrVal = jsc_node_getAttribute(ctx, function, thisObject, argc, argv, ex);
  return JSValueMakeBoolean(ctx, !JSValueIsNull(ctx, attrVal) && !JSValueIsUndefined(ctx, attrVal));
}

JSValueRef jsc_node_cloneNode(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef *ex) {
  uint32_t n_idx = jsc_get_node_idx(thisObject);
  extern uint32_t dom_get_tag(uint32_t idx);
  uint32_t tag = dom_get_tag(n_idx);
  extern uint64_t ext_salt_create_node(uint32_t t);
  uint64_t new_id = ext_salt_create_node(tag);
  return create_js_node_wrapper(ctx, (uint32_t)(new_id & 0xFFFF));
}

JSValueRef jsc_document_getElementsByTagName(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef *ex) {
  JSValueRef args[] = { JSValueMakeNumber(ctx, 0) };
  return JSObjectMakeArray(ctx, 0, args, ex);
}

JSValueRef jsc_document_getElementsByClassName(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef *ex) {
  JSValueRef args[] = { JSValueMakeNumber(ctx, 0) };
  return JSObjectMakeArray(ctx, 0, args, ex);
}

JSValueRef jsc_window_get_innerWidth(JSContextRef ctx, JSObjectRef object, JSStringRef pn, JSValueRef *ex) {
  return JSValueMakeNumber(ctx, 1920.0);
}
JSValueRef jsc_window_get_innerHeight(JSContextRef ctx, JSObjectRef object, JSStringRef pn, JSValueRef *ex) {
  return JSValueMakeNumber(ctx, 1080.0);
}

JSValueRef jsc_atob(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef *ex) {
  if (argc < 1) return JSValueMakeUndefined(ctx);
  size_t in_len;
  char *in_str = jsc_value_to_cstring(ctx, argv[0], &in_len);
  if (!in_str) return JSValueMakeUndefined(ctx);
  uint8_t *out_buf = malloc(in_len);
  int32_t out_len = base64_decode((uint8_t*)in_str, in_len, out_buf, in_len);
  JSValueRef result = JSValueMakeUndefined(ctx);
  // Remove \0 padding that might have been accidentally encoded
  while (out_len > 0 && out_buf[out_len - 1] == 0) out_len--;
  if (out_len >= 0) {
      out_buf[out_len] = '\0';
      result = JSValueMakeString(ctx, JSStringCreateWithUTF8CString((char*)out_buf));
  }
  free(out_buf);
  free(in_str);
  return result;
}

static const char cb64[] = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
JSValueRef jsc_btoa(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef *ex) {
  if (argc < 1) return JSValueMakeUndefined(ctx);
  size_t in_len;
  char *in_str = jsc_value_to_cstring(ctx, argv[0], &in_len);
  if (!in_str) return JSValueMakeUndefined(ctx);
  char *out = malloc(in_len * 2 + 3);
  if (!out) {
    free(in_str);
    return JSValueMakeUndefined(ctx);
  }
  int i = 0, j = 0;
  for (; i < in_len - 2; i += 3) {
      out[j++] = cb64[(in_str[i] >> 2) & 0x3F];
      out[j++] = cb64[((in_str[i] & 0x03) << 4) | ((in_str[i + 1] & 0xF0) >> 4)];
      out[j++] = cb64[((in_str[i + 1] & 0x0F) << 2) | ((in_str[i + 2] & 0xC0) >> 6)];
      out[j++] = cb64[in_str[i + 2] & 0x3F];
  }
  if (i < in_len) {
      out[j++] = cb64[(in_str[i] >> 2) & 0x3F];
      if (i == (in_len - 1)) {
          out[j++] = cb64[((in_str[i] & 0x03) << 4)];
          out[j++] = '=';
      } else {
          out[j++] = cb64[((in_str[i] & 0x03) << 4) | ((in_str[i + 1] & 0xF0) >> 4)];
          out[j++] = cb64[((in_str[i + 1] & 0x0F) << 2)];
      }
      out[j++] = '=';
  }
  out[j] = '\0';
  JSValueRef result = JSValueMakeString(ctx, JSStringCreateWithUTF8CString(out));
  free(out);
  free(in_str);
  return result;
}

JSValueRef jsc_document_fonts_load_then(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef *exception) {
  if (argc > 0 && JSValueIsObject(ctx, argv[0]) && JSObjectIsFunction(ctx, (JSObjectRef)argv[0])) {
      JSObjectCallAsFunction(ctx, (JSObjectRef)argv[0], NULL, 0, NULL, NULL);
  }
  return thisObject;
}

JSValueRef jsc_document_fonts_load_catch(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef *exception) {
  return thisObject;
}

JSValueRef jsc_document_fonts_load(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef *exception) {
  JSObjectRef promiseObj = JSObjectMake(ctx, NULL, NULL);
  JSStringRef thenStr = JSStringCreateWithUTF8CString("then");
  JSObjectSetProperty(ctx, promiseObj, thenStr, JSObjectMakeFunctionWithCallback(ctx, thenStr, jsc_document_fonts_load_then), kJSPropertyAttributeNone, NULL);
  JSStringRelease(thenStr);

  JSStringRef catchStr = JSStringCreateWithUTF8CString("catch");
  JSObjectSetProperty(ctx, promiseObj, catchStr, JSObjectMakeFunctionWithCallback(ctx, catchStr, jsc_document_fonts_load_catch), kJSPropertyAttributeNone, NULL);
  JSStringRelease(catchStr);
  return promiseObj;
}

JSValueRef jsc_xhr_stub_method(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef *exception) {
  return JSValueMakeUndefined(ctx);
}

JSValueRef jsc_IntersectionObserver_observe(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef *exception) {
  if (argc < 1) return JSValueMakeUndefined(ctx);
  JSObjectRef target_obj = JSValueToObject(ctx, argv[0], exception);
  if (!target_obj) return JSValueMakeUndefined(ctx);
  uint32_t node_idx = jsc_get_node_idx(target_obj);
  if (node_idx == 0 || node_idx == 0xFFFFFFFF) return JSValueMakeUndefined(ctx);

  // Retrieve the callback stored in the observer object
  JSStringRef cbStr = JSStringCreateWithUTF8CString("__callback");
  JSValueRef cbVal = JSObjectGetProperty(ctx, thisObject, cbStr, exception);
  JSStringRelease(cbStr);

  if (JSValueIsObject(ctx, cbVal) && JSObjectIsFunction(ctx, (JSObjectRef)cbVal)) {
    extern uint8_t ext_observers_register(uint32_t node_id, uint64_t callback_ptr, uint8_t obs_type);
    ext_observers_register(node_idx, (uint64_t)(uintptr_t)cbVal, 3 /* Intersection */);
  }
  return JSValueMakeUndefined(ctx);
}

JSValueRef jsc_MutationObserver_observe(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef *exception) {
  if (argc < 1) return JSValueMakeUndefined(ctx);
  JSObjectRef target_obj = JSValueToObject(ctx, argv[0], exception);
  if (!target_obj) return JSValueMakeUndefined(ctx);
  uint32_t node_idx = jsc_get_node_idx(target_obj);
  if (node_idx == 0 || node_idx == 0xFFFFFFFF) return JSValueMakeUndefined(ctx);

  JSStringRef cbStr = JSStringCreateWithUTF8CString("__callback");
  JSValueRef cbVal = JSObjectGetProperty(ctx, thisObject, cbStr, exception);
  JSStringRelease(cbStr);

  if (JSValueIsObject(ctx, cbVal) && JSObjectIsFunction(ctx, (JSObjectRef)cbVal)) {
    extern uint8_t ext_observers_register(uint32_t node_id, uint64_t callback_ptr, uint8_t obs_type);
    ext_observers_register(node_idx, (uint64_t)(uintptr_t)cbVal, 2 /* Mutation */);
  }
  return JSValueMakeUndefined(ctx);
}

JSObjectRef jsc_MutationObserver_constructor(JSContextRef ctx, JSObjectRef constructor, size_t argc, const JSValueRef argv[], JSValueRef *exception) {
  JSClassDefinition moDef = kJSClassDefinitionEmpty;
  moDef.className = "MutationObserver";
  static JSStaticFunction moFuncs[] = {
    {"observe", jsc_MutationObserver_observe, kJSPropertyAttributeNone},
    {"disconnect", jsc_xhr_stub_method, kJSPropertyAttributeNone},
    {"takeRecords", jsc_xhr_stub_method, kJSPropertyAttributeNone},
    {0, 0, 0}
  };
  moDef.staticFunctions = moFuncs;
  JSObjectRef mo = JSObjectMake(ctx, JSClassCreate(&moDef), NULL);
  if (argc >= 1) {
    JSStringRef cbStr = JSStringCreateWithUTF8CString("__callback");
    JSObjectSetProperty(ctx, mo, cbStr, argv[0], kJSPropertyAttributeNone, NULL);
    JSStringRelease(cbStr);
  }
  return mo;
}

JSValueRef jsc_window_matchMedia(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef *exception) {
  JSObjectRef mm = JSObjectMake(ctx, NULL, NULL);
  JSStringRef mStr = JSStringCreateWithUTF8CString("matches");
  JSObjectSetProperty(ctx, mm, mStr, JSValueMakeBoolean(ctx, true), kJSPropertyAttributeNone, NULL);
  JSStringRelease(mStr);
  JSStringRef aelStr = JSStringCreateWithUTF8CString("addEventListener");
  JSObjectSetProperty(ctx, mm, aelStr, JSObjectMakeFunctionWithCallback(ctx, aelStr, jsc_xhr_stub_method), kJSPropertyAttributeNone, NULL);
  JSStringRelease(aelStr);
  return mm;
}

JSObjectRef jsc_IntersectionObserver_constructor(JSContextRef ctx, JSObjectRef constructor, size_t argc, const JSValueRef argv[], JSValueRef *exception) {
  JSClassDefinition ioDef = kJSClassDefinitionEmpty;
  ioDef.className = "IntersectionObserver";
  static JSStaticFunction ioFuncs[] = {
    {"observe", jsc_IntersectionObserver_observe, kJSPropertyAttributeNone},
    {"unobserve", jsc_xhr_stub_method, kJSPropertyAttributeNone},
    {"disconnect", jsc_xhr_stub_method, kJSPropertyAttributeNone},
    {0, 0, 0}
  };
  ioDef.staticFunctions = ioFuncs;
  JSObjectRef io = JSObjectMake(ctx, JSClassCreate(&ioDef), NULL);
  if (argc >= 1) {
    JSStringRef cbStr = JSStringCreateWithUTF8CString("__callback");
    JSObjectSetProperty(ctx, io, cbStr, argv[0], kJSPropertyAttributeNone, NULL);
    JSStringRelease(cbStr);
  }
  return io;
}

JSValueRef jsc_window_requestIdleCallback(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef *exception) {
  if (argc < 1) return JSValueMakeUndefined(ctx);
  // Simple shim: call it in the next microtask (via setTimeout 1ms)
  JSStringRef stStr = JSStringCreateWithUTF8CString("setTimeout");
  JSValueRef stVal = JSObjectGetProperty(ctx, JSContextGetGlobalObject(ctx), stStr, exception);
  JSStringRelease(stStr);
  if (JSValueIsObject(ctx, stVal) && JSObjectIsFunction(ctx, (JSObjectRef)stVal)) {
    JSValueRef stArgs[] = { argv[0], JSValueMakeNumber(ctx, 1.0) };
    JSObjectCallAsFunction(ctx, (JSObjectRef)stVal, NULL, 2, stArgs, exception);
  }
  return JSValueMakeNumber(ctx, 1.0);
}

JSObjectRef jsc_XMLHttpRequest_constructor(JSContextRef ctx, JSObjectRef constructor, size_t argc, const JSValueRef argv[], JSValueRef *exception) {
  JSObjectRef xhr = JSObjectMake(ctx, NULL, NULL);
  
  // Set default properties
  JSStringRef rsStr = JSStringCreateWithUTF8CString("readyState");
  JSObjectSetProperty(ctx, xhr, rsStr, JSValueMakeNumber(ctx, 0), kJSPropertyAttributeNone, NULL);
  JSStringRelease(rsStr);

  JSStringRef stStr = JSStringCreateWithUTF8CString("status");
  JSObjectSetProperty(ctx, xhr, stStr, JSValueMakeNumber(ctx, 0), kJSPropertyAttributeNone, NULL);
  JSStringRelease(stStr);

  JSStringRef rtStr = JSStringCreateWithUTF8CString("responseText");
  JSObjectSetProperty(ctx, xhr, rtStr, JSValueMakeString(ctx, JSStringCreateWithUTF8CString("")), kJSPropertyAttributeNone, NULL);
  JSStringRelease(rtStr);

  JSStringRef aelStr = JSStringCreateWithUTF8CString("addEventListener");
  JSObjectSetProperty(ctx, xhr, aelStr, JSObjectMakeFunctionWithCallback(ctx, aelStr, jsc_xhr_stub_method), kJSPropertyAttributeNone, NULL);
  JSStringRelease(aelStr);

  JSStringRef openStr = JSStringCreateWithUTF8CString("open");
  JSObjectSetProperty(ctx, xhr, openStr, JSObjectMakeFunctionWithCallback(ctx, openStr, jsc_xhr_stub_method), kJSPropertyAttributeNone, NULL);
  JSStringRelease(openStr);

  JSStringRef sendStr = JSStringCreateWithUTF8CString("send");
  JSObjectSetProperty(ctx, xhr, sendStr, JSObjectMakeFunctionWithCallback(ctx, sendStr, jsc_xhr_stub_method), kJSPropertyAttributeNone, NULL);
  JSStringRelease(sendStr);

  return xhr;
}

JSObjectRef jsc_image_constructor(JSContextRef ctx, JSObjectRef constructor, size_t argc, const JSValueRef argv[], JSValueRef *ex) {
  extern uint64_t ext_salt_create_node(uint32_t t);
  uint64_t new_id = ext_salt_create_node(8); // TAG_IMG is 8
  return create_js_node_wrapper(ctx, (uint32_t)(new_id & 0xFFFF));
}

JSValueRef jsc_crypto_getRandomValues(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef *ex) {
  if (argc < 1) {
      JSStringRef errorStr = JSStringCreateWithUTF8CString("TypeError: Failed to execute 'getRandomValues' on 'Crypto': 1 argument required.");
      *ex = JSValueMakeString(ctx, errorStr);
      JSStringRelease(errorStr);
      return JSValueMakeUndefined(ctx);
  }

  JSObjectRef arrayObj = JSValueToObject(ctx, argv[0], ex);

  JSTypedArrayType arrayType = JSValueGetTypedArrayType(ctx, arrayObj, ex);
  if (arrayType == kJSTypedArrayTypeNone) {
      JSStringRef errorStr = JSStringCreateWithUTF8CString("TypeMismatchError: Argument 1 is not a TypedArray.");
      *ex = JSValueMakeString(ctx, errorStr);
      JSStringRelease(errorStr);
      return JSValueMakeUndefined(ctx);
  }

  size_t byteLength = JSObjectGetTypedArrayByteLength(ctx, arrayObj, ex);
  if (byteLength > 65536) {
      JSStringRef errMsg = JSStringCreateWithUTF8CString("QuotaExceededError");
      *ex = JSValueMakeString(ctx, errMsg);
      JSStringRelease(errMsg);
      return argv[0];
  }

  void* bytes = JSObjectGetTypedArrayBytesPtr(ctx, arrayObj, ex);
  if (bytes && byteLength > 0) {
      arc4random_buf(bytes, byteLength);
  }

  return argv[0];
}

JSValueRef jsc_storage_getItem(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef *exception) {
  return JSValueMakeNull(ctx);
}
JSValueRef jsc_storage_setItem(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef *exception) {
  return JSValueMakeUndefined(ctx);
}

JSValueRef jsc__DumpException(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef *exception) {
  if (argc > 0) {
    JSStringRef str = JSValueToStringCopy(ctx, argv[0], NULL);
    size_t max_len = JSStringGetMaximumUTF8CStringSize(str);
    char *buf = malloc(max_len);
    JSStringGetUTF8CString(str, buf, max_len);
    printf("[Prisimi JIT] (Google _DumpException hook): Caught -> %s\n", buf);
    free(buf);
    JSStringRelease(str);
  }
  return JSValueMakeUndefined(ctx);
}

void ext_engine_inject_timer_polyfills(JSGlobalContextRef ctx) {
    JSObjectRef global = JSContextGetGlobalObject(ctx);
    
    // Inject setTimeout
    JSStringRef setTimeName = JSStringCreateWithUTF8CString("setTimeout");
    JSObjectRef setTimeFunc = JSObjectMakeFunctionWithCallback(ctx, setTimeName, JSC_SetTimeout);
    JSObjectSetProperty(ctx, global, setTimeName, setTimeFunc, kJSPropertyAttributeReadOnly | kJSPropertyAttributeDontDelete, NULL);
    JSStringRelease(setTimeName);
    
    // Inject clearTimeout
    JSStringRef clearTimeName = JSStringCreateWithUTF8CString("clearTimeout");
    JSObjectRef clearTimeFunc = JSObjectMakeFunctionWithCallback(ctx, clearTimeName, JSC_ClearTimeout);
    JSObjectSetProperty(ctx, global, clearTimeName, clearTimeFunc, kJSPropertyAttributeReadOnly | kJSPropertyAttributeDontDelete, NULL);
    JSStringRelease(clearTimeName);

    // Inject setInterval
    JSStringRef setIntervalName = JSStringCreateWithUTF8CString("setInterval");
    JSObjectRef setIntervalFunc = JSObjectMakeFunctionWithCallback(ctx, setIntervalName, JSC_SetInterval);
    JSObjectSetProperty(ctx, global, setIntervalName, setIntervalFunc, kJSPropertyAttributeReadOnly | kJSPropertyAttributeDontDelete, NULL);
    JSStringRelease(setIntervalName);

    // Inject clearInterval (aliases to clearTimeout dynamically since IDs share atomic keyspace)
    JSStringRef clearIntervalName = JSStringCreateWithUTF8CString("clearInterval");
    JSObjectRef clearIntervalFunc = JSObjectMakeFunctionWithCallback(ctx, clearIntervalName, JSC_ClearTimeout);
    JSObjectSetProperty(ctx, global, clearIntervalName, clearIntervalFunc, kJSPropertyAttributeReadOnly | kJSPropertyAttributeDontDelete, NULL);
    JSStringRelease(clearIntervalName);

    // Inject requestAnimationFrame
    JSStringRef rafName = JSStringCreateWithUTF8CString("requestAnimationFrame");
    JSObjectRef rafFunc = JSObjectMakeFunctionWithCallback(ctx, rafName, JSC_RequestAnimationFrame);
    JSObjectSetProperty(ctx, global, rafName, rafFunc, kJSPropertyAttributeReadOnly | kJSPropertyAttributeDontDelete, NULL);
    JSStringRelease(rafName);

    // Inject cancelAnimationFrame
    JSStringRef cafName = JSStringCreateWithUTF8CString("cancelAnimationFrame");
    JSObjectRef cafFunc = JSObjectMakeFunctionWithCallback(ctx, cafName, JSC_ClearTimeout);
    JSObjectSetProperty(ctx, global, cafName, cafFunc, kJSPropertyAttributeReadOnly | kJSPropertyAttributeDontDelete, NULL);
    JSStringRelease(cafName);
}

static JSObjectRef jsc_cached_document = NULL;
JSObjectRef get_cached_document_wrapper(void) { return jsc_cached_document; }

void bind_native_globals(JSGlobalContextRef ctx) {
  JSObjectRef global = JSContextGetGlobalObject(ctx);

  // Add WebSocket Class
  extern void sys_init_ws_class(JSContextRef ctx);
  sys_init_ws_class(ctx);

  // Add Media Classes
  extern void sys_init_media_classes(JSGlobalContextRef ctx);
  sys_init_media_classes(ctx);

  // Core Utilities
  JSStringRef imgStr = JSStringCreateWithUTF8CString("Image");
  JSObjectSetProperty(ctx, global, imgStr,
                      JSObjectMakeConstructor(ctx, NULL, jsc_image_constructor),
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(imgStr);

  JSStringRef printStr = JSStringCreateWithUTF8CString("print");
  JSObjectSetProperty(
      ctx, global, printStr,
      JSObjectMakeFunctionWithCallback(ctx, printStr, jsc_print),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(printStr);

  JSStringRef gcStr = JSStringCreateWithUTF8CString("gc");
  JSObjectSetProperty(ctx, global, gcStr,
                      JSObjectMakeFunctionWithCallback(ctx, gcStr, jsc_gc),
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(gcStr);

  JSStringRef freeCountStr = JSStringCreateWithUTF8CString("getFreeNodeCount");
  JSObjectSetProperty(ctx, global, freeCountStr,
                      JSObjectMakeFunctionWithCallback(ctx, freeCountStr,
                                                       jsc_get_free_node_count),
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(freeCountStr);

  JSStringRef obsRegStr =
      JSStringCreateWithUTF8CString("sys_observers_register");
  JSObjectSetProperty(
      ctx, global, obsRegStr,
      JSObjectMakeFunctionWithCallback(ctx, obsRegStr, jsc_observers_register),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(obsRegStr);

  JSStringRef setWidthStr = JSStringCreateWithUTF8CString("sys_node_set_width");
  JSObjectSetProperty(
      ctx, global, setWidthStr,
      JSObjectMakeFunctionWithCallback(ctx, setWidthStr, jsc_node_set_width),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(setWidthStr);

  // Crypto
  JSObjectRef crypto = JSObjectMake(ctx, NULL, NULL);
  JSStringRef grvStr = JSStringCreateWithUTF8CString("getRandomValues");
  JSObjectSetProperty(
      ctx, crypto, grvStr,
      JSObjectMakeFunctionWithCallback(ctx, grvStr, jsc_crypto_getRandomValues),
      kJSPropertyAttributeReadOnly | kJSPropertyAttributeDontDelete, NULL);
  JSStringRelease(grvStr);

  JSStringRef subtStr = JSStringCreateWithUTF8CString("subtle");
  JSObjectSetProperty(ctx, crypto, subtStr, JSObjectMake(ctx, NULL, NULL), kJSPropertyAttributeReadOnly, NULL);
  JSStringRelease(subtStr);

  JSStringRef cryptoStr = JSStringCreateWithUTF8CString("crypto");
  JSObjectSetProperty(ctx, global, cryptoStr, crypto, kJSPropertyAttributeReadOnly | kJSPropertyAttributeDontDelete, NULL);
  JSStringRelease(cryptoStr);

  // Performance
  JSObjectRef performance = JSObjectMake(ctx, NULL, NULL);
  JSStringRef nowStr = JSStringCreateWithUTF8CString("now");
  JSObjectSetProperty(
      ctx, performance, nowStr,
      JSObjectMakeFunctionWithCallback(ctx, nowStr, jsc_performance_now),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(nowStr);

  JSStringRef markStr = JSStringCreateWithUTF8CString("mark");
  JSObjectSetProperty(ctx, performance, markStr, JSObjectMakeFunctionWithCallback(ctx, markStr, jsc_xhr_stub_method), kJSPropertyAttributeNone, NULL);
  JSStringRelease(markStr);
  JSStringRef measStr = JSStringCreateWithUTF8CString("measure");
  JSObjectSetProperty(ctx, performance, measStr, JSObjectMakeFunctionWithCallback(ctx, measStr, jsc_xhr_stub_method), kJSPropertyAttributeNone, NULL);
  JSStringRelease(measStr);

  // Google Unblock: performance.timing & navigation
  JSObjectRef timing = JSObjectMake(ctx, NULL, NULL);
  JSStringRef startStr = JSStringCreateWithUTF8CString("navigationStart");
  JSObjectSetProperty(ctx, timing, startStr, JSValueMakeNumber(ctx, 1000.0), kJSPropertyAttributeNone, NULL);
  JSStringRelease(startStr);
  JSStringRef timStr = JSStringCreateWithUTF8CString("timing");
  JSObjectSetProperty(ctx, performance, timStr, timing, kJSPropertyAttributeNone, NULL);
  JSStringRelease(timStr);

  JSObjectRef navigation = JSObjectMake(ctx, NULL, NULL);
  JSStringRef navTypeStr = JSStringCreateWithUTF8CString("type");
  JSObjectSetProperty(ctx, navigation, navTypeStr, JSValueMakeNumber(ctx, 0.0), kJSPropertyAttributeNone, NULL);
  JSStringRelease(navTypeStr);
  JSStringRef navigationStr = JSStringCreateWithUTF8CString("navigation");
  JSObjectSetProperty(ctx, performance, navigationStr, navigation, kJSPropertyAttributeNone, NULL);
  JSStringRelease(navigationStr);

  JSStringRef perfStr = JSStringCreateWithUTF8CString("performance");
  JSObjectSetProperty(ctx, global, perfStr, performance,
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(perfStr);

  // Inject ALL Timer polyfills through our managed GCD function
  ext_engine_inject_timer_polyfills(ctx);



  // Console
  JSObjectRef console = JSObjectMake(ctx, NULL, NULL);
  JSStringRef logStr = JSStringCreateWithUTF8CString("log");
  JSObjectSetProperty(ctx, console, logStr,
                      JSObjectMakeFunctionWithCallback(ctx, logStr, jsc_print),
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(logStr);

  JSStringRef consoleStr = JSStringCreateWithUTF8CString("console");
  JSObjectSetProperty(ctx, global, consoleStr, console,
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(consoleStr);

  // Document
  JSObjectRef document = JSObjectMake(ctx, NULL, NULL);
  jsc_cached_document = document;
  JSStringRef createElemStr = JSStringCreateWithUTF8CString("createElement");
  JSObjectSetProperty(ctx, document, createElemStr,
                      JSObjectMakeFunctionWithCallback(
                          ctx, createElemStr, jsc_document_createElement),
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(createElemStr);

  JSStringRef getElemStr = JSStringCreateWithUTF8CString("getElementById");
  JSObjectSetProperty(ctx, document, getElemStr,
                      JSObjectMakeFunctionWithCallback(
                          ctx, getElemStr, jsc_document_getElementById),
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(getElemStr);

  JSStringRef gtbnStr = JSStringCreateWithUTF8CString("getElementsByTagName");
  JSObjectSetProperty(ctx, document, gtbnStr,
                      JSObjectMakeFunctionWithCallback(
                          ctx, gtbnStr, jsc_document_getElementsByTagName),
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(gtbnStr);

  JSStringRef gbcnStr = JSStringCreateWithUTF8CString("getElementsByClassName");
  JSObjectSetProperty(ctx, document, gbcnStr,
                      JSObjectMakeFunctionWithCallback(
                          ctx, gbcnStr, jsc_document_getElementsByClassName),
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(gbcnStr);

  {
    static JSStaticValue locValues[] = {
      {"href", jsc_location_get_href, jsc_location_set_href, kJSPropertyAttributeNone},
      {"hostname", jsc_location_get_hostname, NULL, kJSPropertyAttributeReadOnly},
      {"host", jsc_location_get_hostname, NULL, kJSPropertyAttributeReadOnly},
      {"protocol", jsc_location_get_protocol, NULL, kJSPropertyAttributeReadOnly},
      {"pathname", jsc_location_get_pathname, NULL, kJSPropertyAttributeReadOnly},
      {"search", jsc_location_get_search, NULL, kJSPropertyAttributeReadOnly},
      {"hash", jsc_location_get_hash, NULL, kJSPropertyAttributeReadOnly},
      {"origin", jsc_location_get_protocol, NULL, kJSPropertyAttributeReadOnly},
      {0, 0, 0, 0}
    };
    JSClassDefinition locDef = kJSClassDefinitionEmpty;
    locDef.className = "Location";
    locDef.staticValues = locValues;
    JSObjectRef locObj = JSObjectMake(ctx, JSClassCreate(&locDef), NULL);
    JSStringRef locStr = JSStringCreateWithUTF8CString("location");
    JSObjectSetProperty(ctx, global, locStr, locObj, kJSPropertyAttributeNone, NULL);
    JSObjectSetProperty(ctx, document, locStr, locObj, kJSPropertyAttributeNone, NULL);
    JSStringRelease(locStr);
  }

  JSStringRef qSelStr = JSStringCreateWithUTF8CString("querySelector");
  JSObjectSetProperty(ctx, document, qSelStr,
                      JSObjectMakeFunctionWithCallback(
                          ctx, qSelStr, jsc_document_querySelector),
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(qSelStr);

  JSStringRef qSelAllStr = JSStringCreateWithUTF8CString("querySelectorAll");
  JSObjectSetProperty(ctx, document, qSelAllStr,
                      JSObjectMakeFunctionWithCallback(
                          ctx, qSelAllStr, jsc_document_querySelectorAll),
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(qSelAllStr);

  JSStringRef createTextStr = JSStringCreateWithUTF8CString("createTextNode");
  JSObjectSetProperty(ctx, document, createTextStr,
                      JSObjectMakeFunctionWithCallback(
                          ctx, createTextStr, jsc_document_createTextNode),
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(createTextStr);

  // Google Unblock: document.write / document.writeln
  JSStringRef writeStr = JSStringCreateWithUTF8CString("write");
  JSObjectSetProperty(
      ctx, document, writeStr,
      JSObjectMakeFunctionWithCallback(ctx, writeStr, jsc_document_write),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(writeStr);

  JSStringRef writelnStr = JSStringCreateWithUTF8CString("writeln");
  JSObjectSetProperty(
      ctx, document, writelnStr,
      JSObjectMakeFunctionWithCallback(ctx, writelnStr, jsc_document_write),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(writelnStr);

  // document.addEventListener (uses node_id=0 sentinel for document-level
  // events)
  JSStringRef docAELStr = JSStringCreateWithUTF8CString("addEventListener");
  JSObjectSetProperty(ctx, document, docAELStr,
                      JSObjectMakeFunctionWithCallback(
                          ctx, docAELStr, jsc_node_addEventListener),
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(docAELStr);

  // document.body / document.head (node_id 1 and 2 by convention)
  extern JSObjectRef create_js_node_wrapper(JSContextRef ctx, uint64_t node_id);
  JSStringRef bodyStr = JSStringCreateWithUTF8CString("body");
  JSObjectSetProperty(ctx, document, bodyStr, create_js_node_wrapper(ctx, 1),
                      kJSPropertyAttributeReadOnly, NULL);
  JSStringRelease(bodyStr);

  JSStringRef headStr = JSStringCreateWithUTF8CString("head");
  JSObjectSetProperty(ctx, document, headStr, create_js_node_wrapper(ctx, 2),
                      kJSPropertyAttributeReadOnly, NULL);
  JSStringRelease(headStr);

  // document.documentElement (The root <html> node, historically mapped to 1
  // alongside body)
  JSStringRef docElementStr = JSStringCreateWithUTF8CString("documentElement");
  JSObjectSetProperty(ctx, document, docElementStr,
                      create_js_node_wrapper(ctx, 1),
                      kJSPropertyAttributeReadOnly, NULL);
  JSStringRelease(docElementStr);

  // Google Unblock: document.cookie (basic get/set string persistence)
  {
    JSStringRef cookieStr = JSStringCreateWithUTF8CString("cookie");
    JSObjectSetProperty(ctx, document, cookieStr, jsc_document_get_cookie(ctx, document, NULL, NULL), kJSPropertyAttributeNone, NULL);
    JSStringRelease(cookieStr);
  }

  {
    JSObjectRef fontsObj = JSObjectMake(ctx, NULL, NULL);
    JSStringRef loadStr = JSStringCreateWithUTF8CString("load");
    JSObjectSetProperty(ctx, fontsObj, loadStr, JSObjectMakeFunctionWithCallback(ctx, loadStr, jsc_document_fonts_load), kJSPropertyAttributeNone, NULL);
    JSStringRelease(loadStr);

    JSStringRef fontsStr = JSStringCreateWithUTF8CString("fonts");
    JSObjectSetProperty(ctx, document, fontsStr, fontsObj, kJSPropertyAttributeNone, NULL);
    JSStringRelease(fontsStr);
  }

  JSStringRef documentStr = JSStringCreateWithUTF8CString("document");
  JSObjectSetProperty(ctx, global, documentStr, document,
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(documentStr);

  JSStringRef windowStr = JSStringCreateWithUTF8CString("window");
  JSObjectSetProperty(ctx, global, windowStr, global,
                      kJSPropertyAttributeReadOnly, NULL);
  JSStringRelease(windowStr);

  JSStringRef topStr = JSStringCreateWithUTF8CString("top");
  JSObjectSetProperty(ctx, global, topStr, global,
                      kJSPropertyAttributeReadOnly, NULL);
  JSStringRelease(topStr);

  JSStringRef selfStr = JSStringCreateWithUTF8CString("self");
  JSObjectSetProperty(ctx, global, selfStr, global,
                      kJSPropertyAttributeReadOnly, NULL);
  JSStringRelease(selfStr);

  JSStringRef globalThisStr = JSStringCreateWithUTF8CString("globalThis");
  JSObjectSetProperty(ctx, global, globalThisStr, global,
                      kJSPropertyAttributeReadOnly, NULL);
  JSStringRelease(globalThisStr);

  JSStringRef winAELStr = JSStringCreateWithUTF8CString("addEventListener");
  JSObjectSetProperty(ctx, global, winAELStr,
                      JSObjectMakeFunctionWithCallback(
                          ctx, winAELStr, jsc_node_addEventListener),
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(winAELStr);

  JSStringRef pmStr = JSStringCreateWithUTF8CString("postMessage");
  JSObjectSetProperty(
      ctx, global, pmStr,
      JSObjectMakeFunctionWithCallback(ctx, pmStr, jsc_postMessage),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(pmStr);

  // Wave 4: fetch
  JSStringRef fetchStr = JSStringCreateWithUTF8CString("fetch");
  JSObjectSetProperty(
      ctx, global, fetchStr,
      JSObjectMakeFunctionWithCallback(ctx, fetchStr, jsc_window_fetch),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(fetchStr);

  // getSelection
  JSStringRef gSelStr = JSStringCreateWithUTF8CString("getSelection");
  JSObjectSetProperty(
      ctx, global, gSelStr,
      JSObjectMakeFunctionWithCallback(ctx, gSelStr, jsc_getSelection),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(gSelStr);

  // customElements.define
  JSObjectRef customElements = JSObjectMake(ctx, NULL, NULL);
  JSStringRef defStr = JSStringCreateWithUTF8CString("define");
  JSObjectSetProperty(
      ctx, customElements, defStr,
      JSObjectMakeFunctionWithCallback(ctx, defStr, jsc_customElements_define),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(defStr);
  JSStringRef ceStr = JSStringCreateWithUTF8CString("customElements");
  JSObjectSetProperty(ctx, global, ceStr, customElements,
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(ceStr);

  // WebSocket, Worker, MediaSource, AudioContext constructors
  JSStringRef wsStr = JSStringCreateWithUTF8CString("WebSocket");
  JSObjectSetProperty(
      ctx, global, wsStr,
      JSObjectMakeConstructor(ctx, NULL, jsc_WebSocket_constructor),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(wsStr);

  JSStringRef wkStr = JSStringCreateWithUTF8CString("Worker");
  JSObjectSetProperty(
      ctx, global, wkStr,
      JSObjectMakeConstructor(ctx, NULL, jsc_Worker_constructor),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(wkStr);

  JSStringRef xhrStr = JSStringCreateWithUTF8CString("XMLHttpRequest");
  JSObjectSetProperty(
      ctx, global, xhrStr,
      JSObjectMakeConstructor(ctx, NULL, jsc_XMLHttpRequest_constructor),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(xhrStr);

  // MediaSource and AudioContext are now registered via sys_init_media_classes

  // History
  JSObjectRef history = JSObjectMake(ctx, NULL, NULL);
  JSStringRef psStr = JSStringCreateWithUTF8CString("pushState");
  JSObjectSetProperty(
      ctx, history, psStr,
      JSObjectMakeFunctionWithCallback(ctx, psStr, jsc_history_pushState),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(psStr);
  JSStringRef rsStr = JSStringCreateWithUTF8CString("replaceState");
  JSObjectSetProperty(
      ctx, history, rsStr,
      JSObjectMakeFunctionWithCallback(ctx, rsStr, jsc_history_pushState),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(rsStr);  
  
  JSStringRef histStr = JSStringCreateWithUTF8CString("history");
  JSObjectSetProperty(ctx, global, histStr, history, kJSPropertyAttributeNone,
                      NULL);
  JSStringRelease(histStr);

  // URL.createObjectURL
  JSObjectRef urlObj = JSObjectMake(ctx, NULL, NULL);
  JSStringRef couStr = JSStringCreateWithUTF8CString("createObjectURL");
  JSObjectSetProperty(
      ctx, urlObj, couStr,
      JSObjectMakeFunctionWithCallback(ctx, couStr, jsc_URL_createObjectURL),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(couStr);
  JSStringRef urlStr = JSStringCreateWithUTF8CString("URL");
  JSObjectSetProperty(ctx, global, urlStr, urlObj, kJSPropertyAttributeNone,
                      NULL);
  JSStringRelease(urlStr);

  // Epic 85: indexedDB — W3C Transaction Lifecycle + flat convenience
  JSObjectRef idb = JSObjectMake(ctx, NULL, NULL);
  JSStringRef putStr = JSStringCreateWithUTF8CString("put");
  JSObjectSetProperty(
      ctx, idb, putStr,
      JSObjectMakeFunctionWithCallback(ctx, putStr, jsc_indexedDB_put),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(putStr);
  JSStringRef getStr = JSStringCreateWithUTF8CString("get");
  JSObjectSetProperty(
      ctx, idb, getStr,
      JSObjectMakeFunctionWithCallback(ctx, getStr, jsc_indexedDB_get),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(getStr);
  JSStringRef openStr = JSStringCreateWithUTF8CString("open");
  JSObjectSetProperty(
      ctx, idb, openStr,
      JSObjectMakeFunctionWithCallback(ctx, openStr, jsc_IDBFactory_open),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(openStr);
  JSStringRef idbStr = JSStringCreateWithUTF8CString("indexedDB");
  JSObjectSetProperty(ctx, global, idbStr, idb, kJSPropertyAttributeNone, NULL);
  JSStringRelease(idbStr);

  // navigator.serviceWorker.register
  JSObjectRef sw = JSObjectMake(ctx, NULL, NULL);
  JSStringRef regStr = JSStringCreateWithUTF8CString("register");
  JSObjectSetProperty(
      ctx, sw, regStr,
      JSObjectMakeFunctionWithCallback(ctx, regStr, jsc_sw_register),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(regStr);
  JSObjectRef navigator = JSObjectMake(ctx, NULL, NULL);
  JSStringRef swStr = JSStringCreateWithUTF8CString("serviceWorker");
  JSObjectSetProperty(ctx, navigator, swStr, sw, kJSPropertyAttributeNone,
                      NULL);
  JSStringRelease(swStr);

  // Google Unblock: navigator.userAgent — return a standard Chrome Desktop UA
  {
    JSStringRef uaKey = JSStringCreateWithUTF8CString("userAgent");
    JSStringRef uaVal = JSStringCreateWithUTF8CString(
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 "
        "(KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36");
    JSObjectSetProperty(ctx, navigator, uaKey,
                        JSValueMakeString(ctx, uaVal),
                        kJSPropertyAttributeReadOnly, NULL);
    JSStringRelease(uaVal);
    JSStringRelease(uaKey);
  }

  // Google Unblock: navigator.language
  {
    JSStringRef langKey = JSStringCreateWithUTF8CString("language");
    JSStringRef langVal = JSStringCreateWithUTF8CString("en-US");
    JSObjectSetProperty(ctx, navigator, langKey,
                        JSValueMakeString(ctx, langVal),
                        kJSPropertyAttributeReadOnly, NULL);
    JSStringRelease(langVal);
    JSStringRelease(langKey);
  }

  // Google Unblock: navigator.onLine
  {
    JSStringRef onlineKey = JSStringCreateWithUTF8CString("onLine");
    JSObjectSetProperty(ctx, navigator, onlineKey,
                        JSValueMakeBoolean(ctx, true),
                        kJSPropertyAttributeReadOnly, NULL);
    JSStringRelease(onlineKey);
  }

  // Google Unblock: navigator.sendBeacon
  {
    JSStringRef sbStr = JSStringCreateWithUTF8CString("sendBeacon");
    JSObjectSetProperty(ctx, navigator, sbStr, JSObjectMakeFunctionWithCallback(ctx, sbStr, jsc_navigator_sendBeacon), kJSPropertyAttributeNone, NULL);
    JSStringRelease(sbStr);
  }

  JSStringRef navStr = JSStringCreateWithUTF8CString("navigator");
  JSObjectSetProperty(ctx, global, navStr, navigator, kJSPropertyAttributeNone,
                      NULL);
  JSStringRelease(navStr);

  // Google Unblock: Modern global constructors
  const char* modern_ctors[] = {"TextEncoder", "TextDecoder", "URLSearchParams"};
  JSObjectCallAsConstructorCallback modern_callbacks[] = {
    jsc_TextEncoder_constructor, jsc_TextDecoder_constructor, jsc_URLSearchParams_constructor
  };
  for (int i = 0; i < 3; i++) {
    JSStringRef cName = JSStringCreateWithUTF8CString(modern_ctors[i]);
    JSObjectSetProperty(ctx, global, cName, JSObjectMakeConstructor(ctx, NULL, modern_callbacks[i]), kJSPropertyAttributeNone, NULL);
    JSStringRelease(cName);
  }

  // Google Unblock: Event and CustomEvent constructors
  JSStringRef eventStr = JSStringCreateWithUTF8CString("Event");
  JSClassDefinition eventDef = kJSClassDefinitionEmpty;
  eventDef.callAsConstructor = jsc_Event_constructor;
  JSObjectRef eventCtor = JSObjectMake(ctx, JSClassCreate(&eventDef), NULL);
  JSObjectSetProperty(ctx, global, eventStr, eventCtor, kJSPropertyAttributeNone, NULL);
  JSStringRelease(eventStr);

  JSStringRef cEventStr = JSStringCreateWithUTF8CString("CustomEvent");
  JSObjectSetProperty(ctx, global, cEventStr, eventCtor, kJSPropertyAttributeNone, NULL); // Use same basic ctor for now
  JSStringRelease(cEventStr);

  JSStringRef atobStr = JSStringCreateWithUTF8CString("atob");
  JSObjectSetProperty(ctx, global, atobStr,
      JSObjectMakeFunctionWithCallback(ctx, atobStr, jsc_atob),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(atobStr);

  JSStringRef ricStr = JSStringCreateWithUTF8CString("requestIdleCallback");
  JSObjectSetProperty(ctx, global, ricStr,
      JSObjectMakeFunctionWithCallback(ctx, ricStr, jsc_window_requestIdleCallback),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(ricStr);

  // Google Unblock: Intl
  JSObjectRef intl = JSObjectMake(ctx, NULL, NULL);
  JSStringRef intlStr = JSStringCreateWithUTF8CString("Intl");
  JSObjectSetProperty(ctx, global, intlStr, intl, kJSPropertyAttributeNone, NULL);
  JSStringRelease(intlStr);

  // Google Unblock: DOMParser
  JSStringRef dpStr = JSStringCreateWithUTF8CString("DOMParser");
  JSObjectSetProperty(ctx, global, dpStr, JSObjectMake(ctx, NULL, NULL), kJSPropertyAttributeNone, NULL);
  JSStringRelease(dpStr);

  // Google Unblock: Base DOM Classes
  const char* dom_classes[] = {
    "EventTarget", "Node", "Element", "HTMLElement", "HTMLInputElement", 
    "HTMLButtonElement", "HTMLFormElement", "HTMLAnchorElement", "HTMLTableElement",
    "HTMLDivElement", "HTMLSpanElement", "HTMLImageElement", "HTMLScriptElement"
  };
  for (int i = 0; i < 13; i++) {
    JSStringRef clsStr = JSStringCreateWithUTF8CString(dom_classes[i]);
    JSObjectSetProperty(ctx, global, clsStr, JSObjectMake(ctx, NULL, NULL), kJSPropertyAttributeNone, NULL);
    JSStringRelease(clsStr);
  }

  JSStringRef ioStr = JSStringCreateWithUTF8CString("IntersectionObserver");
  JSObjectSetProperty(ctx, global, ioStr,
      JSObjectMakeConstructor(ctx, NULL, jsc_IntersectionObserver_constructor),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(ioStr);

  JSStringRef moStr = JSStringCreateWithUTF8CString("MutationObserver");
  JSObjectSetProperty(ctx, global, moStr,
      JSObjectMakeConstructor(ctx, NULL, jsc_MutationObserver_constructor),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(moStr);

  JSStringRef mmStr = JSStringCreateWithUTF8CString("matchMedia");
  JSObjectSetProperty(ctx, global, mmStr,
      JSObjectMakeFunctionWithCallback(ctx, mmStr, jsc_window_matchMedia),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(mmStr);

  JSStringRef btoaStr = JSStringCreateWithUTF8CString("btoa");
  JSObjectSetProperty(ctx, global, btoaStr,
      JSObjectMakeFunctionWithCallback(ctx, btoaStr, jsc_btoa),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(btoaStr);

  JSStringRef imageStr = JSStringCreateWithUTF8CString("Image");
  JSObjectSetProperty(ctx, global, imageStr,
      JSObjectMakeConstructor(ctx, NULL, jsc_image_constructor),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(imageStr);

  JSStringRef innerWidthStr = JSStringCreateWithUTF8CString("innerWidth");
  JSObjectSetProperty(ctx, global, innerWidthStr, jsc_window_get_innerWidth(ctx, global, NULL, NULL), kJSPropertyAttributeNone, NULL);
  JSStringRelease(innerWidthStr);

  JSStringRef dprStr = JSStringCreateWithUTF8CString("devicePixelRatio");
  JSObjectSetProperty(ctx, global, dprStr, jsc_window_get_devicePixelRatio(ctx, global, NULL, NULL), kJSPropertyAttributeNone, NULL);
  JSStringRelease(dprStr);

  JSObjectRef screen = JSObjectMake(ctx, NULL, NULL);
  JSStringRef scrwStr = JSStringCreateWithUTF8CString("width");
  JSObjectSetProperty(ctx, screen, scrwStr, JSValueMakeNumber(ctx, 1920), kJSPropertyAttributeNone, NULL);
  JSStringRelease(scrwStr);
  JSStringRef shStr = JSStringCreateWithUTF8CString("height");
  JSObjectSetProperty(ctx, screen, shStr, JSValueMakeNumber(ctx, 1080), kJSPropertyAttributeNone, NULL);
  JSStringRelease(shStr);
  JSStringRef sawStr = JSStringCreateWithUTF8CString("availWidth");
  JSObjectSetProperty(ctx, screen, sawStr, JSValueMakeNumber(ctx, 1920), kJSPropertyAttributeNone, NULL);
  JSStringRelease(sawStr);
  JSStringRef sahStr = JSStringCreateWithUTF8CString("availHeight");
  JSObjectSetProperty(ctx, screen, sahStr, JSValueMakeNumber(ctx, 1050), kJSPropertyAttributeNone, NULL);
  JSStringRelease(sahStr);
  JSStringRef sdStr = JSStringCreateWithUTF8CString("colorDepth");
  JSObjectSetProperty(ctx, screen, sdStr, JSValueMakeNumber(ctx, 24), kJSPropertyAttributeNone, NULL);
  JSStringRelease(sdStr);

  JSStringRef screenStr = JSStringCreateWithUTF8CString("screen");
  JSObjectSetProperty(ctx, global, screenStr, screen, kJSPropertyAttributeNone, NULL);
  JSStringRelease(screenStr);

  JSStringRef radyStr = JSStringCreateWithUTF8CString("readyState");
  JSObjectSetProperty(ctx, document, radyStr, jsc_document_get_readyState(ctx, document, NULL, NULL), kJSPropertyAttributeNone, NULL);
  JSStringRelease(radyStr);

  JSStringRef refStr = JSStringCreateWithUTF8CString("referrer");
  JSObjectSetProperty(ctx, document, refStr, jsc_document_get_referrer(ctx, document, NULL, NULL), kJSPropertyAttributeNone, NULL);
  JSStringRelease(refStr);

  JSStringRef aeStr = JSStringCreateWithUTF8CString("activeElement");
  JSObjectSetProperty(ctx, document, aeStr, jsc_document_get_activeElement(ctx, document, NULL, NULL), kJSPropertyAttributeNone, NULL);
  JSStringRelease(aeStr);

  JSStringRef titleStr = JSStringCreateWithUTF8CString("title");
  JSObjectSetProperty(ctx, document, titleStr, jsc_document_get_title(ctx, document, NULL, NULL), kJSPropertyAttributeNone, NULL);
  // setter requires a class, but we are using a direct object.
  // We can just use the catchAll setter if we made one for document, but we haven't.
  // For now, let's just make it a simple property, JS will overwrite it but our getter won't be called.
  // To do it properly we need a Document class. 
  // Let's just use JSObjectSetProperty with getter/setter for now.
  
  JSStringRelease(titleStr);

  JSStringRef dvStr = JSStringCreateWithUTF8CString("defaultView");
  JSObjectSetProperty(ctx, document, dvStr, global, kJSPropertyAttributeNone, NULL);
  JSStringRelease(dvStr);

  JSStringRef innerHeightStr = JSStringCreateWithUTF8CString("innerHeight");
  JSObjectSetProperty(ctx, global, innerHeightStr, jsc_window_get_innerHeight(ctx, global, NULL, NULL), kJSPropertyAttributeNone, NULL);
  JSStringRelease(innerHeightStr);

  // Google Unblock: window.__google — empty object to prevent prototype crashes
  {
    JSObjectRef googleInternal = JSObjectMake(ctx, NULL, NULL);
    JSStringRef giStr = JSStringCreateWithUTF8CString("__google");
    JSObjectSetProperty(ctx, global, giStr, googleInternal,
                        kJSPropertyAttributeNone, NULL);
    JSStringRelease(giStr);
  }

  // localStorage and sessionStorage
  {
    JSObjectRef storageObj = JSObjectMake(ctx, NULL, NULL);
    JSStringRef getItemStr = JSStringCreateWithUTF8CString("getItem");
    JSObjectSetProperty(ctx, storageObj, getItemStr, JSObjectMakeFunctionWithCallback(ctx, getItemStr, jsc_storage_getItem), kJSPropertyAttributeNone, NULL);
    JSStringRelease(getItemStr);
    JSStringRef setItemStr = JSStringCreateWithUTF8CString("setItem");
    JSObjectSetProperty(ctx, storageObj, setItemStr, JSObjectMakeFunctionWithCallback(ctx, setItemStr, jsc_storage_setItem), kJSPropertyAttributeNone, NULL);
    JSStringRelease(setItemStr);
    
    JSStringRef lsStr = JSStringCreateWithUTF8CString("localStorage");
    JSObjectSetProperty(ctx, global, lsStr, storageObj, kJSPropertyAttributeNone, NULL);
    JSStringRelease(lsStr);
    JSStringRef ssStr = JSStringCreateWithUTF8CString("sessionStorage");
    JSObjectSetProperty(ctx, global, ssStr, storageObj, kJSPropertyAttributeNone, NULL);
    JSStringRelease(ssStr);
  }

  // window._._DumpException AND Object.prototype._DumpException
  {
    JSObjectRef _obj = JSObjectMake(ctx, NULL, NULL);
    JSStringRef _DumpStr = JSStringCreateWithUTF8CString("_DumpException");
    JSObjectRef dumpFunc = JSObjectMakeFunctionWithCallback(ctx, _DumpStr, jsc__DumpException);
    
    JSObjectSetProperty(ctx, _obj, _DumpStr, dumpFunc, kJSPropertyAttributeNone, NULL);
    JSStringRef _str = JSStringCreateWithUTF8CString("_");
    JSObjectSetProperty(ctx, global, _str, _obj, kJSPropertyAttributeNone, NULL);
    JSStringRelease(_str);
    
    // Inject onto Object.prototype because Google uses scoped variables `(function(_){...})(this.gbar_)`
    JSStringRef objStr = JSStringCreateWithUTF8CString("Object");
    JSValueRef objVal = JSObjectGetProperty(ctx, global, objStr, NULL);
    JSObjectRef objConstructor = JSValueToObject(ctx, objVal, NULL);
    
    JSStringRef protoStr = JSStringCreateWithUTF8CString("prototype");
    JSValueRef protoVal = JSObjectGetProperty(ctx, objConstructor, protoStr, NULL);
    JSObjectRef protoObj = JSValueToObject(ctx, protoVal, NULL);

    JSObjectSetProperty(ctx, protoObj, _DumpStr, dumpFunc, kJSPropertyAttributeNone, NULL);
    
    JSStringRelease(protoStr);
    JSStringRelease(objStr);
    JSStringRelease(_DumpStr);
  }

  {
    JSObjectRef googleObj = JSObjectMake(ctx, NULL, NULL);
    JSStringRef goStr = JSStringCreateWithUTF8CString("google");
    JSObjectSetProperty(ctx, global, goStr, googleObj,
                        kJSPropertyAttributeNone, NULL);
    
    // Epic 111: Google Search Unblock Stubs
    JSStringRef gxStr = JSStringCreateWithUTF8CString("x");
    JSObjectSetProperty(ctx, googleObj, gxStr, JSObjectMakeFunctionWithCallback(ctx, gxStr, jsc_xhr_stub_method), kJSPropertyAttributeNone, NULL);
    JSStringRelease(gxStr);
    
    JSStringRef glStr = JSStringCreateWithUTF8CString("log");
    JSObjectSetProperty(ctx, googleObj, glStr, JSObjectMakeFunctionWithCallback(ctx, glStr, jsc_xhr_stub_method), kJSPropertyAttributeNone, NULL);
    JSStringRelease(glStr);

    JSStringRef kStr = JSStringCreateWithUTF8CString("kEXPI");
    JSObjectSetProperty(ctx, googleObj, kStr, JSValueMakeString(ctx, JSStringCreateWithUTF8CString("0")), kJSPropertyAttributeNone, NULL);
    JSStringRelease(kStr);

    JSStringRelease(goStr);
  }
}

void jsc_bindings_teardown(JSContextRef ctx) {
    for (int i = 0; i < 256; i++) {
      if (fetch_cache[i].active) {
        JSValueUnprotect(ctx, fetch_cache[i].resolve_func);
        JSValueUnprotect(ctx, fetch_cache[i].reject_func);
        fetch_cache[i].active = 0;
      }
      if (idb_cache[i].active) {
        JSValueUnprotect(ctx, idb_cache[i].resolve_func);
        JSValueUnprotect(ctx, idb_cache[i].reject_func);
        idb_cache[i].active = 0;
      }
    }
}

// --- Callback Dispatchers (invoked from Salt) ---

void sys_js_execute_timer(uint32_t timer_id, uint8_t is_interval) {
    // Obsolete - Handled by GCD directly.
}

void sys_js_execute_raf(uint32_t raf_id, double timestamp) {
    // Obsolete - Handled by GCD directly.
}

void ext_net_navigate(uint64_t url_ptr, uint32_t url_len) {
    extern void sys_browser_navigate(uint64_t ptr, uint32_t len);
    sys_browser_navigate(url_ptr, url_len);
}
