#import <JavaScriptCore/JavaScriptCore.h>
#import <Foundation/Foundation.h>
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#include <stdio.h>

extern void ext_anim_queue_push(uint32_t node_id, float start_tx, float end_tx, float start_op, float end_op, double duration_ms);

static double sys_jsc_get_double_property(JSContextRef ctx, JSObjectRef obj, const char* name, double default_val) {
    JSStringRef name_str = JSStringCreateWithUTF8CString(name);
    JSValueRef val = JSObjectGetProperty(ctx, obj, name_str, NULL);
    JSStringRelease(name_str);
    if (!JSValueIsUndefined(ctx, val) && !JSValueIsNull(ctx, val)) {
        return JSValueToNumber(ctx, val, NULL);
    }
    return default_val;
}

static void sys_jsc_extract_keyframes(JSContextRef ctx, JSValueRef keyframes_val, float *start_tx, float *end_tx, float *start_op, float *end_op) {
    *start_tx = 0.0f; *end_tx = 0.0f;
    *start_op = 1.0f; *end_op = 1.0f;

    if (!JSValueIsObject(ctx, keyframes_val)) return;
    JSObjectRef keyframes = JSValueToObject(ctx, keyframes_val, NULL);

    JSStringRef transform_str = JSStringCreateWithUTF8CString("transform");
    JSValueRef transform_val = JSObjectGetProperty(ctx, keyframes, transform_str, NULL);
    JSStringRelease(transform_str);

    if (JSValueIsObject(ctx, transform_val)) {
        JSObjectRef transform_arr = JSValueToObject(ctx, transform_val, NULL);
        JSValueRef v0 = JSObjectGetPropertyAtIndex(ctx, transform_arr, 0, NULL);
        JSValueRef v1 = JSObjectGetPropertyAtIndex(ctx, transform_arr, 1, NULL);
        
        JSStringRef s0 = JSValueToStringCopy(ctx, v0, NULL);
        JSStringRef s1 = JSValueToStringCopy(ctx, v1, NULL);
        
        char buf0[256]; char buf1[256];
        JSStringGetUTF8CString(s0, buf0, 256);
        JSStringGetUTF8CString(s1, buf1, 256);
        
        // Simple parser for translateX(Npx)
        const char* p0 = strstr(buf0, "translateX(");
        if (p0) {
            *start_tx = (float)atof(p0 + 11);
        }
        const char* p1 = strstr(buf1, "translateX(");
        if (p1) {
            *end_tx = (float)atof(p1 + 11);
        }
        
        JSStringRelease(s0);
        JSStringRelease(s1);
    }

    JSStringRef opacity_str = JSStringCreateWithUTF8CString("opacity");
    JSValueRef opacity_val = JSObjectGetProperty(ctx, keyframes, opacity_str, NULL);
    JSStringRelease(opacity_str);

    if (JSValueIsObject(ctx, opacity_val)) {
        JSObjectRef opacity_arr = JSValueToObject(ctx, opacity_val, NULL);
        JSValueRef v0 = JSObjectGetPropertyAtIndex(ctx, opacity_arr, 0, NULL);
        JSValueRef v1 = JSObjectGetPropertyAtIndex(ctx, opacity_arr, 1, NULL);
        if (!JSValueIsUndefined(ctx, v0)) *start_op = (float)JSValueToNumber(ctx, v0, NULL);
        if (!JSValueIsUndefined(ctx, v1)) *end_op = (float)JSValueToNumber(ctx, v1, NULL);
    }
}

JSClassRef animation_class = NULL;

JSValueRef jsc_Element_animate(JSContextRef ctx, JSObjectRef function, JSObjectRef thisObject, size_t argumentCount, const JSValueRef arguments[], JSValueRef* exception) {
    if (argumentCount < 2) return JSValueMakeUndefined(ctx);
    
    uint64_t packed_node_id = (uint64_t)(uintptr_t)JSObjectGetPrivate(thisObject);
    uint32_t node_id = (uint32_t)(packed_node_id & 0xFFFF);
    if (!node_id) return JSValueMakeUndefined(ctx);

    // 1. Parse Keyframes (arguments[0])
    float start_tx = 0.0f, end_tx = 0.0f;
    float start_op = 1.0f, end_op = 1.0f;
    sys_jsc_extract_keyframes(ctx, arguments[0], &start_tx, &end_tx, &start_op, &end_op);
    
    // 2. Parse Timing Options (arguments[1])
    JSObjectRef options = JSValueToObject(ctx, arguments[1], exception);
    double duration_ms = sys_jsc_get_double_property(ctx, options, "duration", 1000.0);
    
    // 3. Queue to the Lock-Free Compositor Ring (simulated via Salt bridge for now)
    ext_anim_queue_push(node_id, start_tx, end_tx, start_op, end_op, duration_ms);
    
    // 4. Return W3C Animation Object (stubbed for play()/pause() control)
    if (!animation_class) {
        JSClassDefinition def = kJSClassDefinitionEmpty;
        def.className = "Animation";
        animation_class = JSClassCreate(&def);
    }
    JSObjectRef anim_obj = JSObjectMake(ctx, animation_class, (void*)(uintptr_t)node_id);
    return anim_obj;
}
