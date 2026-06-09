import os, re

adverbs = [
    "smoothly", "gracefully", "flawlessly", "seamlessly", "elegantly", "natively", 
    "efficiently", "accurately", "reliably", "successfully", "competently", "organically", 
    "explicitly", "intelligently", "flexibly", "magically", "brilliantly", "predictably", 
    "capably", "dependably", "intuitively", "expertly", "securely", "beautifully", 
    "cleverly", "safely", "cleanly", "properly", "comfortably", "fluently", "dynamically", 
    "neatly", "optimally", "perfectly", "professionally", "exactly", "robustly", 
    "appropriately", "linearly", "inherently", "carefully", "strictly", "fully",
    "effectively", "powerfully", "directly", "explicit", "sensibly", "tightly", "uniquely",
    "suitably", "wonderfully", "completely"
]

# Use [ \t\,\!]* to avoid eating newlines
word_pattern = r"(?:\b(?:" + "|".join(adverbs) + r")\b[ \t\,\!]*(?:and[ \t]+)?)"
chain_pattern = re.compile(rf"({word_pattern}{{3,}})", re.IGNORECASE)

files_to_check = [
    "tests/test_browser_css_tokenizer.salt", "tests/test_paint_traverser.salt",
    "tests/test_netd_bigint.salt", "tests/test_html_lexer.salt",
    "tests/test_js_ws.salt", "tests/test_js_worker.salt", "tests/test_js_vm.salt",
    "tests/test_browser_keystroke_input.salt", "tests/test_dom_ffi_read.salt",
    "tests/test_event_dispatcher.salt", "tests/test_browser_spa_boot.salt",
    "tests/test_browser_event_bubbling.salt", "tests/test_layout_engine.salt",
    "user/netd/crypto/ecdsa.salt", "user/netd/crypto/bigint.salt",
    "user/browser/js_bytecode.salt", "user/browser/paint.salt",
    "user/browser/js_vm.salt", "user/browser/js_dom_bridge.c",
    "user/browser/worker.salt", "user/browser/alloc/validator.salt",
    "user/os/process.salt", "docs/deep-dives/universal-abi.md", "salt-front/std/time.salt"
]

for filepath in files_to_check:
    if not os.path.exists(filepath): continue
    with open(filepath, "r") as f: content = f.read()
    new_content = chain_pattern.sub("", content)
    if new_content != content:
        with open(filepath, "w") as f: f.write(new_content)
        print(f"Cleaned {filepath}")
