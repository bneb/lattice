#import <Foundation/Foundation.h>
#import <JavaScriptCore/JavaScriptCore.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static JSGlobalContextRef global_ctx = NULL;
static JSClassRef fetchevent_class = NULL;

extern void sys_ipc_send_r2m_command_with_payload(uint32_t cmd_type,
                                                  uint64_t arg1, uint64_t p_ptr,
                                                  uint32_t p_len);

// FetchEvent.respondWith
JSValueRef js_fetchevent_respondWith(JSContextRef ctx, JSObjectRef function,
                                     JSObjectRef thisObject, size_t argc,
                                     const JSValueRef argv[],
                                     JSValueRef *exception) {
  if (argc < 1)
    return JSValueMakeUndefined(ctx);

  // Retrieve fetch_id stored in private data
  uint64_t fetch_id = (uint64_t)(uintptr_t)JSObjectGetPrivate(thisObject);

  if (JSValueIsString(ctx, argv[0])) {
    JSStringRef str = JSValueToStringCopy(ctx, argv[0], exception);
    size_t len = JSStringGetMaximumUTF8CStringSize(str);
    char *buf = (char *)malloc(len);
    JSStringGetUTF8CString(str, buf, len);

    // Send CMD_FETCH_RESPONSE (type 9) back to Main
    sys_ipc_send_r2m_command_with_payload(9, fetch_id, (uint64_t)(uintptr_t)buf,
                                          (uint32_t)(strlen(buf)));
    free(buf);
    JSStringRelease(str);
  }
  return JSValueMakeUndefined(ctx);
}

// self.addEventListener
JSValueRef jsc_worker_self_addEventListener(JSContextRef ctx,
                                            JSObjectRef function,
                                            JSObjectRef thisObject, size_t argc,
                                            const JSValueRef argv[],
                                            JSValueRef *exception) {
  if (argc < 2)
    return JSValueMakeUndefined(ctx);

  JSStringRef typeStr = JSValueToStringCopy(ctx, argv[0], exception);
  size_t len = JSStringGetMaximumUTF8CStringSize(typeStr);
  char *type = (char *)malloc(len);
  JSStringGetUTF8CString(typeStr, type, len);

  if (strcmp(type, "fetch") == 0) {
    JSObjectRef global_obj = JSContextGetGlobalObject(ctx);
    JSStringRef onfetchStr = JSStringCreateWithUTF8CString("onfetch");
    JSObjectSetProperty(ctx, global_obj, onfetchStr, argv[1],
                        kJSPropertyAttributeNone, NULL);
    JSStringRelease(onfetchStr);
  }

  free(type);
  JSStringRelease(typeStr);
  return JSValueMakeUndefined(ctx);
}

void sys_jsc_init_worker() {
  printf("[Prisimi JSC] Initializing JavaScriptCore Matrix (Headless "
         "Worker)...\n");

  JSContextGroupRef group = JSContextGroupCreate();

  JSClassDefinition globalClassDef = kJSClassDefinitionEmpty;
  JSClassRef globalClass = JSClassCreate(&globalClassDef);
  global_ctx = JSGlobalContextCreateInGroup(group, globalClass);
  JSClassRelease(globalClass);

  JSObjectRef global_obj = JSContextGetGlobalObject(global_ctx);

  // Define FetchEvent class
  JSClassDefinition feDef = kJSClassDefinitionEmpty;
  feDef.className = "FetchEvent";
  static JSStaticFunction feFuncs[] = {
      {"respondWith", js_fetchevent_respondWith, kJSPropertyAttributeNone},
      {0, 0, 0}};
  feDef.staticFunctions = feFuncs;
  fetchevent_class = JSClassCreate(&feDef);

  // Add self referencing global
  JSStringRef selfStr = JSStringCreateWithUTF8CString("self");
  JSObjectSetProperty(global_ctx, global_obj, selfStr, global_obj,
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(selfStr);

  // Add addEventListener
  JSStringRef aelStr = JSStringCreateWithUTF8CString("addEventListener");
  JSObjectSetProperty(global_ctx, global_obj, aelStr,
                      JSObjectMakeFunctionWithCallback(
                          global_ctx, aelStr, jsc_worker_self_addEventListener),
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(aelStr);

  // Add console.log stub
  JSObjectRef console = JSObjectMake(global_ctx, NULL, NULL);
  JSStringRef logStr = JSStringCreateWithUTF8CString("log");
  JSObjectSetProperty(
      global_ctx, console, logStr,
      JSObjectMakeFunctionWithCallback(global_ctx, logStr, NULL),
      kJSPropertyAttributeNone, NULL);
  JSStringRelease(logStr);

  JSStringRef consoleStr = JSStringCreateWithUTF8CString("console");
  JSObjectSetProperty(global_ctx, global_obj, consoleStr, console,
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(consoleStr);
}

void sys_jsc_evaluate_script(uint64_t script_ptr, uint32_t script_len,
                             const char *filename) {
  if (!global_ctx)
    return;

  char *code = (char *)malloc(script_len + 1);
  memcpy(code, (void *)(uintptr_t)script_ptr, script_len);
  code[script_len] = '\0';

  JSStringRef scriptJS = JSStringCreateWithUTF8CString(code);
  JSStringRef fileJS =
      filename ? JSStringCreateWithUTF8CString(filename) : NULL;

  JSValueRef exception = NULL;
  JSEvaluateScript(global_ctx, scriptJS, NULL, fileJS, 1, &exception);

  if (exception) {
    JSStringRef excStr = JSValueToStringCopy(global_ctx, exception, NULL);
    size_t max_sz = JSStringGetMaximumUTF8CStringSize(excStr);
    char *buf = malloc(max_sz);
    JSStringGetUTF8CString(excStr, buf, max_sz);
    printf("[Prisimi JSC Worker] Exception: %s\n", buf);
    free(buf);
    JSStringRelease(excStr);
  }

  JSStringRelease(scriptJS);
  if (fileJS)
    JSStringRelease(fileJS);
  free(code);
}

void sys_jsc_flush_microtasks() {
  // JSC internal
}

void sys_jsc_dispatch_fetch_event(uint32_t fetch_id, uint64_t url_ptr,
                                  uint32_t url_len) {
  if (!global_ctx)
    return;

  JSObjectRef global_obj = JSContextGetGlobalObject(global_ctx);
  JSStringRef onfetchStr = JSStringCreateWithUTF8CString("onfetch");
  JSValueRef onfetchVal =
      JSObjectGetProperty(global_ctx, global_obj, onfetchStr, NULL);
  JSStringRelease(onfetchStr);

  if (!JSValueIsObject(global_ctx, onfetchVal)) {
    return;
  }
  JSObjectRef onfetch = (JSObjectRef)onfetchVal;

  if (!JSObjectIsFunction(global_ctx, onfetch))
    return;

  // Create new FetchEvent wrapper
  JSObjectRef event_obj =
      JSObjectMake(global_ctx, fetchevent_class, (void *)(uintptr_t)fetch_id);

  // Set request.url
  char *url_buf = (char *)malloc(url_len + 1);
  memcpy(url_buf, (void *)(uintptr_t)url_ptr, url_len);
  url_buf[url_len] = '\0';

  JSObjectRef request_obj = JSObjectMake(global_ctx, NULL, NULL);
  JSStringRef urlStr = JSStringCreateWithUTF8CString("url");
  JSStringRef urlVal = JSStringCreateWithUTF8CString(url_buf);
  JSObjectSetProperty(global_ctx, request_obj, urlStr,
                      JSValueMakeString(global_ctx, urlVal),
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(urlStr);
  JSStringRelease(urlVal);
  free(url_buf);

  JSStringRef reqStr = JSStringCreateWithUTF8CString("request");
  JSObjectSetProperty(global_ctx, event_obj, reqStr, request_obj,
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(reqStr);

  JSValueRef args[] = {event_obj};
  JSValueRef exception = NULL;
  JSObjectCallAsFunction(global_ctx, onfetch, global_obj, 1, args, &exception);

  if (exception) {
    JSStringRef excStr = JSValueToStringCopy(global_ctx, exception, NULL);
    size_t max_sz = JSStringGetMaximumUTF8CStringSize(excStr);
    char *buf = malloc(max_sz);
    JSStringGetUTF8CString(excStr, buf, max_sz);
    printf("[Prisimi JSC Worker] Dispatch Exception: %s\n", buf);
    free(buf);
    JSStringRelease(excStr);
  }
}

// System Stubs
void sys_sleep_ms(uint32_t ms) {
  struct timespec ts;
  ts.tv_sec = ms / 1000;
  ts.tv_nsec = (ms % 1000) * 1000000;
  nanosleep(&ts, NULL);
}

void push_get_request(uint64_t fetch_id, uint64_t url_ptr, uint32_t url_len) {
  // Workers don't push to VirtIO directly
}
