#!/usr/bin/env python3
"""
HTML Lexer Differential Comparator.
Runs Salt lexer and Python reference on the same HTML file,
normalizes both outputs, and compares them line-by-line.
"""
import subprocess
import sys
import os
import html

def run_command(cmd, env=None):
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True, env=env)
    return result.stdout, result.stderr, result.returncode

def extract_ir(output, header_marker):
    """Extract IR lines after the header marker, filtering empty lines."""
    lines = []
    found_header = False
    for line in output.splitlines():
        if header_marker in line:
            found_header = True
            continue
        if found_header:
            stripped = line.strip()
            if stripped:  # Skip empty lines from both sides
                if stripped.startswith("ATTR N="):
                    parts = stripped.split(" V=", 1)
                    if len(parts) == 2:
                        parts[1] = html.unescape(parts[1])
                        stripped = parts[0] + " V=" + parts[1]
                lines.append(stripped)
    return lines

def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <html_file>")
        sys.exit(1)

    html_file = sys.argv[1]
    
    # 1. Run Salt Lexer
    env = os.environ.copy()
    env["LEXER_TEST_HTML"] = html_file
    salt_out, salt_err, ret = run_command("./test_lexer_diff", env=env)
    if ret != 0:
        print(f"Error running test_lexer_diff (exit {ret})")
        if salt_err:
            print(f"  stderr: {salt_err.strip()}")
        sys.exit(1)
    
    salt_ir = extract_ir(salt_out, "== SALT LEXER IR =")
    if not salt_ir:
        print("ERROR: Salt lexer produced no IR output")
        sys.exit(1)
    
    # 2. Run Python Reference
    py_out, py_err, ret = run_command(f"python3 tests/lexer_reference.py {html_file}")
    if ret != 0:
        print(f"Error running lexer_reference.py (exit {ret})")
        if py_err:
            print(f"  stderr: {py_err.strip()}")
        sys.exit(1)
    
    py_ir = [line.strip() for line in py_out.splitlines() if line.strip()]
    if not py_ir:
        print("ERROR: Python reference produced no IR output")
        sys.exit(1)

    # 3. Compare
    if salt_ir == py_ir:
        print(f"✅ HTML Lexer Differential Test PASSED: {html_file} ({len(salt_ir)} lines)")
        sys.exit(0)
    else:
        print(f"❌ HTML Lexer Differential Test FAILED: {html_file}")
        print(f"   Salt: {len(salt_ir)} lines  |  Reference: {len(py_ir)} lines")
        import difflib
        diff = list(difflib.unified_diff(py_ir, salt_ir, fromfile='reference', tofile='prisimi', lineterm=''))
        for line in diff:
            print(f"  {line}")
        sys.exit(1)

if __name__ == "__main__":
    main()
