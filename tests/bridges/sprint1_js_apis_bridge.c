#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <JavaScriptCore/JavaScriptCore.h>

// Extern dependencies 
extern void airlock_init_allocator(void);
extern void init_arrays(void);
extern void sys_jsc_init(void);
extern void sys_jsc_teardown(void);

extern void user__browser__css__init_css_defaults(void);
extern uint64_t create_node(uint32_t tag);
extern void http_set_root_node(uint64_t node_id);

extern JSGlobalContextRef global_ctx;

int sprint1_js_apis_test(void) {
    int failures = 0;
    printf("\n[Sprint 1] Testing JavaScript DOM API Mocks for Google\n");

    airlock_init_allocator();
    init_arrays();
    user__browser__css__init_css_defaults();
    
    sys_jsc_init();

    // Create a dummy root node for document.body to correctly attach
    uint64_t js_root = create_node(1); // TAG_HTML
    http_set_root_node(js_root);
    // Needs body and head internally matching typical structure
    uint64_t body_node = create_node(3); // TAG_BODY
    extern void js_dom_append_child(uint32_t parent, uint32_t child);
    js_dom_append_child(1, (uint32_t)(body_node & 0xFFFF));

    const char* test_script = 
        "let errors = [];"
        "try {"
        "  if (typeof window.location === 'undefined') errors.push('window.location missing');"
        "  else if (typeof window.location.href !== 'string') errors.push('location.href invalid');"
        "  "
        "  let div = document.createElement('div');"
        "  if (typeof div.getBoundingClientRect !== 'function') errors.push('getBoundingClientRect missing');"
        "  if (typeof div.offsetWidth !== 'number') errors.push('offsetWidth missing');"
        "  if (typeof div.offsetHeight !== 'number') errors.push('offsetHeight missing');"
        "  if (typeof div.clientWidth !== 'number') errors.push('clientWidth missing');"
        "  if (typeof div.clientHeight !== 'number') errors.push('clientHeight missing');"
        "  "
        "  if (typeof window.innerWidth !== 'number') errors.push('innerWidth missing');"
        "  if (typeof window.innerHeight !== 'number') errors.push('innerHeight missing');"
        "  "
        "  if (typeof atob !== 'function') errors.push('atob missing');"
        "  else if (atob('aGVsbG8=') !== 'hello') errors.push('atob failed');"
        "  "
        "  if (typeof btoa !== 'function') errors.push('btoa missing');"
        "  else if (btoa('hello') !== 'aGVsbG8=') errors.push('btoa failed');"
        "  "
        "  if (typeof document.getElementsByTagName !== 'function') errors.push('getElementsByTagName missing');"
        "  if (typeof document.getElementsByClassName !== 'function') errors.push('getElementsByClassName missing');"
        "  "
        "  if (typeof div.hasAttribute !== 'function') errors.push('hasAttribute missing');"
        "  if (typeof div.cloneNode !== 'function') errors.push('cloneNode missing');"
        "  "
        "  if (typeof div.style !== 'object') errors.push('div.style is missing');"
        "  else {"
        "    div.style.padding = '10px';"
        "    div.style.margin = '10px';"
        "    div.style.color = '#fff';"
        "    div.style.fontSize = '12px';"
        "    div.style.textAlign = 'center';"
        "    div.style.lineHeight = '1';"
        "    div.style.fontFamily = 'Arial';"
        "    div.style.cursor = 'pointer';"
        "    div.style.visibility = 'hidden';"
        "    div.style.whiteSpace = 'nowrap';"
        "    div.style.maxWidth = '100px';"
        "    div.style.minWidth = '50px';"
        "    div.style.boxSizing = 'border-box';"
        "    div.style.borderWidth = '1px';"
        "  }"
        "} catch (e) {"
        "  errors.push('Exception: ' + e.toString());"
        "}"
        "errors.join('|');";

    JSStringRef script_str = JSStringCreateWithUTF8CString(test_script);
    JSValueRef exception = NULL;
    JSValueRef result = JSEvaluateScript(global_ctx, script_str, NULL, NULL, 0, &exception);
    
    if (exception) {
        JSStringRef ex_str = JSValueToStringCopy(global_ctx, exception, NULL);
        size_t len = JSStringGetMaximumUTF8CStringSize(ex_str);
        char* buf = malloc(len);
        JSStringGetUTF8CString(ex_str, buf, len);
        printf("  [FAIL] Script threw exception: %s\n", buf);
        free(buf);
        JSStringRelease(ex_str);
        failures++;
    } else {
        JSStringRef res_str = JSValueToStringCopy(global_ctx, result, NULL);
        size_t len = JSStringGetMaximumUTF8CStringSize(res_str);
        char* buf = malloc(len);
        JSStringGetUTF8CString(res_str, buf, len);
        
        if (strlen(buf) > 0) {
            printf("  [FAIL] Encountered errors:\n");
            char* token = strtok(buf, "|");
            while (token != NULL) {
                printf("    - %s\n", token);
                failures++;
                token = strtok(NULL, "|");
            }
        } else {
            printf("  [OK] All Sprint 1 JS APIs exist and function properly.\n");
        }
        
        free(buf);
        JSStringRelease(res_str);
    }
    
    JSStringRelease(script_str);

    sys_jsc_teardown();
    
    printf("\n=== Results: %d failures ===\n", failures);
    return failures;
}
