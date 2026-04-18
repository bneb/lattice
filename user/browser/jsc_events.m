#import <JavaScriptCore/JavaScriptCore.h>
#import <Foundation/Foundation.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern JSGlobalContextRef global_ctx;
extern JSObjectRef get_cached_js_wrapper(uint64_t node_id);
extern uint32_t sys_hit_test(float x, float y, uint32_t root_node);
extern JSClassRef dom_node_class;

// Hashing helper
static uint32_t fnv1a_hash_str(const char *str) {
    uint32_t hash = 2166136261u;
    for (int i = 0; str[i]; i++) {
        hash ^= (uint8_t)str[i];
        hash *= 16777619;
    }
    return hash;
}

static JSClassRef event_class = NULL;

static JSValueRef event_stopPropagation(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef* exception) {
    uintptr_t priv = (uintptr_t)JSObjectGetPrivate(thisObject);
    priv |= 1; // Set stopPropagation flag
    JSObjectSetPrivate(thisObject, (void*)priv);
    return JSValueMakeUndefined(ctx);
}

static JSValueRef event_stopImmediatePropagation(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef* exception) {
    uintptr_t priv = (uintptr_t)JSObjectGetPrivate(thisObject);
    priv |= 3; // Set both stopPropagation and stopImmediatePropagation flags
    JSObjectSetPrivate(thisObject, (void*)priv);
    return JSValueMakeUndefined(ctx);
}

JSObjectRef construct_native_js_event(uint32_t type_hash, float client_x, float client_y) {
    if (!event_class) {
        JSClassDefinition def = kJSClassDefinitionEmpty;
        def.className = "Event";
        event_class = JSClassCreate(&def);
    }
    JSObjectRef evt = JSObjectMake(global_ctx, event_class, (void*)0);
    
    JSStringRef x_str = JSStringCreateWithUTF8CString("clientX");
    JSObjectSetProperty(global_ctx, evt, x_str, JSValueMakeNumber(global_ctx, client_x), kJSPropertyAttributeReadOnly, NULL);
    JSStringRelease(x_str);

    JSStringRef y_str = JSStringCreateWithUTF8CString("clientY");
    JSObjectSetProperty(global_ctx, evt, y_str, JSValueMakeNumber(global_ctx, client_y), kJSPropertyAttributeReadOnly, NULL);
    JSStringRelease(y_str);
    
    // Add methods
    JSStringRef sp_str = JSStringCreateWithUTF8CString("stopPropagation");
    JSObjectSetProperty(global_ctx, evt, sp_str, JSObjectMakeFunctionWithCallback(global_ctx, sp_str, event_stopPropagation), kJSPropertyAttributeReadOnly, NULL);
    JSStringRelease(sp_str);

    JSStringRef sip_str = JSStringCreateWithUTF8CString("stopImmediatePropagation");
    JSObjectSetProperty(global_ctx, evt, sip_str, JSObjectMakeFunctionWithCallback(global_ctx, sip_str, event_stopImmediatePropagation), kJSPropertyAttributeReadOnly, NULL);
    JSStringRelease(sip_str);
    
    return evt;
}

int check_stop_propagation(JSObjectRef event) {
    uintptr_t priv = (uintptr_t)JSObjectGetPrivate(event);
    return (priv & 1) != 0;
}

int check_stop_immediate_propagation(JSObjectRef event) {
    uintptr_t priv = (uintptr_t)JSObjectGetPrivate(event);
    return (priv & 2) != 0;
}

extern void ext_events_invoke(uint32_t node_id, uint32_t type_hash, uint64_t js_event, uint8_t is_capture);
extern uint32_t ext_dom_get_parent_idx(uint32_t node_idx);
extern uint32_t dom_get_generation(uint32_t node_idx);

void invoke_listeners(uint32_t node_idx, uint32_t type_hash, JSObjectRef js_event, uint8_t is_capture) {
    ext_events_invoke(node_idx, type_hash, (uint64_t)js_event, is_capture);
}

void ext_jsc_call_event_callback(uint64_t callback_ptr, uint64_t js_event_ptr) {
    if (!global_ctx) return;
    JSObjectRef callback = (JSObjectRef)callback_ptr;
    JSObjectRef js_event = (JSObjectRef)js_event_ptr;
    JSValueRef arg = js_event;
    JSObjectCallAsFunction(global_ctx, callback, NULL, 1, &arg, NULL);
}

void ext_jsc_unprotect_callback(uint64_t callback_ptr) {
    if (!global_ctx) return;
    JSObjectRef callback = (JSObjectRef)callback_ptr;
    JSValueUnprotect(global_ctx, callback);
}

void sys_jsc_dispatch_event(uint32_t target_node_idx, uint32_t type_hash, float client_x, float client_y) {
    if (!global_ctx) return;
    
    // 1. Build Ancestor Chain
    uint32_t ancestors[128];
    int ancestor_count = 0;
    uint32_t curr = ext_dom_get_parent_idx(target_node_idx);
    while (curr != 0 && curr != 999999 && ancestor_count < 128) {
        ancestors[ancestor_count++] = curr;
        curr = ext_dom_get_parent_idx(curr);
    }
    
    // 2. Construct JS Event Object
    JSObjectRef js_event = construct_native_js_event(type_hash, client_x, client_y);
    JSValueProtect(global_ctx, js_event);

    // 3. CAPTURE PHASE (Iterate backwards: Root -> Parent)
    for (int i = ancestor_count - 1; i >= 0; i--) {
        if (check_stop_propagation(js_event)) break;
        invoke_listeners(ancestors[i], type_hash, js_event, 1 /* capture */);
    }
    
    // 4. TARGET PHASE
    if (!check_stop_propagation(js_event)) {
        // Target phase fires capture then bubble listeners
        invoke_listeners(target_node_idx, type_hash, js_event, 1); // Capture listeners
        if (!check_stop_immediate_propagation(js_event)) {
            invoke_listeners(target_node_idx, type_hash, js_event, 0); // Bubble listeners
        }
    }
    
    // 5. BUBBLING PHASE (Iterate forwards: Parent -> Root)
    for (int i = 0; i < ancestor_count; i++) {
        if (check_stop_propagation(js_event)) break;
        invoke_listeners(ancestors[i], type_hash, js_event, 0 /* bubble */);
    }
    
    JSValueUnprotect(global_ctx, js_event);
}

void sys_on_mouse_click(int32_t x, int32_t y) {
    uint32_t target_node_idx = sys_hit_test((float)x, (float)y, 1); // 1 = BODY
    if (target_node_idx == 0) return;
    
    uint32_t type_hash = fnv1a_hash_str("click");
    sys_jsc_dispatch_event(target_node_idx, type_hash, (float)x, (float)y);
}
