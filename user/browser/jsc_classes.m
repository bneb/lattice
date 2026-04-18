#import <JavaScriptCore/JavaScriptCore.h>
#import <Foundation/Foundation.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// Extern Salt symbols
extern void ext_dom_free_node(uint32_t node_id);
extern uint64_t dom_get_text_ptr(uint32_t idx);
extern uint32_t dom_get_text_len(uint32_t idx);
extern uint32_t dom_get_tag(uint32_t idx);

extern uint32_t ext_observers_get_and_clear_callbacks(uint32_t node_id, uint64_t out_buf_ptr);
extern JSGlobalContextRef global_ctx;

// Extern binding functions (defined in jsc_bindings.m)
extern JSValueRef get_node_text_content(JSContextRef ctx, JSObjectRef object, JSStringRef propertyName, JSValueRef* exception);
extern bool set_node_text_content(JSContextRef ctx, JSObjectRef object, JSStringRef propertyName, JSValueRef value, JSValueRef* exception);
extern JSValueRef get_node_type(JSContextRef ctx, JSObjectRef object, JSStringRef propertyName, JSValueRef* exception);
extern JSValueRef get_node_tag_name(JSContextRef ctx, JSObjectRef object, JSStringRef propertyName, JSValueRef* exception);

// Epic 80A: DOM API port functions (defined in jsc_bindings.m)
extern JSValueRef jsc_node_addEventListener(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef* exception);
extern JSValueRef jsc_node_removeEventListener(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef* exception);
extern JSValueRef jsc_node_dispatchEvent(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef* exception);
extern JSValueRef jsc_node_setAttribute(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef* exception);
extern JSValueRef jsc_node_getAttribute(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef* exception);
extern JSValueRef jsc_node_appendChild(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef* exception);
extern JSValueRef jsc_node_removeChild(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef* exception);
extern JSValueRef get_node_innerHTML(JSContextRef ctx, JSObjectRef object, JSStringRef propertyName, JSValueRef* exception);
extern bool set_node_innerHTML(JSContextRef ctx, JSObjectRef object, JSStringRef propertyName, JSValueRef value, JSValueRef* exception);
extern JSValueRef get_node_parentNode(JSContextRef ctx, JSObjectRef object, JSStringRef propertyName, JSValueRef* exception);
extern JSValueRef get_node_childNodes(JSContextRef ctx, JSObjectRef object, JSStringRef propertyName, JSValueRef* exception);
extern JSValueRef get_node_style(JSContextRef ctx, JSObjectRef object, JSStringRef propertyName, JSValueRef* exception);
extern JSValueRef get_node_classList(JSContextRef ctx, JSObjectRef object, JSStringRef propertyName, JSValueRef* exception);
extern JSValueRef get_node_firstChild(JSContextRef ctx, JSObjectRef object, JSStringRef propertyName, JSValueRef* exception);
extern JSValueRef get_node_nextSibling(JSContextRef ctx, JSObjectRef object, JSStringRef propertyName, JSValueRef* exception);
extern JSValueRef get_node_id(JSContextRef ctx, JSObjectRef object, JSStringRef propertyName, JSValueRef* exception);
extern bool set_node_id(JSContextRef ctx, JSObjectRef object, JSStringRef propertyName, JSValueRef value, JSValueRef* exception);
extern JSValueRef get_node_className(JSContextRef ctx, JSObjectRef object, JSStringRef propertyName, JSValueRef* exception);
extern bool set_node_className(JSContextRef ctx, JSObjectRef object, JSStringRef propertyName, JSValueRef value, JSValueRef* exception);
extern JSObjectRef jsc_Event_constructor(JSContextRef ctx, JSObjectRef constructor, size_t argc, const JSValueRef argv[], JSValueRef* exception);
extern JSValueRef jsc_document_createTextNode(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef* exception);

// Epic 80A Wave 4: remaining externs
extern JSValueRef jsc_node_insertBefore(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef* exception);
extern JSValueRef jsc_node_replaceChild(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef* exception);
extern JSValueRef jsc_node_removeAttribute(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef* exception);
extern JSValueRef jsc_node_click(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef* exception);
extern JSValueRef jsc_node_attachShadow(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef* exception);
extern JSValueRef jsc_node_getContext(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef* exception);
extern JSValueRef jsc_Element_animate(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argc, const JSValueRef argv[], JSValueRef* exception);
extern JSValueRef get_node_nodeValue(JSContextRef ctx, JSObjectRef object, JSStringRef pn, JSValueRef* ex);
extern bool set_node_nodeValue(JSContextRef ctx, JSObjectRef object, JSStringRef pn, JSValueRef v, JSValueRef* ex);
extern JSValueRef get_node_value(JSContextRef ctx, JSObjectRef object, JSStringRef pn, JSValueRef* ex);
extern bool set_node_value(JSContextRef ctx, JSObjectRef object, JSStringRef pn, JSValueRef v, JSValueRef* ex);
extern JSValueRef get_node_src(JSContextRef ctx, JSObjectRef object, JSStringRef pn, JSValueRef* ex);
extern bool set_node_src(JSContextRef ctx, JSObjectRef object, JSStringRef pn, JSValueRef v, JSValueRef* ex);
extern bool set_node_scrollTop(JSContextRef ctx, JSObjectRef object, JSStringRef pn, JSValueRef value, JSValueRef* ex);
extern JSValueRef get_node_scrollTop(JSContextRef ctx, JSObjectRef object, JSStringRef pn, JSValueRef* ex);
extern JSObjectRef jsc_HTMLElement_constructor(JSContextRef ctx, JSObjectRef constructor, size_t argc, const JSValueRef argv[], JSValueRef* exception);

JSClassRef dom_node_class = NULL;

void dom_node_finalize(JSObjectRef object) {
    uint64_t node_id_packed = (uint64_t)(uintptr_t)JSObjectGetPrivate(object);
    uint32_t node_idx = (uint32_t)(node_id_packed & 0xFFFF);
    if (node_idx != 0) {
        if (global_ctx) {
            uint64_t callbacks[3] = {0, 0, 0};
            uint32_t count = ext_observers_get_and_clear_callbacks(node_idx, (uint64_t)(uintptr_t)callbacks);
            for (uint32_t i = 0; i < count; i++) {
                if (callbacks[i]) {
                    JSValueUnprotect(global_ctx, (JSValueRef)(uintptr_t)callbacks[i]);
                }
            }
        }
        ext_dom_free_node(node_idx);
    }
}

JSValueRef get_node_index(JSContextRef ctx, JSObjectRef object, JSStringRef propertyName, JSValueRef* exception) {
    uint64_t node_id_packed = (uint64_t)(uintptr_t)JSObjectGetPrivate(object);
    uint32_t node_idx = (uint32_t)(node_id_packed & 0xFFFF);
    return JSValueMakeNumber(ctx, (double)node_idx);
}

void init_dom_classes(JSGlobalContextRef ctx) {
    printf("[Prisimi JIT] Initializing Object Class Matrix...\n");
    
    // --- Node Class ---
    JSClassDefinition nodeDef = kJSClassDefinitionEmpty;
    nodeDef.className = "Node";
    nodeDef.finalize = dom_node_finalize;
    
    static JSStaticValue nodeValues[] = {
        { "textContent", get_node_text_content, set_node_text_content, kJSPropertyAttributeNone },
        { "nodeType", get_node_type, NULL, kJSPropertyAttributeReadOnly },
        { "tagName", get_node_tag_name, NULL, kJSPropertyAttributeReadOnly },
        { "node_index", get_node_index, NULL, kJSPropertyAttributeReadOnly },
        { "innerHTML", get_node_innerHTML, set_node_innerHTML, kJSPropertyAttributeNone },
        { "parentNode", get_node_parentNode, NULL, kJSPropertyAttributeReadOnly },
        { "childNodes", get_node_childNodes, NULL, kJSPropertyAttributeReadOnly },
        { "style", get_node_style, NULL, kJSPropertyAttributeReadOnly },
        { "classList", get_node_classList, NULL, kJSPropertyAttributeReadOnly },
        { "firstChild", get_node_firstChild, NULL, kJSPropertyAttributeReadOnly },
        { "nextSibling", get_node_nextSibling, NULL, kJSPropertyAttributeReadOnly },
        { "id", get_node_id, set_node_id, kJSPropertyAttributeNone },
        { "className", get_node_className, set_node_className, kJSPropertyAttributeNone },
        { "nodeValue", get_node_nodeValue, set_node_nodeValue, kJSPropertyAttributeNone },
        { "value", get_node_value, set_node_value, kJSPropertyAttributeNone },
        { "scrollTop", get_node_scrollTop, set_node_scrollTop, kJSPropertyAttributeNone },
        { "src", get_node_src, set_node_src, kJSPropertyAttributeNone },
        { 0, 0, 0, 0 }
    };
    nodeDef.staticValues = nodeValues;
    
    // Epic 80A: DOM methods
    static JSStaticFunction nodeFuncs[] = {
        { "addEventListener", jsc_node_addEventListener, kJSPropertyAttributeNone },
        { "removeEventListener", jsc_node_removeEventListener, kJSPropertyAttributeNone },
        { "dispatchEvent", jsc_node_dispatchEvent, kJSPropertyAttributeNone },
        { "setAttribute", jsc_node_setAttribute, kJSPropertyAttributeNone },
        { "getAttribute", jsc_node_getAttribute, kJSPropertyAttributeNone },
        { "appendChild", jsc_node_appendChild, kJSPropertyAttributeNone },
        { "removeChild", jsc_node_removeChild, kJSPropertyAttributeNone },
        { "insertBefore", jsc_node_insertBefore, kJSPropertyAttributeNone },
        { "replaceChild", jsc_node_replaceChild, kJSPropertyAttributeNone },
        { "removeAttribute", jsc_node_removeAttribute, kJSPropertyAttributeNone },
        { "click", jsc_node_click, kJSPropertyAttributeNone },
        { "attachShadow", jsc_node_attachShadow, kJSPropertyAttributeNone },
        { "getContext", jsc_node_getContext, kJSPropertyAttributeNone },
        { "animate", jsc_Element_animate, kJSPropertyAttributeNone },
        { 0, 0, 0 }
    };
    nodeDef.staticFunctions = nodeFuncs;
    
    dom_node_class = JSClassCreate(&nodeDef);
    
    JSObjectRef global = JSContextGetGlobalObject(ctx);
    JSStringRef nodeName = JSStringCreateWithUTF8CString("Node");
    JSObjectRef nodeConstructor = JSObjectMakeConstructor(ctx, dom_node_class, NULL);
    JSObjectSetProperty(ctx, global, nodeName, nodeConstructor, kJSPropertyAttributeDontEnum, NULL);
    JSStringRelease(nodeName);
    
    // Epic 80A: Event constructor — new Event('type')
    JSStringRef eventName = JSStringCreateWithUTF8CString("Event");
    JSObjectRef eventCtor = JSObjectMakeConstructor(ctx, NULL, jsc_Event_constructor);
    JSObjectSetProperty(ctx, global, eventName, eventCtor, kJSPropertyAttributeNone, NULL);
    JSStringRelease(eventName);
    
    // HTMLElement constructor for Custom Elements
    JSStringRef htmlElemName = JSStringCreateWithUTF8CString("HTMLElement");
    JSObjectRef htmlElemCtor = JSObjectMakeConstructor(ctx, dom_node_class, jsc_HTMLElement_constructor);
    JSObjectSetProperty(ctx, global, htmlElemName, htmlElemCtor, kJSPropertyAttributeNone, NULL);
    JSStringRelease(htmlElemName);
}

static JSObjectRef node_wrapper_cache[65536] = {0};

extern JSObjectRef get_ce_prototype(JSContextRef ctx, uint32_t tag_hash);
extern uint32_t dom_get_tag(uint32_t node_idx);

JSObjectRef create_js_node_wrapper(JSContextRef ctx, uint64_t node_id) {
    uint32_t idx = (uint32_t)(node_id & 0xFFFF);
    if (node_wrapper_cache[idx]) {
        uint64_t cached_packed = (uint64_t)(uintptr_t)JSObjectGetPrivate(node_wrapper_cache[idx]);
        if (cached_packed == node_id) {
            return node_wrapper_cache[idx];
        }
        // Generation changed, unprotect the old and make a new one!
        JSValueUnprotect(ctx, node_wrapper_cache[idx]);
        node_wrapper_cache[idx] = NULL;
    }
    
    JSObjectRef wrapper = JSObjectMake(ctx, dom_node_class, (void*)(uintptr_t)node_id);
    
    // If it's a custom element, we set its prototype to the registered constructor's prototype
    uint32_t tag = dom_get_tag(idx);
    if (tag > 100) { // arbitrary threshold for custom hashes vs enums
        JSObjectRef proto = get_ce_prototype(ctx, tag);
        if (proto) {
            JSObjectSetPrototype(ctx, wrapper, proto);
        }
    }
    
    node_wrapper_cache[idx] = wrapper;
    JSValueProtect(ctx, wrapper);
    return wrapper;
}

JSObjectRef get_cached_js_wrapper(uint64_t node_id) {
    uint32_t idx = (node_id & 0xFFFF);
    if (node_wrapper_cache[idx]) {
        uint64_t cached_packed = (uint64_t)(uintptr_t)JSObjectGetPrivate(node_wrapper_cache[idx]);
        if (cached_packed == node_id) {
            return node_wrapper_cache[idx];
        }
    }
    return NULL;
}

void ext_jsc_trigger_connected_callback(uint64_t node_id) {
    if (!global_ctx) return;
    JSObjectRef js_node = get_cached_js_wrapper(node_id);
    if (!js_node) return;
    
    // Check if the JS object has a 'connectedCallback' property
    JSStringRef cb_name = JSStringCreateWithUTF8CString("connectedCallback");
    if (JSObjectHasProperty(global_ctx, js_node, cb_name)) {
        JSValueRef cb_val = JSObjectGetProperty(global_ctx, js_node, cb_name, NULL);
        if (JSValueIsObject(global_ctx, cb_val)) {
            JSObjectRef cb_func = (JSObjectRef)cb_val;
            if (JSObjectIsFunction(global_ctx, cb_func)) {
                // Execute synchronously
                JSObjectCallAsFunction(global_ctx, cb_func, js_node, 0, NULL, NULL);
            }
        }
    }
    JSStringRelease(cb_name);
}

void ext_jsc_trigger_disconnected_callback(uint64_t node_id) {
    if (!global_ctx) return;
    JSObjectRef js_node = get_cached_js_wrapper(node_id);
    if (!js_node) return;
    
    JSStringRef cb_name = JSStringCreateWithUTF8CString("disconnectedCallback");
    if (JSObjectHasProperty(global_ctx, js_node, cb_name)) {
        JSValueRef cb_val = JSObjectGetProperty(global_ctx, js_node, cb_name, NULL);
        if (JSValueIsObject(global_ctx, cb_val)) {
            JSObjectRef cb_func = (JSObjectRef)cb_val;
            if (JSObjectIsFunction(global_ctx, cb_func)) {
                JSObjectCallAsFunction(global_ctx, cb_func, js_node, 0, NULL, NULL);
            }
        }
    }
    JSStringRelease(cb_name);
}
