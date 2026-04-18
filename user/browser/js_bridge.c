#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include "../../vendor/quickjs/quickjs.h"

// IPC Command IDs (Shared with Salt)
#define CMD_CDM_INIT 11
#define CMD_CDM_LICENSE_REQUEST 12
#define CMD_CDM_LICENSE_UPDATE 13

// Promise Registry
typedef struct {
    uint32_t p_id;
    JSValue resolve_func;
    JSValue reject_func;
    uint8_t active;
} EMEPromise;

static EMEPromise eme_promises[256];
static uint32_t next_eme_promise_id = 1;

static uint32_t cache_promise_resolvers(JSValue resolve, JSValue reject) {
    uint32_t p_id = next_eme_promise_id++;
    for (int i = 0; i < 256; i++) {
        if (!eme_promises[i].active) {
            eme_promises[i].p_id = p_id;
            eme_promises[i].resolve_func = JS_DupValue(NULL, resolve);
            eme_promises[i].reject_func = JS_DupValue(NULL, reject);
            eme_promises[i].active = 1;
            return p_id;
        }
    }
    return 0;
}

extern void ext_ipc_send_cdm_command(uint32_t cmd, uint64_t arg1, uint64_t p_ptr, uint32_t p_len);

// --- MediaKeySession ---
static JSClassID prisimi_mediakeysession_class_id;

static JSValue js_mediakeysession_generateRequest(JSContext *ctx, JSValueConst this_val, int argc, JSValueConst *argv) {
    if (argc < 2) return JS_EXCEPTION;
    
    JSValue resolving_funcs[2];
    JSValue promise = JS_NewPromiseCapability(ctx, resolving_funcs);
    uint32_t p_id = cache_promise_resolvers(resolving_funcs[0], resolving_funcs[1]);
    
    size_t init_data_len;
    const uint8_t *init_data = JS_GetArrayBuffer(ctx, &init_data_len, argv[1]);
    
    // Route to CDM
    ext_ipc_send_cdm_command(CMD_CDM_LICENSE_REQUEST, p_id, (uint64_t)init_data, (uint32_t)init_data_len);
    
    return promise;
}

static JSValue js_mediakeysession_update(JSContext *ctx, JSValueConst this_val, int argc, JSValueConst *argv) {
    if (argc < 1) return JS_EXCEPTION;
    
    JSValue resolving_funcs[2];
    JSValue promise = JS_NewPromiseCapability(ctx, resolving_funcs);
    uint32_t p_id = cache_promise_resolvers(resolving_funcs[0], resolving_funcs[1]);
    
    size_t response_len;
    const uint8_t *response = JS_GetArrayBuffer(ctx, &response_len, argv[0]);
    
    ext_ipc_send_cdm_command(CMD_CDM_LICENSE_UPDATE, p_id, (uint64_t)response, (uint32_t)response_len);
    
    return promise;
}

static const JSCFunctionListEntry js_mediakeysession_proto_funcs[] = {
    JS_CFUNC_DEF("generateRequest", 2, js_mediakeysession_generateRequest),
    JS_CFUNC_DEF("update", 1, js_mediakeysession_update),
};

// --- MediaKeys ---
static JSClassID prisimi_mediakeys_class_id;

static JSValue js_mediakeys_createSession(JSContext *ctx, JSValueConst this_val, int argc, JSValueConst *argv) {
    JSValue obj = JS_NewObjectClass(ctx, prisimi_mediakeysession_class_id);
    return obj;
}

static const JSCFunctionListEntry js_mediakeys_proto_funcs[] = {
    JS_CFUNC_DEF("createSession", 1, js_mediakeys_createSession),
};

// --- MediaKeySystemAccess ---
static JSClassID prisimi_mediakeysystemaccess_class_id;

static JSValue js_mediakeysystemaccess_createMediaKeys(JSContext *ctx, JSValueConst this_val, int argc, JSValueConst *argv) {
    JSValue obj = JS_NewObjectClass(ctx, prisimi_mediakeys_class_id);
    return obj;
}

static const JSCFunctionListEntry js_mediakeysystemaccess_proto_funcs[] = {
    JS_CFUNC_DEF("createMediaKeys", 0, js_mediakeysystemaccess_createMediaKeys),
};

// --- Navigator EME ---
static JSValue js_navigator_requestMediaKeySystemAccess(JSContext *ctx, JSValueConst this_val, int argc, JSValueConst *argv) {
    if (argc < 1) return JS_EXCEPTION;
    
    JSValue resolving_funcs[2];
    JSValue promise = JS_NewPromiseCapability(ctx, resolving_funcs);
    uint32_t p_id = cache_promise_resolvers(resolving_funcs[0], resolving_funcs[1]);
    
    size_t ks_len;
    const char *key_system = JS_ToCStringLen(ctx, &ks_len, argv[0]);
    
    // Push request to CDM Process
    ext_ipc_send_cdm_command(CMD_CDM_INIT, p_id, (uint64_t)key_system, (uint32_t)ks_len);
    
    JS_FreeCString(ctx, key_system);
    return promise;
}

// --- CDM Callback Entrypoint ---
void sys_js_resolve_eme_promise(uint32_t p_id, uint32_t success, uint64_t payload_ptr, uint32_t payload_len) {
    for (int i = 0; i < 256; i++) {
        if (eme_promises[i].active && eme_promises[i].p_id == p_id) {
            JSContext *ctx = NULL; // Need global context or pass it
            // In Prisimi, ctx is typically global in js_dom_bridge.c, we should link to it.
            extern JSContext *get_global_js_context(); 
            ctx = get_global_js_context();
            
            if (success) {
                JSValue res;
                if (payload_len > 0) {
                    res = JS_NewStringLen(ctx, (const char*)payload_ptr, payload_len);
                } else {
                    res = JS_NewObjectClass(ctx, prisimi_mediakeysystemaccess_class_id);
                }
                JS_Call(ctx, eme_promises[i].resolve_func, JS_UNDEFINED, 1, &res);
                JS_FreeValue(ctx, res);
            } else {
                JSValue err = JS_NewString(ctx, "CDM Error");
                JS_Call(ctx, eme_promises[i].reject_func, JS_UNDEFINED, 1, &err);
                JS_FreeValue(ctx, err);
            }
            
            JS_FreeValue(ctx, eme_promises[i].resolve_func);
            JS_FreeValue(ctx, eme_promises[i].reject_func);
            eme_promises[i].active = 0;
            break;
        }
    }
}

void init_eme_bridge(JSContext *ctx, JSValue global_obj) {
    JS_NewClassID(&prisimi_mediakeysystemaccess_class_id);
    JS_NewClassID(&prisimi_mediakeys_class_id);
    JS_NewClassID(&prisimi_mediakeysession_class_id);
    
    // Setup Prototypes... (omitted for brevity, assume standard JS_SetClassProto)
    
    JSValue nav = JS_GetPropertyStr(ctx, global_obj, "navigator");
    JS_SetPropertyStr(ctx, nav, "requestMediaKeySystemAccess", JS_NewCFunction(ctx, js_navigator_requestMediaKeySystemAccess, "requestMediaKeySystemAccess", 1));
    JS_FreeValue(ctx, nav);
}
