#import <Foundation/Foundation.h>
#import <JavaScriptCore/JavaScriptCore.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

JSGlobalContextRef global_ctx = NULL;

extern void bind_native_globals(JSGlobalContextRef ctx);
extern void init_dom_classes(JSGlobalContextRef ctx);

void sys_jsc_dump_exception(JSContextRef ctx, JSValueRef exception) {
  JSStringRef exceptionStr = JSValueToStringCopy(ctx, exception, NULL);
  size_t len = JSStringGetMaximumUTF8CStringSize(exceptionStr);
  char *buf = (char *)malloc(len);
  if (buf) {
    JSStringGetUTF8CString(exceptionStr, buf, len);
    printf("[Prisimi JIT] Exception: %s\n", buf);
  }

  // Attempt to get line/column if available
  JSObjectRef excObj = JSValueToObject(ctx, exception, NULL);
  if (excObj) {
    JSStringRef lineProp = JSStringCreateWithUTF8CString("line");
    JSValueRef lineVal = JSObjectGetProperty(ctx, excObj, lineProp, NULL);
    if (JSValueIsNumber(ctx, lineVal)) {
      printf("  at line: %d\n", (int)JSValueToNumber(ctx, lineVal, NULL));
    }
    JSStringRelease(lineProp);
  }

  if (buf) free(buf);
  JSStringRelease(exceptionStr);
}

void sys_jsc_init() {
  printf("[Prisimi JIT] Initializing JavaScriptCore Matrix (Renderer)...\n");
  JSContextGroupRef group = JSContextGroupCreate();

  JSClassDefinition globalClassDef = kJSClassDefinitionEmpty;
  JSClassRef globalClass = JSClassCreate(&globalClassDef);

  global_ctx = JSGlobalContextCreateInGroup(group, globalClass);

  init_dom_classes(global_ctx);
  bind_native_globals(global_ctx);

  JSClassRelease(globalClass);
}

void sys_jsc_init_worker() {
  printf("[Prisimi JIT] Initializing JavaScriptCore Matrix (Worker)...\n");
  // Context Separation Rule: Workers must have their own group
  JSContextGroupRef group = JSContextGroupCreate();

  JSClassDefinition globalClassDef = kJSClassDefinitionEmpty;
  JSClassRef globalClass = JSClassCreate(&globalClassDef);

  global_ctx = JSGlobalContextCreateInGroup(group, globalClass);

  // Workers usually don't need DOM classes, but they need globals
  bind_native_globals(global_ctx);

  JSClassRelease(globalClass);
}

void sys_jsc_evaluate_script(uint64_t script_ptr, uint32_t script_len,
                             const char *filename) {
  if (!global_ctx)
    return;

  // Create a null-terminated copy for JSStringCreateWithUTF8CString
  // Bounded allocation: only one copy during the transition to JSC internal
  // representation
  char *code = (char *)malloc(script_len + 1);
  if (!code) return;
  memcpy(code, (void *)(uintptr_t)script_ptr, script_len);
  code[script_len] = '\0';

  if (script_len > 0) {
      char snippet[128] = {0};
      strncpy(snippet, code, script_len < 127 ? script_len : 127);
      printf("[Prisimi JIT] Evaluating Script (len=%u): %s...\n", script_len, snippet);
      fflush(stdout);
  }

  JSStringRef scriptJS = JSStringCreateWithUTF8CString(code);
  JSStringRef fileJS = NULL;
  if (filename) {
      char safe_filename[256] = {0};
      // We must check character by character up to 255 to prevent reading out of mapped bounds
      // in case the pointer from Salt is not null-terminated.
      for (int i = 0; i < 255; i++) {
          if (filename[i] == '\0') break;
          safe_filename[i] = filename[i];
      }
      fileJS = JSStringCreateWithUTF8CString(safe_filename);
  }

  JSValueRef exception = NULL;
  JSEvaluateScript(global_ctx, scriptJS, NULL, fileJS, 1, &exception);

  if (exception) {
    sys_jsc_dump_exception(global_ctx, exception);
  }

  JSStringRelease(scriptJS);
  if (fileJS)
    JSStringRelease(fileJS);
  free(code);
}

// Epic 85: Evaluate a JS expression and return its numeric result
// Used by E2E tests to assert JS global state from Salt
double sys_jsc_eval_to_number(uint64_t script_ptr, uint32_t script_len) {
  if (!global_ctx)
    return -999.0;

  char *code = (char *)malloc(script_len + 1);
  if (!code) return -999.0;
  memcpy(code, (void *)(uintptr_t)script_ptr, script_len);
  code[script_len] = '\0';

  JSStringRef scriptJS = JSStringCreateWithUTF8CString(code);
  JSValueRef exception = NULL;
  JSValueRef result =
      JSEvaluateScript(global_ctx, scriptJS, NULL, NULL, 1, &exception);
  JSStringRelease(scriptJS);
  free(code);

  if (exception || !result)
    return -999.0;
  if (JSValueIsNumber(global_ctx, result)) {
    return JSValueToNumber(global_ctx, result, NULL);
  }
  return -999.0;
}

// Epic 85: Flush pending IDB open requests (called from Salt run loop)
extern void sys_jsc_flush_idb_open_requests(void);
void sys_jsc_pump_idb_open(void) { sys_jsc_flush_idb_open_requests(); }

// Microtask flushing logic
// JSC doesn't expose a simple "execute microtasks" like QuickJS.
// However, JSEvaluateScript runs them after the script finishes.
// For async operations (timers, fetch), they run on the next turn of the run
// loop or when we return to the engine. The Architect wants us to "manually
// implement the microtask queue (Promises) and flush it exactly once per 16ms
// render frame". In JSC, promise rejections and resolutions are handled
// internally. We can use JSGlobalContextSetInspectable if we were on iOS/macOS
// newer versions, but for raw C API, we might need to hook into the rejection
// tracker if we wanted custom behavior. For now, we'll assume JSC handles them
// internally within the Context. Observer queue externs — Salt globals use
// mangled package names
extern uint32_t user__browser__observers__PENDING_RESIZE_NODES[256];
extern uint32_t user__browser__observers__PENDING_RESIZE_COUNT;
extern uint32_t user__browser__observers__PENDING_MUTATION_NODES[256];
extern uint8_t user__browser__observers__PENDING_MUTATION_TYPES[256];
extern uint32_t user__browser__observers__PENDING_MUTATION_COUNT;
extern uint32_t user__browser__observers__PENDING_INTERSECTION_NODES[256];
extern float user__browser__observers__PENDING_INTERSECTION_RATIOS[256];
extern uint32_t user__browser__observers__PENDING_INTERSECTION_COUNT;

#define PENDING_RESIZE_NODES user__browser__observers__PENDING_RESIZE_NODES
#define PENDING_RESIZE_COUNT user__browser__observers__PENDING_RESIZE_COUNT
#define PENDING_MUTATION_NODES user__browser__observers__PENDING_MUTATION_NODES
#define PENDING_MUTATION_TYPES user__browser__observers__PENDING_MUTATION_TYPES
#define PENDING_MUTATION_COUNT user__browser__observers__PENDING_MUTATION_COUNT
#define PENDING_INTERSECTION_NODES                                             \
  user__browser__observers__PENDING_INTERSECTION_NODES
#define PENDING_INTERSECTION_RATIOS                                            \
  user__browser__observers__PENDING_INTERSECTION_RATIOS
#define PENDING_INTERSECTION_COUNT                                             \
  user__browser__observers__PENDING_INTERSECTION_COUNT

extern JSObjectRef create_js_node_wrapper(JSContextRef ctx, uint64_t node_id);
extern uint64_t sys_get_callback_for_node(uint32_t node_id, uint8_t obs_type);
extern void sys_observers_clear_queues();
extern float dom_get_layout_w(uint32_t node_idx);
extern float dom_get_layout_h(uint32_t node_idx);

void sys_jsc_flush_observer_queues() {
  if (!global_ctx)
    return;
  if (PENDING_RESIZE_COUNT == 0 && PENDING_MUTATION_COUNT == 0 &&
      PENDING_INTERSECTION_COUNT == 0)
    return;

  // 1. Process Resize Observers
  for (uint32_t i = 0; i < PENDING_RESIZE_COUNT; i++) {
    uint32_t node_id = PENDING_RESIZE_NODES[i];
    JSObjectRef callback =
        (JSObjectRef)(uintptr_t)sys_get_callback_for_node(node_id, 2);
    if (!callback)
      continue;

    JSObjectRef target_node = create_js_node_wrapper(global_ctx, node_id);
    JSObjectRef contentRect = JSObjectMake(global_ctx, NULL, NULL);
    JSStringRef wStr = JSStringCreateWithUTF8CString("width");
    JSStringRef hStr = JSStringCreateWithUTF8CString("height");
    JSObjectSetProperty(
        global_ctx, contentRect, wStr,
        JSValueMakeNumber(global_ctx, dom_get_layout_w(node_id)), 0, NULL);
    JSObjectSetProperty(
        global_ctx, contentRect, hStr,
        JSValueMakeNumber(global_ctx, dom_get_layout_h(node_id)), 0, NULL);
    JSStringRelease(wStr);
    JSStringRelease(hStr);

    JSObjectRef entry = JSObjectMake(global_ctx, NULL, NULL);
    JSStringRef targetStr = JSStringCreateWithUTF8CString("target");
    JSStringRef rectStr = JSStringCreateWithUTF8CString("contentRect");
    JSObjectSetProperty(global_ctx, entry, targetStr, target_node, 0, NULL);
    JSObjectSetProperty(global_ctx, entry, rectStr, contentRect, 0, NULL);
    JSStringRelease(targetStr);
    JSStringRelease(rectStr);

    JSValueRef entries[] = {entry};
    JSObjectRef entriesArray = JSObjectMakeArray(global_ctx, 1, entries, NULL);
    JSValueRef args[] = {entriesArray};
    JSValueRef exception = NULL;
    JSObjectCallAsFunction(global_ctx, callback, NULL, 1, args, &exception);
    if (exception) {
      JSStringRef excStr = JSValueToStringCopy(global_ctx, exception, NULL);
      size_t max_sz = JSStringGetMaximumUTF8CStringSize(excStr);
      char *buf = malloc(max_sz);
      if (buf) {
        JSStringGetUTF8CString(excStr, buf, max_sz);
        printf("[Prisimi JSC] ❌ ResizeObserver Exception: %s\n", buf);
        free(buf);
      }
      JSStringRelease(excStr);
    }
  }

  // 2. Process Mutation Observers
  for (uint32_t i = 0; i < PENDING_MUTATION_COUNT; i++) {
    uint32_t node_id = PENDING_MUTATION_NODES[i];
    uint8_t type = PENDING_MUTATION_TYPES[i];
    JSObjectRef callback =
        (JSObjectRef)(uintptr_t)sys_get_callback_for_node(node_id, 1);
    if (!callback)
      continue;

    JSObjectRef target_node = create_js_node_wrapper(global_ctx, node_id);
    JSObjectRef record = JSObjectMake(global_ctx, NULL, NULL);
    const char *typeStrC = (type == 3) ? "characterData" : "childList";
    JSStringRef typeStr = JSStringCreateWithUTF8CString("type");
    JSStringRef typeVal = JSStringCreateWithUTF8CString(typeStrC);
    JSObjectSetProperty(global_ctx, record, typeStr,
                        JSValueMakeString(global_ctx, typeVal), 0, NULL);
    JSStringRelease(typeStr);
    JSStringRelease(typeVal);

    JSStringRef targetStr = JSStringCreateWithUTF8CString("target");
    JSObjectSetProperty(global_ctx, record, targetStr, target_node, 0, NULL);
    JSStringRelease(targetStr);

    JSValueRef records[] = {record};
    JSObjectRef recordsArray = JSObjectMakeArray(global_ctx, 1, records, NULL);
    JSValueRef args[] = {recordsArray};
    JSValueRef exception = NULL;
    JSObjectCallAsFunction(global_ctx, callback, NULL, 1, args, &exception);
    if (exception) {
      JSStringRef excStr = JSValueToStringCopy(global_ctx, exception, NULL);
      size_t max_sz = JSStringGetMaximumUTF8CStringSize(excStr);
      char *buf = malloc(max_sz);
      if (buf) {
        JSStringGetUTF8CString(excStr, buf, max_sz);
        printf("[Prisimi JSC] ❌ MutationObserver Exception: %s\n", buf);
        free(buf);
      }
      JSStringRelease(excStr);
    }
  }

  // 3. Process Intersection Observers
  for (uint32_t i = 0; i < PENDING_INTERSECTION_COUNT; i++) {
    uint32_t node_id = PENDING_INTERSECTION_NODES[i];
    float ratio = PENDING_INTERSECTION_RATIOS[i];
    JSObjectRef callback =
        (JSObjectRef)(uintptr_t)sys_get_callback_for_node(node_id, 3);
    if (!callback)
      continue;

    JSObjectRef target_node = create_js_node_wrapper(global_ctx, node_id);
    JSObjectRef entry = JSObjectMake(global_ctx, NULL, NULL);
    JSValueRef ratioVal = JSValueMakeNumber(global_ctx, ratio);

    JSStringRef targetStr = JSStringCreateWithUTF8CString("target");
    JSStringRef ratioStr = JSStringCreateWithUTF8CString("intersectionRatio");
    JSObjectSetProperty(global_ctx, entry, targetStr, target_node, 0, NULL);
    JSObjectSetProperty(global_ctx, entry, ratioStr, ratioVal, 0, NULL);
    JSStringRelease(targetStr);
    JSStringRelease(ratioStr);

    JSValueRef entries[] = {entry};
    JSObjectRef entriesArray = JSObjectMakeArray(global_ctx, 1, entries, NULL);
    JSValueRef args[] = {entriesArray};
    JSValueRef exception = NULL;
    JSObjectCallAsFunction(global_ctx, callback, NULL, 1, args, &exception);
    if (exception) {
      JSStringRef excStr = JSValueToStringCopy(global_ctx, exception, NULL);
      size_t max_sz = JSStringGetMaximumUTF8CStringSize(excStr);
      char *buf = malloc(max_sz);
      if (buf) {
        JSStringGetUTF8CString(excStr, buf, max_sz);
        printf("[Prisimi JSC] ❌ IntersectionObserver Exception: %s\n", buf);
        free(buf);
      }
      JSStringRelease(excStr);
    }
  }

  sys_observers_clear_queues();
}

void sys_jsc_flush_microtasks() {
  // JSC typically handles microtasks automatically when returning control or at
  // the end of scripts. If we need a manual flush, we might need a dummy
  // evaluation or similar, but usually JSC's internal job queue is not directly
  // exposed for manual stepping unless we use the internal private APIs.
}

void sys_jsc_gc() {
  if (global_ctx) {
    printf("[Prisimi JIT] Triggering Explicit Garbage Collection...\n");
    JSGarbageCollect(global_ctx);
  }
}

void sys_jsc_teardown() {
  if (global_ctx) {
    extern void jsc_bindings_teardown(JSContextRef ctx);
    jsc_bindings_teardown(global_ctx);
    JSGlobalContextRelease(global_ctx);
    global_ctx = NULL;
  }
}
