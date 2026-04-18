import re

with open("tests/test_e2e_http2.salt", "r") as f:
    text = f.read()

def repl(m):
    s = m.group(1).encode('utf-8')
    a = ", ".join(str(b) for b in s)
    return f"let hname: [u8; {len(s)}] = [{a}];\n    sys_net_init_h2_connection((&hname) as u64);"

def repl_fetch(m):
    s = m.group(1).encode('utf-8')
    a = ", ".join(str(b) for b in s)
    return f"let fetch_arr: [u8; {len(s)}] = [{a}];\n    js_quickjs_eval_string((&fetch_arr) as u64, {len(s)});"

def repl_verify(m):
    s = m.group(1).encode('utf-8')
    a = ", ".join(str(b) for b in s)
    return f"let verify_arr: [u8; {len(s)}] = [{a}];\n    js_quickjs_eval_string((&verify_arr) as u64, {len(s)});"

# Fix hname
text = re.sub(r'let hname = "(prisimi\.io\\0)";\n\s*sys_net_init_h2_connection\(hname\.ptr\);', 
              lambda m: f"let hname: [u8; 11] = [112, 114, 105, 115, 105, 109, 105, 46, 105, 111, 0];\n    sys_net_init_h2_connection((&hname) as u64);", text)

# Fix fetch_script
text = re.sub(r'let fetch_script = "([^"]+)";\n\s*js_quickjs_eval_string\(fetch_script\.ptr as u64, 210\);', repl_fetch, text)

# Fix verify_script
text = re.sub(r'let verify_script = "([^"]+)";\n\s*js_quickjs_eval_string\(verify_script\.ptr as u64, 280\);', repl_verify, text)

with open("tests/test_e2e_http2.salt", "w") as f:
    f.write(text)
