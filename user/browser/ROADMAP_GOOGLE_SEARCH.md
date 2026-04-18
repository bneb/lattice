# Sprint Roadmap: Google.com → Search Query → Results

**Objective:** Load `https://google.com`, render the homepage (logo, search bar, buttons), type a query, submit, and render search results.

**Baseline Audit (April 17, 2026):**
| Subsystem | Status | Gap |
|---|---|---|
| Network (TLS/H2 fetch) | ✅ Working | — |
| HTML Lexer | ✅ Working | — |
| CSS Lexer/Cascade | ✅ Working | Missing properties (margin, border, inline-block) |
| Script Pump | ✅ Working (just shipped) | — |
| JSC Evaluate | ✅ Working | — |
| DOM API (createElement, appendChild, etc.) | ⚠️ Partial | Missing `location`, `getBoundingClientRect`, keyboard input |
| `element.style.*` setters | ⚠️ Partial | Missing `padding`, `margin`, `color`, `fontSize`, `textAlign`, `cursor`, `outline`, `border`, `fontFamily`, `textDecoration`, `verticalAlign`, `lineHeight`, `minWidth`, `maxWidth`, `boxSizing` |
| `document.write` | ✅ Working | — |
| Layout: Block | ✅ Working | Missing margin, border in box model |
| Layout: Flex (row+col) | ✅ Working | — |
| Layout: Inline | ⚠️ Basic | No line-breaking across mixed inline+block |
| Layout: Inline-Block | ❌ Missing | Needed for buttons side-by-side |
| Layout: Table | ❌ Missing | Needed for footer links |
| Layout: Form Intrinsics | ❌ Missing | INPUT=0px wide, BUTTON=0px wide |
| Layout: margin:auto | ❌ Missing | Google's centering mechanism |
| Image Decode/Render | ❌ Missing | Google logo |
| Keyboard Input | ❌ Missing | Can't type in search bar |
| Form Submission | ❌ Missing | Can't navigate to results |
| `window.location` | ❌ Missing | Google's boot script crashes |

---

## Sprint 1: JavaScript API Surface (Unblock Script Execution)

**Goal:** Google's 10 inline scripts execute to completion without throwing. This unblocks all dynamic DOM construction.

**Measurement:** `console.log` at end of last `<script>` block fires. DOM node count rises from ~106 (static) to ~400+ (dynamic UI fully constructed).

---

### 1.1 `window.location` Object

Google's very first script reads `location.href`. Without this, execution halts on line 1.

**File:** `jsc_bindings.m` → `bind_native_globals()`

```objc
// After the window.google stub block:

// ── window.location ──
// Store the current URL globally so location.href returns something real.
static char jsc_current_url[4096] = "https://www.google.com/";
static uint32_t jsc_current_url_len = 24;

void jsc_set_current_url(const char *url, uint32_t len) {
  uint32_t copy_len = len < 4095 ? len : 4095;
  memcpy(jsc_current_url, url, copy_len);
  jsc_current_url[copy_len] = '\0';
  jsc_current_url_len = copy_len;
}

static JSValueRef jsc_location_get_href(JSContextRef ctx, JSObjectRef object,
                                         JSStringRef pn, JSValueRef *ex) {
  return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(jsc_current_url));
}

static bool jsc_location_set_href(JSContextRef ctx, JSObjectRef object,
                                   JSStringRef pn, JSValueRef value,
                                   JSValueRef *ex) {
  // Setting location.href triggers navigation
  size_t len = JSStringGetMaximumUTF8CStringSize(JSValueToStringCopy(ctx, value, NULL));
  char *buf = malloc(len);
  JSStringGetUTF8CString(JSValueToStringCopy(ctx, value, NULL), buf, len);
  extern void sys_browser_navigate(uint64_t ptr, uint32_t len);
  sys_browser_navigate((uint64_t)buf, (uint32_t)strlen(buf));
  free(buf);
  return true;
}

// Parse URL components from jsc_current_url for hostname, protocol, etc.
static JSValueRef jsc_location_get_hostname(JSContextRef ctx, JSObjectRef obj,
                                             JSStringRef pn, JSValueRef *ex) {
  // Extract hostname from "https://www.google.com/..."
  const char *p = strstr(jsc_current_url, "://");
  if (!p) return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(""));
  p += 3;
  const char *end = strchr(p, '/');
  if (!end) end = p + strlen(p);
  char host[512] = {0};
  size_t hlen = end - p;
  if (hlen > 511) hlen = 511;
  memcpy(host, p, hlen);
  return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(host));
}

static JSValueRef jsc_location_get_protocol(JSContextRef ctx, JSObjectRef obj,
                                             JSStringRef pn, JSValueRef *ex) {
  if (strncmp(jsc_current_url, "https", 5) == 0)
    return JSValueMakeString(ctx, JSStringCreateWithUTF8CString("https:"));
  return JSValueMakeString(ctx, JSStringCreateWithUTF8CString("http:"));
}

static JSValueRef jsc_location_get_pathname(JSContextRef ctx, JSObjectRef obj,
                                             JSStringRef pn, JSValueRef *ex) {
  const char *p = strstr(jsc_current_url, "://");
  if (!p) return JSValueMakeString(ctx, JSStringCreateWithUTF8CString("/"));
  p += 3;
  const char *slash = strchr(p, '/');
  if (!slash) return JSValueMakeString(ctx, JSStringCreateWithUTF8CString("/"));
  return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(slash));
}

static JSValueRef jsc_location_get_search(JSContextRef ctx, JSObjectRef obj,
                                           JSStringRef pn, JSValueRef *ex) {
  const char *q = strchr(jsc_current_url, '?');
  if (!q) return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(""));
  return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(q));
}

static JSValueRef jsc_location_get_hash(JSContextRef ctx, JSObjectRef obj,
                                         JSStringRef pn, JSValueRef *ex) {
  const char *h = strchr(jsc_current_url, '#');
  if (!h) return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(""));
  return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(h));
}
```

**Registration in `bind_native_globals`:**
```objc
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
  JSClassRef locClass = JSClassCreate(&locDef);
  JSObjectRef locObj = JSObjectMake(ctx, locClass, NULL);

  JSStringRef locStr = JSStringCreateWithUTF8CString("location");
  JSObjectSetProperty(ctx, global, locStr, locObj, kJSPropertyAttributeNone, NULL);
  JSStringRelease(locStr);

  // Also bind on document
  JSStringRef docLocStr = JSStringCreateWithUTF8CString("location");
  JSObjectSetProperty(ctx, document, docLocStr, locObj, kJSPropertyAttributeNone, NULL);
  JSStringRelease(docLocStr);
}
```

---

### 1.2 `getBoundingClientRect()`

Google's layout JS calls this to measure elements. Returns the LAYOUT_X/Y/W/H from our solver.

**File:** `jsc_bindings.m` — add as a Node method

```objc
// ---- getBoundingClientRect() → {x, y, width, height, top, left, right, bottom} ----
extern int32_t dom_get_layout_x(uint32_t idx);
extern int32_t dom_get_layout_y(uint32_t idx);
extern int32_t dom_get_layout_w(uint32_t idx);
extern int32_t dom_get_layout_h(uint32_t idx);

JSValueRef jsc_node_getBoundingClientRect(JSContextRef ctx, JSObjectRef function,
    JSObjectRef thisObject, size_t argc, const JSValueRef argv[],
    JSValueRef *exception) {
  uint32_t n_idx = jsc_get_node_idx(thisObject);

  double x = (double)dom_get_layout_x(n_idx);
  double y = (double)dom_get_layout_y(n_idx);
  double w = (double)dom_get_layout_w(n_idx);
  double h = (double)dom_get_layout_h(n_idx);

  JSObjectRef rect = JSObjectMake(ctx, NULL, NULL);
  JSStringRef xStr = JSStringCreateWithUTF8CString("x");
  JSObjectSetProperty(ctx, rect, xStr, JSValueMakeNumber(ctx, x), 0, NULL);
  JSStringRelease(xStr);
  // ... repeat for y, width, height, top (=y), left (=x), right (=x+w), bottom (=y+h)

  return rect;
}
```

**Register in `jsc_classes.m` → `nodeFuncs[]`:**
```objc
{ "getBoundingClientRect", jsc_node_getBoundingClientRect, kJSPropertyAttributeNone },
```

---

### 1.3 `offsetWidth`, `offsetHeight`, `clientWidth`, `clientHeight`

**File:** `jsc_classes.m` → `nodeValues[]`

```objc
// Add as read-only static values:
{ "offsetWidth", get_node_offsetWidth, NULL, kJSPropertyAttributeReadOnly },
{ "offsetHeight", get_node_offsetHeight, NULL, kJSPropertyAttributeReadOnly },
{ "clientWidth", get_node_offsetWidth, NULL, kJSPropertyAttributeReadOnly },
{ "clientHeight", get_node_offsetHeight, NULL, kJSPropertyAttributeReadOnly },
```

**Implementation in `jsc_bindings.m`:**
```objc
JSValueRef get_node_offsetWidth(JSContextRef ctx, JSObjectRef object,
                                 JSStringRef pn, JSValueRef *ex) {
  uint32_t n_idx = jsc_get_node_idx(object);
  return JSValueMakeNumber(ctx, (double)dom_get_layout_w(n_idx));
}
JSValueRef get_node_offsetHeight(JSContextRef ctx, JSObjectRef object,
                                  JSStringRef pn, JSValueRef *ex) {
  uint32_t n_idx = jsc_get_node_idx(object);
  return JSValueMakeNumber(ctx, (double)dom_get_layout_h(n_idx));
}
```

---

### 1.4 `window.innerWidth` / `window.innerHeight`

```objc
// In bind_native_globals():
{
  JSStringRef iwStr = JSStringCreateWithUTF8CString("innerWidth");
  JSObjectSetProperty(ctx, global, iwStr, JSValueMakeNumber(ctx, 1920), 0, NULL);
  JSStringRelease(iwStr);
  JSStringRef ihStr = JSStringCreateWithUTF8CString("innerHeight");
  JSObjectSetProperty(ctx, global, ihStr, JSValueMakeNumber(ctx, 1080), 0, NULL);
  JSStringRelease(ihStr);
  JSStringRef owStr = JSStringCreateWithUTF8CString("outerWidth");
  JSObjectSetProperty(ctx, global, owStr, JSValueMakeNumber(ctx, 1920), 0, NULL);
  JSStringRelease(owStr);
  JSStringRef ohStr = JSStringCreateWithUTF8CString("outerHeight");
  JSObjectSetProperty(ctx, global, ohStr, JSValueMakeNumber(ctx, 1080), 0, NULL);
  JSStringRelease(ohStr);
}
```

---

### 1.5 `atob()` / `btoa()` (Base64)

Google's bootstrapper uses `atob` to decode inline data.

**File:** `jsc_bindings.m`

```objc
extern uint32_t ext_base64_decode(const uint8_t *src, uint32_t src_len,
                                   uint8_t *dst, uint32_t dst_max);

JSValueRef jsc_atob(JSContextRef ctx, JSObjectRef function,
    JSObjectRef thisObject, size_t argc, const JSValueRef argv[],
    JSValueRef *exception) {
  if (argc < 1) return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(""));

  JSStringRef jsStr = JSValueToStringCopy(ctx, argv[0], exception);
  size_t len = JSStringGetMaximumUTF8CStringSize(jsStr);
  char *src = malloc(len);
  JSStringGetUTF8CString(jsStr, src, len);
  JSStringRelease(jsStr);

  uint32_t src_len = (uint32_t)strlen(src);
  uint8_t *dst = malloc(src_len); // decoded is always <= encoded
  uint32_t decoded_len = ext_base64_decode((uint8_t *)src, src_len, dst, src_len);

  // Create string from raw bytes (binary-safe via Latin1 path)
  JSStringRef result = JSStringCreateWithUTF8CString((char *)dst);
  free(src);
  free(dst);
  return JSValueMakeString(ctx, result);
}

// btoa: encode to base64 (trivial lookup table)
static const char b64_table[] =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

JSValueRef jsc_btoa(JSContextRef ctx, JSObjectRef function,
    JSObjectRef thisObject, size_t argc, const JSValueRef argv[],
    JSValueRef *exception) {
  if (argc < 1) return JSValueMakeString(ctx, JSStringCreateWithUTF8CString(""));

  JSStringRef jsStr = JSValueToStringCopy(ctx, argv[0], exception);
  size_t maxLen = JSStringGetMaximumUTF8CStringSize(jsStr);
  char *src = malloc(maxLen);
  JSStringGetUTF8CString(jsStr, src, maxLen);
  JSStringRelease(jsStr);

  uint32_t slen = (uint32_t)strlen(src);
  uint32_t out_len = 4 * ((slen + 2) / 3);
  char *out = malloc(out_len + 1);
  uint32_t i = 0, j = 0;
  while (i < slen) {
    uint32_t a = (uint8_t)src[i++];
    uint32_t b = i < slen ? (uint8_t)src[i++] : 0;
    uint32_t c = i < slen ? (uint8_t)src[i++] : 0;
    uint32_t triple = (a << 16) | (b << 8) | c;
    out[j++] = b64_table[(triple >> 18) & 0x3F];
    out[j++] = b64_table[(triple >> 12) & 0x3F];
    out[j++] = (i > slen + 1) ? '=' : b64_table[(triple >> 6) & 0x3F];
    out[j++] = (i > slen) ? '=' : b64_table[triple & 0x3F];
  }
  out[j] = '\0';

  JSStringRef r = JSStringCreateWithUTF8CString(out);
  free(src); free(out);
  return JSValueMakeString(ctx, r);
}
```

**Register in `bind_native_globals`:**
```objc
JSStringRef atobStr = JSStringCreateWithUTF8CString("atob");
JSObjectSetProperty(ctx, global, atobStr,
    JSObjectMakeFunctionWithCallback(ctx, atobStr, jsc_atob), 0, NULL);
JSStringRelease(atobStr);

JSStringRef btoaStr = JSStringCreateWithUTF8CString("btoa");
JSObjectSetProperty(ctx, global, btoaStr,
    JSObjectMakeFunctionWithCallback(ctx, btoaStr, jsc_btoa), 0, NULL);
JSStringRelease(btoaStr);
```

---

### 1.6 `getElementsByTagName` / `getElementsByClassName` / `querySelectorAll`

**File:** `jsc_bindings.m`

`querySelectorAll` already exists. We need:

```objc
// getElementsByTagName — linear scan over DOM_NODE_TAG[]
extern uint32_t dom_get_node_count(void);
extern uint32_t dom_get_tag(uint32_t idx);
extern int32_t match_tag_str(const char *name, uint32_t len);
extern uint64_t node_id_from_idx(uint32_t idx);

JSValueRef jsc_document_getElementsByTagName(JSContextRef ctx,
    JSObjectRef function, JSObjectRef thisObject, size_t argc,
    const JSValueRef argv[], JSValueRef *exception) {
  if (argc < 1) return JSObjectMakeArray(ctx, 0, NULL, NULL);

  JSStringRef jsStr = JSValueToStringCopy(ctx, argv[0], exception);
  char name[64];
  JSStringGetUTF8CString(jsStr, name, 64);
  JSStringRelease(jsStr);

  uint32_t target_tag = match_tag_str(name, (uint32_t)strlen(name));
  uint32_t count = dom_get_node_count();
  JSValueRef results[256]; // bounded
  uint32_t found = 0;

  for (uint32_t i = 1; i < count && found < 256; i++) {
    if (dom_get_tag(i) == target_tag || strcmp(name, "*") == 0) {
      uint64_t nid = node_id_from_idx(i);
      results[found++] = create_js_node_wrapper(ctx, nid);
    }
  }
  return JSObjectMakeArray(ctx, found, results, NULL);
}
```

**Register on document object in `bind_native_globals`:**
```objc
JSStringRef getnStr = JSStringCreateWithUTF8CString("getElementsByTagName");
JSObjectSetProperty(ctx, document, getnStr,
    JSObjectMakeFunctionWithCallback(ctx, getnStr, jsc_document_getElementsByTagName), 0, NULL);
JSStringRelease(getnStr);
```

Same pattern for `getElementsByClassName` (scan `DOM_CLASS_PTR[]`/`DOM_CLASS_LEN[]`).

---

### 1.7 Remaining Style Setters

The `CSSStyleDeclaration` class currently handles: `backgroundColor`, `width`, `height`, `display`, `opacity`, `position`, `top`, `left`, `zIndex`, `overflow`, `transform`, `flexGrow`, `gridTemplateColumns`, `gridColumnStart`.

**Must add to `staticValues[]` in `init_style_class()`:**

| Property | Setter calls | DOM SoA Target |
|---|---|---|
| `padding` | Parse `"Npx"` → `dom_set_style_padding(idx, t, r, b, l)` | `STYLE_PADDING_*` |
| `paddingTop/Right/Bottom/Left` | Same, individual | `STYLE_PADDING_*` |
| `margin` | Parse `"Npx"` or `"auto"` → new `STYLE_MARGIN_*` arrays | New arrays |
| `marginTop/Right/Bottom/Left` | Same, individual | New arrays |
| `color` | Parse hex/rgb → `dom_set_style_color(idx, r, g, b)` | `STYLE_COLOR_*` |
| `fontSize` | Parse `"Npx"` → `STYLE_FONT_SIZE[idx]` | Existing |
| `fontFamily` | No-op (use system font) | — |
| `textAlign` | `"center"` → 2, `"right"` → 1, etc. | `STYLE_TEXT_ALIGN` |
| `textDecoration` | No-op initially | — |
| `lineHeight` | Parse px → `dom_set_style_line_height` | Existing |
| `cursor` | No-op initially | — |
| `outline` | No-op initially | — |
| `border` | Parse → new `STYLE_BORDER_W` array | New arrays |
| `borderRadius` | Already exists | — |
| `boxSizing` | Store u8: 0=content-box, 1=border-box | New `STYLE_BOX_SIZING` |
| `maxWidth` / `minWidth` | Store i32 → new arrays | New arrays |
| `maxHeight` / `minHeight` | Store i32 → new arrays | New arrays |
| `verticalAlign` | Store u8: 0=baseline, 1=middle, 2=top, 3=bottom | New array |
| `visibility` | Store u8: 0=visible, 1=hidden → paint skip | New array |
| `whiteSpace` | Store u8: 0=normal, 1=nowrap | New array |
| `flexDirection` | `"row"` / `"column"` | Existing |
| `justifyContent` | `"center"` / `"space-between"` etc. | Existing |
| `alignItems` | `"center"` / `"stretch"` / `"flex-end"` | Existing |
| `flexWrap` | `"wrap"` / `"nowrap"` | New `STYLE_FLEX_WRAP` |

---

### 1.8 `hasAttribute()` / `cloneNode()` / `contains()`

```objc
// hasAttribute — check if attribute key exists in DOM_ATTR_* arrays
JSValueRef jsc_node_hasAttribute(JSContextRef ctx, JSObjectRef function,
    JSObjectRef thisObject, size_t argc, const JSValueRef argv[],
    JSValueRef *exception) {
  if (argc < 1) return JSValueMakeBoolean(ctx, false);
  uint32_t n_idx = jsc_get_node_idx(thisObject);
  // Linear scan attr arrays; or check specific known attributes (id, class, src)
  // For Google, checking setAttribute/getAttribute consistency is enough
  JSValueRef result = jsc_node_getAttribute(ctx, function, thisObject, argc, argv, exception);
  return JSValueMakeBoolean(ctx, !JSValueIsNull(ctx, result));
}

// cloneNode(deep) — duplicate DOM subtree
JSValueRef jsc_node_cloneNode(JSContextRef ctx, JSObjectRef function,
    JSObjectRef thisObject, size_t argc, const JSValueRef argv[],
    JSValueRef *exception) {
  uint32_t n_idx = jsc_get_node_idx(thisObject);
  extern uint64_t ext_dom_clone_node(uint32_t idx, uint8_t deep);
  uint8_t deep = (argc > 0 && JSValueToBoolean(ctx, argv[0])) ? 1 : 0;
  uint64_t new_id = ext_dom_clone_node(n_idx, deep);
  extern JSObjectRef create_js_node_wrapper(JSContextRef ctx, uint64_t node_id);
  return create_js_node_wrapper(ctx, new_id);
}
```

---

### Sprint 1 E2E Test

**File:** `tests/test_google_js_apis.salt`

Validates that all 10 Google inline scripts execute to completion. Success = DOM node count > 300 after script pump, zero JSC exceptions logged.

---

## Sprint 2: DOM Data Model — Margin, Border, Box-Sizing

**Goal:** Add the SoA arrays needed for margin, border, and box-sizing so the layout solver and CSS cascade can reference them.

---

### 2.1 New SoA Arrays in `dom.salt`

```salt
// Margin Box (i32, px, -1 = unset, -2 = auto)
pub global STYLE_MARGIN_TOP:    [i32; 65536] = [-1; 65536];
pub global STYLE_MARGIN_RIGHT:  [i32; 65536] = [-1; 65536];
pub global STYLE_MARGIN_BOTTOM: [i32; 65536] = [-1; 65536];
pub global STYLE_MARGIN_LEFT:   [i32; 65536] = [-1; 65536];

// Border Width (i32, px, 0 = no border)
pub global STYLE_BORDER_WIDTH_TOP:    [i32; 65536] = [0; 65536];
pub global STYLE_BORDER_WIDTH_RIGHT:  [i32; 65536] = [0; 65536];
pub global STYLE_BORDER_WIDTH_BOTTOM: [i32; 65536] = [0; 65536];
pub global STYLE_BORDER_WIDTH_LEFT:   [i32; 65536] = [0; 65536];

// Border Color (packed RGB as before: r, g, b per side — simplified to uniform)
pub global STYLE_BORDER_COLOR_R: [u8; 65536] = [0; 65536];
pub global STYLE_BORDER_COLOR_G: [u8; 65536] = [0; 65536];
pub global STYLE_BORDER_COLOR_B: [u8; 65536] = [0; 65536];

// Box-Sizing: 0=content-box, 1=border-box
pub global STYLE_BOX_SIZING: [u8; 65536] = [0; 65536];

// Min/Max Constraints (i32, px, -1 = unset)
pub global STYLE_MIN_WIDTH:  [i32; 65536] = [-1; 65536];
pub global STYLE_MAX_WIDTH:  [i32; 65536] = [-1; 65536];
pub global STYLE_MIN_HEIGHT: [i32; 65536] = [-1; 65536];
pub global STYLE_MAX_HEIGHT: [i32; 65536] = [-1; 65536];

// Visibility: 0=visible, 1=hidden
pub global STYLE_VISIBILITY: [u8; 65536] = [0; 65536];
```

Add `dom_ptr_*()` export functions and pointer injection plumbing for each new array (same pattern as existing ~50 arrays).

---

### 2.2 Pointer Injection for New Arrays

**File:** `layout.salt` — add to `layout_inject_dom_pointers()`:

```salt
global P_STYLE_MARGIN_TOP: u64 = 0;
global P_STYLE_MARGIN_RIGHT: u64 = 0;
global P_STYLE_MARGIN_BOTTOM: u64 = 0;
global P_STYLE_MARGIN_LEFT: u64 = 0;
global P_STYLE_BORDER_W_TOP: u64 = 0;
global P_STYLE_BORDER_W_RIGHT: u64 = 0;
global P_STYLE_BORDER_W_BOTTOM: u64 = 0;
global P_STYLE_BORDER_W_LEFT: u64 = 0;
global P_STYLE_BOX_SIZING: u64 = 0;
global P_STYLE_MIN_WIDTH: u64 = 0;
global P_STYLE_MAX_WIDTH: u64 = 0;
global P_STYLE_MIN_HEIGHT: u64 = 0;
global P_STYLE_MAX_HEIGHT: u64 = 0;
```

---

### 2.3 CSS Lexer: Parse Margin, Border, Box-Sizing

**File:** `css_lexer.salt` — add property matchers:

```salt
// Match "margin" (len=6) → parse shorthand: "10px", "10px 20px", "auto"
// Match "border" (len=6) → parse shorthand: "1px solid #ccc"
// Match "box-sizing" (len=10) → "border-box" | "content-box"
// Match "min-width" (len=9) → parse px value
// Match "max-width" (len=9) → parse px value
```

---

## Sprint 3: Layout Solver — Margin, Inline-Block, Form Intrinsics

**Goal:** The layout solver produces correct geometry for Google's centered search bar UI.

---

### 3.1 Margin in Block Layout

**File:** `layout.salt` → `compute_layout_core()`, block mode (display_mode == 1)

**The key change:** Before laying out each block child, read its margin values and apply:

```salt
// In BLOCK child loop, after child_pos check:
let margin_top = *((P_STYLE_MARGIN_TOP + (child_idx as u64) * 4) as &i32);
let margin_bottom = *((P_STYLE_MARGIN_BOTTOM + (child_idx as u64) * 4) as &i32);
let margin_left = *((P_STYLE_MARGIN_LEFT + (child_idx as u64) * 4) as &i32);
let margin_right = *((P_STYLE_MARGIN_RIGHT + (child_idx as u64) * 4) as &i32);

// Apply margin-top spacing
if margin_top > 0 {
    child_offset_y = child_offset_y + margin_top;
}

// margin:auto centering (horizontal)
let mut effective_left = if margin_left == -2 { 0 } else { margin_left.max(0) };
let mut effective_right = if margin_right == -2 { 0 } else { margin_right.max(0) };

let child_explicit_w = *((P_STYLE_W + (child_idx as u64) * 4) as &i32);
if margin_left == -2 && margin_right == -2 && child_explicit_w > 0 {
    // Both margins auto → center
    let free = content_w - child_explicit_w;
    if free > 0 {
        effective_left = free / 2;
    }
}

let ch_height = compute_layout(child_idx,
    child_offset_x + effective_left,
    child_offset_y,
    content_w - effective_left - effective_right);

child_offset_y = child_offset_y + ch_height;

// Apply margin-bottom
if margin_bottom > 0 {
    child_offset_y = child_offset_y + margin_bottom;
}
```

**IMPORTANT:** Use `-2` as the sentinel for `auto` (since `-1` means "unset"). The CSS setter maps `"auto"` → `-2`.

---

### 3.2 `display: inline-block` (New Display Mode = 5)

**File:** `constants.salt`

```salt
pub const DISPLAY_GRID: u8 = 4;            // existing
pub const DISPLAY_INLINE_BLOCK: u8 = 5;    // NEW
```

**File:** `layout.salt` — add handling in block parent's child loop:

```salt
if child_display == 5 { // INLINE-BLOCK
    // Behaves like inline horizontally (sits on the line)
    // but renders like a block internally (has width/height)
    if !in_inline_run {
        in_inline_run = true;
        line_x = child_offset_x;
        line_y = child_offset_y;
        line_height = 0;
    }

    // Compute the child with its explicit or intrinsic width
    let child_explicit_w = *((P_STYLE_W + (child_idx as u64) * 4) as &i32);
    let child_max_w = if child_explicit_w != -1 { child_explicit_w } else { remaining_w };
    let ch_height = compute_layout(child_idx, line_x, line_y, child_max_w);
    let ch_width = *((P_LAYOUT_W + (child_idx as u64) * 4) as &i32);

    // Check line wrap
    if line_x + ch_width > child_offset_x + content_w && line_x > child_offset_x {
        child_offset_y = line_y + line_height;
        line_x = child_offset_x;
        line_y = child_offset_y;
        line_height = 0;
        // Re-layout at new line position
        compute_layout(child_idx, line_x, line_y, child_max_w);
    }

    line_x = line_x + ch_width;
    if ch_height > line_height { line_height = ch_height; }
}
```

**File:** `jsc_bindings.m` — `jsc_style_set_display`:

```objc
// Add to existing display setter:
else if (strcmp(str, "inline-block") == 0)
    dom_set_style_display(n_idx, 5);
```

**File:** `css_lexer.salt` — same mapping in the CSS value parser.

---

### 3.3 Form Control Intrinsic Sizing

**File:** `layout.salt` → `compute_layout_core()`, add before the display_mode switch:

```salt
// ── Form Control Intrinsic Dimensions ──
// W3C: <input> has a default width of ~173px and height based on font-size + padding.
// <button> has intrinsic width from text content + padding.
// <select> is similar to input.
// <textarea> has intrinsic 300x150.

let current_tag = *((P_DOM_NODE_TAG + (node_idx as u64) * 4) as &u32);

if current_tag == 18 { // TAG_INPUT
    if *((P_STYLE_W + (node_idx as u64) * 4) as &i32) == -1 {
        *((P_STYLE_W + (node_idx as u64) * 4) as &mut i32) = 173; // Chrome default
    }
    if *((P_STYLE_H + (node_idx as u64) * 4) as &i32) == -1 {
        let fs = *((P_STYLE_FONT_SIZE + (node_idx as u64) * 4) as &f32);
        let intrinsic_h = (fs + 8.0) as i32; // font-size + internal padding
        if intrinsic_h > 0 {
            *((P_STYLE_H + (node_idx as u64) * 4) as &mut i32) = intrinsic_h;
        }
    }
}

if current_tag == 20 { // TAG_BUTTON
    if *((P_STYLE_W + (node_idx as u64) * 4) as &i32) == -1 {
        // Measure text content width + 16px padding
        let text_w = compute_inline_text_width(node_idx, max_width as f32);
        *((P_STYLE_W + (node_idx as u64) * 4) as &mut i32) = (text_w as i32) + 16;
    }
    if *((P_STYLE_H + (node_idx as u64) * 4) as &i32) == -1 {
        let fs = *((P_STYLE_FONT_SIZE + (node_idx as u64) * 4) as &f32);
        *((P_STYLE_H + (node_idx as u64) * 4) as &mut i32) = (fs + 10.0) as i32;
    }
}
```

---

### 3.4 Min/Max Width Constraints

**File:** `layout.salt` — add after `target_w` is resolved (line ~510):

```salt
// Apply min-width / max-width constraints
let min_w = *((P_STYLE_MIN_WIDTH + (node_idx as u64) * 4) as &i32);
let max_w = *((P_STYLE_MAX_WIDTH + (node_idx as u64) * 4) as &i32);
if min_w != -1 && target_w < min_w as f32 { target_w = min_w as f32; }
if max_w != -1 && target_w > max_w as f32 { target_w = max_w as f32; }
```

Same for height after `used_h` is resolved:

```salt
let min_h = *((P_STYLE_MIN_HEIGHT + (node_idx as u64) * 4) as &i32);
let max_h = *((P_STYLE_MAX_HEIGHT + (node_idx as u64) * 4) as &i32);
if min_h != -1 && computed_h < min_h { computed_h = min_h; }
if max_h != -1 && computed_h > max_h { computed_h = max_h; }
```

---

### 3.5 Border in Box Model

**File:** `layout.salt` — modify content_w calculation:

```salt
let border_top = *((P_STYLE_BORDER_W_TOP + (node_idx as u64) * 4) as &i32);
let border_right = *((P_STYLE_BORDER_W_RIGHT + (node_idx as u64) * 4) as &i32);
let border_bottom = *((P_STYLE_BORDER_W_BOTTOM + (node_idx as u64) * 4) as &i32);
let border_left = *((P_STYLE_BORDER_W_LEFT + (node_idx as u64) * 4) as &i32);

let content_w = used_w - pad_left - pad_right - border_left - border_right;
```

And for border-box sizing:

```salt
let box_sizing = *((P_STYLE_BOX_SIZING + (node_idx as u64)) as &u8);
if box_sizing == 1 { // border-box
    // Width already includes padding + border, don't subtract again for layout
    // Adjust content_w to shrink only by padding+border
    let content_w = used_w - pad_left - pad_right - border_left - border_right;
} else { // content-box (default)
    // Width is content only, total box = width + padding + border
    let content_w = used_w;
    used_w = used_w + pad_left + pad_right + border_left + border_right;
}
```

---

### Sprint 3 E2E Test

Verify Google's search bar and buttons receive correct layout bounds:
- Search `<input>`: x centered (≈ `(1920 - 560) / 2`), w ≈ 560, h ≈ 34
- "Google Search" `<button>`: w > 0, positioned below input
- All elements centered via margin:auto

---

## Sprint 4: Image Loading Pipeline

**Goal:** The Google logo renders as a visible image.

---

### 4.1 `<IMG>` Resource Fetch

When the HTML lexer encounters `<img src="...">`, it already stores the URL via `dom_set_script_src` (reuse the src infrastructure). We need to:

1. **Emit a `CMD_FETCH_REQUEST`** with a new bit flag (bit 59) for image fetches
2. **In the main process**, download the image data and deliver it to the renderer via IPC bulk ingress
3. **In the renderer**, decode the image and store pixel data in a texture slot

**File:** `lexer.salt` — in the `src` attribute handler for TAG_IMG:

```salt
if current_tag == 8 { // TAG_IMG
    let img_fetch_id = queue_image_fetch(
        html_ptr + attr_val_start as u64, attr_val_len);
    if img_fetch_id != 0 {
        let mut img_bit: u64 = 1;
        img_bit = img_bit << 59;
        let multiplexed_id = img_bit | img_fetch_id;
        sys_ipc_send_r2m_command_with_payload(
            12 /* CMD_FETCH_REQUEST */,
            multiplexed_id,
            html_ptr + attr_val_start as u64,
            attr_val_len);
    }
}
```

---

### 4.2 Image Decoding (Core Graphics / ImageIO)

**File:** `image_decode.m` (NEW)

```objc
#import <ImageIO/ImageIO.h>
#import <CoreGraphics/CoreGraphics.h>

// Decode PNG/JPEG/WebP/GIF to RGBA bitmap
// Returns a pointer to the pixel buffer and fills out_w/out_h.
uint8_t *ext_decode_image(const uint8_t *data, uint32_t data_len,
                           uint32_t *out_w, uint32_t *out_h) {
  CFDataRef cfData = CFDataCreateWithBytesNoCopy(
      NULL, data, data_len, kCFAllocatorNull);
  CGImageSourceRef source = CGImageSourceCreateWithData(cfData, NULL);
  if (!source) { CFRelease(cfData); return NULL; }

  CGImageRef cgImage = CGImageSourceCreateImageAtIndex(source, 0, NULL);
  if (!cgImage) { CFRelease(source); CFRelease(cfData); return NULL; }

  *out_w = (uint32_t)CGImageGetWidth(cgImage);
  *out_h = (uint32_t)CGImageGetHeight(cgImage);

  uint32_t bpp = 4;
  uint8_t *pixels = calloc(*out_w * *out_h, bpp);
  CGColorSpaceRef cs = CGColorSpaceCreateDeviceRGB();
  CGContextRef ctx = CGBitmapContextCreate(
      pixels, *out_w, *out_h, 8, *out_w * bpp, cs,
      kCGImageAlphaPremultipliedLast);
  CGContextDrawImage(ctx, CGRectMake(0, 0, *out_w, *out_h), cgImage);
  CGContextRelease(ctx);
  CGColorSpaceRelease(cs);
  CGImageRelease(cgImage);
  CFRelease(source);
  CFRelease(cfData);

  return pixels;
}
```

---

### 4.3 Image Texture Slots in DOM

**File:** `dom.salt`

```salt
// Image pixel data: up to 64 loaded images
pub global IMG_PIXEL_DATA: [u64; 64] = [0; 64];   // pointer to RGBA buffer
pub global IMG_WIDTH:      [u32; 64] = [0; 64];
pub global IMG_HEIGHT:     [u32; 64] = [0; 64];
pub global DOM_IMG_SLOT:   [i32; 65536] = [-1; 65536]; // node_idx → img slot
```

---

### 4.4 Paint: Emit Image Quads

**File:** `compositor.salt` — in the paint rect emission loop, after background rect:

```salt
// If node has an image slot, emit a textured quad
let img_slot = dom.DOM_IMG_SLOT[node_idx as usize];
if img_slot >= 0 {
    // Emit PAINT_CMD_IMAGE with the slot index, position, and dimensions
    emit_image_rect(layout_x, layout_y, layout_w, layout_h, img_slot as u32);
}
```

---

### 4.5 Metal: Textured Quad Rendering

**File:** `metal.m` — modify the shader to support textured quads alongside solid-color quads.

Add a `texture` uniform and sample from it when `rect.type == 1` (image).

---

## Sprint 5: Keyboard Input & Text Editing

**Goal:** User can click on the search `<input>` and type characters.

---

### 5.1 Focus Management

**File:** `dom.salt`

```salt
pub global FOCUSED_NODE_IDX: u32 = 0; // 0 = nothing focused

@no_mangle
pub fn dom_set_focus(node_idx: u32) {
    FOCUSED_NODE_IDX = node_idx;
    DIRTY_PAINT[node_idx as usize] = 1;
}

@no_mangle
pub fn dom_get_focused_node() -> u32 {
    return FOCUSED_NODE_IDX;
}
```

---

### 5.2 Click → Focus for `<input>` Elements

**File:** `main.salt` — in the mouse click handler:

```salt
// After hit-test resolves target node:
let target_tag = dom.DOM_NODE_TAG[target_idx as usize];
if target_tag == 18 { // TAG_INPUT
    dom.dom_set_focus(target_idx);
}
```

---

### 5.3 Keyboard Event Pipeline

**File:** `mac_app.m` — in the `keyDown:` handler, push to IPC:

```objc
// CMD_KEYDOWN = 5 (new IPC command)
// arg1 = packed(keyCode, charCode)
// payload = the UTF-8 character bytes
- (void)keyDown:(NSEvent *)event {
  NSString *chars = [event characters];
  if (!chars.length) return;

  const char *utf8 = [chars UTF8String];
  uint32_t len = (uint32_t)strlen(utf8);

  // Push key event into M2R ring buffer
  sys_ipc_push_command_with_payload(5 /* CMD_KEYDOWN */,
      (uint64_t)[event keyCode], (uint64_t)utf8, len);
}
```

**File:** `main.salt` — in `app_run_loop`, add key event handling:

```salt
// Read key events from IPC
if cmd_type == 5 { // CMD_KEYDOWN
    let key_code = read_ipc_command_arg1(cmd_ptr);
    let char_ptr = read_ipc_command_payload_ptr(cmd_ptr);
    let char_len = read_ipc_command_payload_len(cmd_ptr);

    let focused = dom.FOCUSED_NODE_IDX;
    if focused != 0 && dom.DOM_NODE_TAG[focused as usize] == 18 {
        // Append character to input value
        input_append_char(focused, char_ptr, char_len);
        // Re-measure text width for cursor positioning
        dom.DIRTY_LAYOUT[focused as usize] = 1;
        dom.DIRTY_PAINT[focused as usize] = 1;
    }
}
```

---

### 5.4 Input Value Storage

**File:** `dom.salt`

```salt
// Input element text content (separate from display text nodes)
// Each input can hold up to 1024 bytes of UTF-8 text
pub global INPUT_VALUE_BUF: [u8; 65536] = [0; 65536]; // 64 inputs × 1024 bytes
pub global INPUT_VALUE_LEN: [u32; 64] = [0; 64];
pub global DOM_INPUT_SLOT:  [i32; 65536] = [-1; 65536]; // node_idx → input slot

@no_mangle
pub fn input_append_char(node_idx: u32, char_ptr: u64, char_len: u32) {
    let slot = DOM_INPUT_SLOT[node_idx as usize];
    if slot < 0 { return; }
    let u_slot = slot as u32;
    let offset = u_slot * 1024;
    let cur_len = INPUT_VALUE_LEN[u_slot as usize];
    if cur_len + char_len > 1024 { return; }
    let mut i: u32 = 0;
    while i < char_len {
        INPUT_VALUE_BUF[(offset + cur_len + i) as usize] =
            *((char_ptr + i as u64) as &u8);
        i = i + 1;
    }
    INPUT_VALUE_LEN[u_slot as usize] = cur_len + char_len;
}
```

---

### 5.5 Paint: Render Input Text + Cursor

**File:** `paint.salt` — when painting TAG_INPUT nodes:

```salt
// Emit text glyphs for input value
let slot = dom.DOM_INPUT_SLOT[node_idx as usize];
if slot >= 0 {
    let u_slot = slot as u32;
    let offset = u_slot * 1024;
    let value_len = dom.INPUT_VALUE_LEN[u_slot as usize];
    if value_len > 0 {
        let text_ptr = (&dom.INPUT_VALUE_BUF) as u64 + offset as u64;
        emit_text_run(layout_x + 4, layout_y + 2, text_ptr, value_len, font_size);
    }
    // Blinking cursor at end of text
    if dom.FOCUSED_NODE_IDX == node_idx {
        let cursor_x = layout_x + 4 + measure_text_width(text_ptr, value_len, font_size);
        emit_rect(cursor_x, layout_y + 2, 1, font_size as i32, 0x000000FF);
    }
}
```

---

### 5.6 Dispatch `input` and `keydown` Events to JS

```objc
// After appending the character in Salt:
js_bridge_dispatch_event(focused_node_id, 3 /* EVENT_INPUT */);

// Also dispatch keydown event:
js_bridge_dispatch_event(focused_node_id, 4 /* EVENT_KEYDOWN */);
```

---

## Sprint 6: Form Submission & Navigation

**Goal:** Pressing `Enter` in the search bar submits the form and navigates to search results.

---

### 6.1 `<form>` Support in Lexer

**File:** `lexer.salt` — add `form` to `match_tag`:

```salt
// len == 4: add 'form' check
if b0 == 102 && b1 == 111 && b2 == 114 && b3 == 109 { return 21; } // TAG_FORM=21
```

**File:** `constants.salt`:
```salt
pub const TAG_INPUT:  u32 = 18;
pub const TAG_TEXTAREA: u32 = 19;
pub const TAG_BUTTON: u32 = 20;
pub const TAG_FORM:   u32 = 21;  // NEW
```

---

### 6.2 Form Data Collection

**File:** `dom.salt`:

```salt
// Walks the DOM upward from input to find enclosing <form>
@no_mangle
pub fn find_form_ancestor(node_idx: u32) -> u32 {
    let mut parent = STYLE_PARENT[node_idx as usize];
    while parent != 0 && parent != 999999 {
        if DOM_NODE_TAG[parent as usize] == 21 { // TAG_FORM
            return parent;
        }
        parent = STYLE_PARENT[parent as usize];
    }
    return 0;
}
```

---

### 6.3 Submit Handler (Enter Key)

**File:** `main.salt` — in key event handler:

```salt
if key_code == 36 { // Enter key (macOS keyCode)
    let focused = dom.FOCUSED_NODE_IDX;
    if focused != 0 && dom.DOM_NODE_TAG[focused as usize] == 18 {
        let form_idx = dom.find_form_ancestor(focused);
        if form_idx != 0 {
            // Read the "action" attribute from the form
            let action_ptr = dom.dom_get_attr_val_ptr(form_idx, "action");
            let action_len = dom.dom_get_attr_val_len(form_idx, "action");

            // Read input name + value
            let name_ptr = dom.dom_get_attr_val_ptr(focused, "name");
            let name_len = dom.dom_get_attr_val_len(focused, "name");
            let slot = dom.DOM_INPUT_SLOT[focused as usize];
            let value_len = dom.INPUT_VALUE_LEN[slot as u32 as usize];
            let value_offset = (slot as u32) * 1024;

            // Construct URL: action + "?" + name + "=" + urlencoded(value)
            construct_search_url_and_navigate(
                action_ptr, action_len,
                name_ptr, name_len,
                (&dom.INPUT_VALUE_BUF) as u64 + value_offset as u64,
                value_len);
        }
    }
}
```

---

### 6.4 URL Construction + Navigate

**File:** `dom.salt` or `main.salt`:

```salt
@no_mangle
pub fn construct_search_url_and_navigate(
    action_ptr: u64, action_len: u32,
    name_ptr: u64, name_len: u32,
    value_ptr: u64, value_len: u32
) {
    // Build: "https://www.google.com/search?q=hello+world"
    let mut buf: [u8; 4096] = [0; 4096];
    let mut pos: u32 = 0;

    // Copy action URL
    let mut i: u32 = 0;
    while i < action_len {
        buf[pos as usize] = *((action_ptr + i as u64) as &u8);
        pos = pos + 1;
        i = i + 1;
    }

    // Append "?"
    buf[pos as usize] = 63; // '?'
    pos = pos + 1;

    // Append name=
    i = 0;
    while i < name_len {
        buf[pos as usize] = *((name_ptr + i as u64) as &u8);
        pos = pos + 1;
        i = i + 1;
    }
    buf[pos as usize] = 61; // '='
    pos = pos + 1;

    // Append URL-encoded value (spaces → +, special chars → %XX)
    i = 0;
    while i < value_len {
        let c = *((value_ptr + i as u64) as &u8);
        if c == 32 { // space → '+'
            buf[pos as usize] = 43;
            pos = pos + 1;
        } else if (c >= 65 && c <= 90) || (c >= 97 && c <= 122) || (c >= 48 && c <= 57) {
            buf[pos as usize] = c;
            pos = pos + 1;
        } else {
            // %XX encoding
            buf[pos as usize] = 37; // '%'
            buf[(pos+1) as usize] = hex_nibble((c >> 4) & 0x0F);
            buf[(pos+2) as usize] = hex_nibble(c & 0x0F);
            pos = pos + 3;
        }
        i = i + 1;
    }

    // Trigger navigation
    sys_browser_navigate((&buf) as u64, pos);
}
```

---

## Sprint 7: Table Layout (Footer Links)

**Goal:** Google's footer link grid renders correctly.

---

### 7.1 Basic Table Layout Mode

**File:** `layout.salt` — add `display_mode == 6` (TABLE):

```salt
pub const DISPLAY_TABLE: u8 = 6;

// TABLE layout: columns auto-sized by content width
// Simple algorithm:
// 1. Collect all <tr> children
// 2. For each <td>, measure text width
// 3. Max column widths across all rows
// 4. Position each cell at cumulative x offsets
```

The table layout follows the same recursive pattern as grid but with auto-sized columns based on content measurement.

Google's footer table is simple (2 rows, 3 links each), so a basic implementation suffices.

---

## Sprint 8: Search Results Page

**Goal:** After navigating to `/search?q=...`, the results page renders with clickable links.

---

### 8.1 Link Click → Navigation

**File:** `main.salt` — in click handler, after hit-test:

```salt
let target_tag = dom.DOM_NODE_TAG[target_idx as usize];
if target_tag == 7 { // TAG_A
    // Read href attribute
    let href_ptr = dom.dom_get_attr_val_ptr(target_idx, "href");
    let href_len = dom.dom_get_attr_val_len(target_idx, "href");
    if href_len > 0 {
        sys_browser_navigate(href_ptr, href_len);
    }
}
```

---

### 8.2 Scroll

Google results pages are longer than the viewport. Basic scroll support:

**File:** `main.salt` — handle scroll wheel events:

```salt
// CMD_SCROLL = 6 (new IPC command)
if cmd_type == 6 {
    let delta_y = read_ipc_command_arg1(cmd_ptr) as i32;
    let body_idx = dom.dom_find_body_node();
    dom.LAYOUT_SCROLL_Y[body_idx as usize] =
        dom.LAYOUT_SCROLL_Y[body_idx as usize] + (delta_y as f32);
    dom.DIRTY_PAINT[body_idx as usize] = 1;
}
```

**File:** `mac_app.m` — in `scrollWheel:`:

```objc
- (void)scrollWheel:(NSEvent *)event {
  int32_t dy = (int32_t)([event scrollingDeltaY] * -3.0);
  sys_ipc_push_command(6 /* CMD_SCROLL */, (uint64_t)(uint32_t)dy, 0);
}
```

---

## Sprint 9: Polish & Performance

### 9.1 Paint: Border Rendering

Emit 4 thin rects for each border edge (top/right/bottom/left) using the border-width and border-color SoA arrays.

### 9.2 Paint: Input Focus Ring

When a focused input is painted, emit a 2px blue outline rect around it.

### 9.3 Paint: Button Hover State

Track the currently hovered node (from mouse move events). When painting a button under hover, darken the background by 10%.

### 9.4 Performance: Layout Caching

The dirty-flag system already exists. Ensure that after JS execution + script pump, only newly created/modified nodes have `DIRTY_LAYOUT = 1`. The rest should bypass via the existing fast path (line 409 in `layout.salt`).

### 9.5 Performance: Incremental Paint

Only repaint nodes with `DIRTY_PAINT = 1`. The compositor already has this infrastructure.

---

## Dependency Graph

```
Sprint 1 (JS APIs)
    └── Sprint 2 (DOM Data: margin/border arrays)
          └── Sprint 3 (Layout: margin, inline-block, form intrinsics)
                ├── Sprint 4 (Image loading) [parallel]
                ├── Sprint 5 (Keyboard input) [parallel]
                │     └── Sprint 6 (Form submission)
                │           └── Sprint 8 (Results page)
                └── Sprint 7 (Table layout) [parallel]
                      └── Sprint 9 (Polish)
```

**Critical Path:** 1 → 2 → 3 → 5 → 6 → 8

Sprints 4, 7, and 9 are parallelizable and non-blocking for the core search flow.

---

## Estimated Scope by Sprint

| Sprint | Files Changed | Lines Added (est.) | Risk |
|---|---|---|---|
| 1. JS APIs | jsc_bindings.m, jsc_classes.m | ~800 | **Medium** — each API is small but there are ~15 |
| 2. DOM Data | dom.salt, layout.salt | ~200 | **Low** — mechanical SoA additions |
| 3. Layout Solver | layout.salt, constants.salt, css_lexer.salt | ~400 | **High** — margin:auto + inline-block are subtle |
| 4. Image Loading | image_decode.m (new), lexer.salt, compositor.salt, metal.m | ~500 | **Medium** — well-scoped |
| 5. Keyboard Input | mac_app.m, main.salt, dom.salt, paint.salt | ~300 | **Medium** — new subsystem |
| 6. Form Submission | main.salt, dom.salt | ~150 | **Low** — builds on Sprint 5 |
| 7. Table Layout | layout.salt, constants.salt, lexer.salt | ~200 | **Low** — bounded scope |
| 8. Results Page | — (reuses everything) | ~50 | **Low** — link navigation |
| 9. Polish | paint.salt, compositor.salt | ~200 | **Low** — incremental |

**Total:** ~2,800 lines across 9 sprints.

---

## Verification Matrix

Each sprint has an E2E test in `tests/`:

| Sprint | Test File | Pass Criteria |
|---|---|---|
| 1 | `test_google_js_apis.salt` | Google's 10 scripts complete, DOM nodes > 300 |
| 2 | (extend render_pipeline bridge) | Margin arrays initialized, box model correct |
| 3 | `test_layout_margin_auto.salt` | Centered div at x≈480 in 1920 viewport |
| 4 | `test_image_decode.salt` | PNG decoded, pixel data at slot 0 |
| 5 | `test_keyboard_input.salt` | Char appended, value.length increases |
| 6 | `test_form_submit.salt` | Enter key → navigation URL contains "?q=" |
| 7 | `test_table_layout.salt` | 3 cells in row, x offsets monotonically increasing |
| 8 | `test_google_search_e2e.salt` | Full flow: load → type → submit → results DOM exists |
| 9 | (manual visual) | Screenshot comparison |
