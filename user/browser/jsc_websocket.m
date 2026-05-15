#import <Foundation/Foundation.h>
#import <JavaScriptCore/JavaScriptCore.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

extern JSContextRef global_ctx;

extern uint32_t ext_ws_create_connection(const char *url, uint32_t url_len);
extern uint32_t ext_ws_get_pending_message_count();
extern void ext_ws_pop_message(uint32_t *socket_id, uint64_t *payload_ptr,
                               uint32_t *payload_len, uint8_t *is_binary);

static JSObjectRef ws_wrappers[1024];
JSClassRef ws_class = NULL;

JSObjectRef sys_jsc_get_ws_wrapper(uint32_t socket_id) {
  if (socket_id < 1024)
    return ws_wrappers[socket_id];
  return NULL;
}

void sys_jsc_invoke_property_callback(JSObjectRef obj,
                                      const char *property_name,
                                      JSObjectRef arg) {
  if (!global_ctx)
    return;
  JSStringRef prop_str = JSStringCreateWithUTF8CString(property_name);
  JSValueRef callback_val =
      JSObjectGetProperty(global_ctx, obj, prop_str, NULL);
  JSStringRelease(prop_str);

  if (JSValueIsObject(global_ctx, callback_val)) {
    JSObjectRef callback_fn = JSValueToObject(global_ctx, callback_val, NULL);
    if (JSObjectIsFunction(global_ctx, callback_fn)) {
      JSValueRef args[] = {arg};
      JSObjectCallAsFunction(global_ctx, callback_fn, obj, 1, args, NULL);
    }
  }
}

JSObjectRef js_ws_constructor(JSContextRef ctx, JSObjectRef constructor,
                              size_t argumentCount,
                              const JSValueRef arguments[],
                              JSValueRef *exception) {
  if (argumentCount < 1)
    return NULL;

  JSStringRef url_str = JSValueToStringCopy(ctx, arguments[0], exception);
  size_t url_len = JSStringGetMaximumUTF8CStringSize(url_str);
  char *url = malloc(url_len);
  if (!url) {
    JSStringRelease(url_str);
    return NULL;
  }
  JSStringGetUTF8CString(url_str, url, url_len);
  JSStringRelease(url_str);

  uint32_t socket_id = ext_ws_create_connection(url, (uint32_t)strlen(url));
  free(url);

  JSObjectRef ws_instance =
      JSObjectMake(ctx, ws_class, (void *)(uintptr_t)socket_id);
  if (socket_id < 1024) {
    ws_wrappers[socket_id] = ws_instance;
    JSValueProtect(ctx, ws_instance);
  }
  return ws_instance;
}

void sys_jsc_flush_ws_events() {
  if (!global_ctx)
    return;
  uint32_t pending_count = ext_ws_get_pending_message_count();
  for (uint32_t i = 0; i < pending_count; i++) {
    uint32_t socket_id;
    uint64_t payload_ptr;
    uint32_t payload_len;
    uint8_t is_binary;

    ext_ws_pop_message(&socket_id, &payload_ptr, &payload_len, &is_binary);

    JSObjectRef ws_instance = sys_jsc_get_ws_wrapper(socket_id);
    if (!ws_instance)
      continue;

    JSObjectRef event_obj = JSObjectMake(global_ctx, NULL, NULL);

    if (is_binary) {
      JSObjectRef array_buffer = JSObjectMakeArrayBufferWithBytesNoCopy(
          global_ctx, (void *)payload_ptr, payload_len, NULL, NULL, NULL);
      JSStringRef data_str = JSStringCreateWithUTF8CString("data");
      JSObjectSetProperty(global_ctx, event_obj, data_str, array_buffer, 0,
                          NULL);
      JSStringRelease(data_str);
    } else {
      char *text_buf = (char *)malloc(payload_len + 1);
      if (text_buf) {
        memcpy(text_buf, (void *)payload_ptr, payload_len);
        text_buf[payload_len] = '\0';

        JSStringRef text = JSStringCreateWithUTF8CString(text_buf);
        JSStringRef data_str = JSStringCreateWithUTF8CString("data");
        JSObjectSetProperty(global_ctx, event_obj, data_str,
                            JSValueMakeString(global_ctx, text), 0, NULL);
        JSStringRelease(text);
        JSStringRelease(data_str);
        free(text_buf);
      }
    }

    sys_jsc_invoke_property_callback(ws_instance, "onmessage", event_obj);
  }
}

void sys_init_ws_class(JSContextRef ctx) {
  global_ctx = ctx;
  JSClassDefinition classDef = kJSClassDefinitionEmpty;
  classDef.className = "WebSocket";
  classDef.callAsConstructor = js_ws_constructor;
  ws_class = JSClassCreate(&classDef);

  JSObjectRef globalObj = JSContextGetGlobalObject(ctx);
  JSObjectRef ws_constructor_obj = JSObjectMake(ctx, ws_class, NULL);
  JSStringRef ws_str = JSStringCreateWithUTF8CString("WebSocket");
  JSObjectSetProperty(ctx, globalObj, ws_str, ws_constructor_obj,
                      kJSPropertyAttributeNone, NULL);
  JSStringRelease(ws_str);
}
