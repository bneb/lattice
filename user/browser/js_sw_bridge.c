#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#include <time.h>
#include "../../vendor/quickjs/quickjs.h"

// Reach into js_dom_bridge.c for shared state if needed, 
// but for headless worker we want total isolation.
static JSRuntime *rt = NULL;
static JSContext *ctx = NULL;

// Epic 76: Service Worker Matrix - Headless Fetch Event Bridge
static JSClassID prisimi_fetchevent_class_id;

static void js_prisimi_fetchevent_finalizer(JSRuntime *rt, JSValue val) {
    // No-op for now
}

static JSClassDef prisimi_fetchevent_class = { "FetchEvent", .finalizer = js_prisimi_fetchevent_finalizer };

static JSValue js_fetchevent_respondWith(JSContext *ctx, JSValueConst this_val, int argc, JSValueConst *argv) {
    uint32_t fetch_id = (uint32_t)(uintptr_t)JS_GetOpaque(this_val, prisimi_fetchevent_class_id);
    if (argc < 1) return JS_EXCEPTION;

    if (JS_IsString(argv[0])) {
        size_t len;
        const char *str = JS_ToCStringLen(ctx, &len, argv[0]);
        // Send CMD_FETCH_RESPONSE (type 9) back to Main
        extern void sys_ipc_send_r2m_command_with_payload(uint32_t cmd_type, uint64_t arg1, uint64_t p_ptr, uint32_t p_len);
        sys_ipc_send_r2m_command_with_payload(9 /* CMD_FETCH_RESPONSE */, fetch_id, (uint64_t)(uintptr_t)str, (uint32_t)len);
        JS_FreeCString(ctx, str);
    }
    
    return JS_UNDEFINED;
}

void sys_js_dispatch_fetch_event(uint32_t fetch_id, uint64_t url_ptr, uint32_t url_len) {
    if (!ctx) return;
    JSValue global_obj = JS_GetGlobalObject(ctx);
    JSValue onfetch = JS_GetPropertyStr(ctx, global_obj, "onfetch");
    
    // Fallback to addEventListener logic if onfetch isn't set directly
    if (!JS_IsFunction(ctx, onfetch)) {
        // Simple listener dispatch for MVP
        JS_FreeValue(ctx, onfetch);
        return; 
    }

    JSValue event_obj = JS_NewObjectClass(ctx, prisimi_fetchevent_class_id);
    JS_SetOpaque(event_obj, (void*)(uintptr_t)fetch_id);
    
    JSValue request_obj = JS_NewObject(ctx);
    JSValue url_str = JS_NewStringLen(ctx, (const char*)url_ptr, url_len);
    JS_SetPropertyStr(ctx, request_obj, "url", url_str);
    JS_SetPropertyStr(ctx, event_obj, "request", request_obj);
    
    JSValue ret = JS_Call(ctx, onfetch, global_obj, 1, &event_obj);
    JS_FreeValue(ctx, ret);
    JS_FreeValue(ctx, event_obj);
    
    JS_FreeValue(ctx, onfetch);
    JS_FreeValue(ctx, global_obj);
}

static JSValue js_worker_self_addEventListener(JSContext *ctx, JSValueConst this_val, int argc, JSValueConst *argv) {
    if (argc < 2) return JS_EXCEPTION;
    const char *type = JS_ToCString(ctx, argv[0]);
    if (strcmp(type, "fetch") == 0) {
        JSValue global_obj = JS_GetGlobalObject(ctx);
        JS_SetPropertyStr(ctx, global_obj, "onfetch", JS_DupValue(ctx, argv[1]));
        JS_FreeValue(ctx, global_obj);
    }
    JS_FreeCString(ctx, type);
    return JS_UNDEFINED;
}

int32_t sys_js_init_worker_context() {
    rt = JS_NewRuntime();
    ctx = JS_NewContext(rt);
    
    JS_NewClassID(&prisimi_fetchevent_class_id);
    JS_NewClass(rt, prisimi_fetchevent_class_id, &prisimi_fetchevent_class);
    
    JSValue global_obj = JS_GetGlobalObject(ctx);
    JSValue fe_proto = JS_NewObject(ctx);
    JS_SetPropertyStr(ctx, fe_proto, "respondWith", JS_NewCFunction(ctx, js_fetchevent_respondWith, "respondWith", 1));
    JS_SetClassProto(ctx, prisimi_fetchevent_class_id, fe_proto);
    
    JS_SetPropertyStr(ctx, global_obj, "self", JS_DupValue(ctx, global_obj));
    JS_SetPropertyStr(ctx, global_obj, "addEventListener", JS_NewCFunction(ctx, js_worker_self_addEventListener, "addEventListener", 2));
    
    // console.log stub
    JSValue console = JS_NewObject(ctx);
    JS_SetPropertyStr(ctx, console, "log", JS_NewCFunction(ctx, NULL, "log", 1)); // Placeholder
    JS_SetPropertyStr(ctx, global_obj, "console", console);

    JS_FreeValue(ctx, global_obj);
    return 1;
}

void sys_js_evaluate_script(const char* code_ptr, uint32_t len, const char* name_ptr, uint32_t name_len) {
    if (!ctx) return;
    char *buf = malloc(len + 1);
    memcpy(buf, code_ptr, len);
    buf[len] = '\0';
    JS_Eval(ctx, buf, len, name_ptr ? name_ptr : "<worker>", JS_EVAL_TYPE_GLOBAL);
    free(buf);
}

void sys_js_flush_microtasks() {
    if (!rt) return;
    JSContext *pctx;
    while (JS_ExecutePendingJob(rt, &pctx) > 0);
}

// System Stubs
void sys_sleep_ms(uint32_t ms) {
    // Basic nanosleep stub for worker
    struct timespec ts;
    ts.tv_sec = ms / 1000;
    ts.tv_nsec = (ms % 1000) * 1000000;
    nanosleep(&ts, NULL);
}

void push_get_request(uint64_t fetch_id, uint64_t url_ptr, uint32_t url_len) {
    // Workers don't push to VirtIO directly
}
