#!/usr/bin/env python3
"""
CSS Lexer Differential Comparator.
Runs Salt CSS lexer and Rust cssparser reference on the same CSS file,
normalizes and sorts both outputs (hash table order is nondeterministic),
and compares them line-by-line.

The Rust reference uses the industry-standard `cssparser` crate v0.34.
"""
import subprocess
import sys
import os

RUST_PARSER = "./tools/rust_css_parser/target/release/rust_css_parser"

def run_command(cmd, env=None):
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True, env=env)
    return result.stdout, result.stderr, result.returncode

def extract_rules(output):
    """Extract RULE lines after the header, filtering empty lines."""
    lines = []
    found_header = False
    for line in output.splitlines():
        if "== CSS LEXER IR" in line:
            found_header = True
            continue
        if found_header:
            stripped = line.strip()
            if stripped and stripped.startswith("RULE"):
                lines.append(stripped)
    return sorted(lines)

def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <css_file>")
        sys.exit(1)

    css_file = sys.argv[1]
    
    # 1. Run Salt Lexer
    env = os.environ.copy()
    env["CSS_TEST_FILE"] = css_file
    salt_out, salt_err, ret = run_command("./test_css_lexer_diff", env=env)
    if ret != 0:
        print(f"Error running test_css_lexer_diff (exit {ret})")
        if salt_err:
            print(f"  stderr: {salt_err.strip()}")
        sys.exit(1)
    
    salt_ir = extract_rules(salt_out)
    if not salt_ir:
        print("ERROR: Salt CSS lexer produced no rules")
        sys.exit(1)
    
    # 2. Run Rust Reference (production-grade cssparser crate)
    if not os.path.exists(RUST_PARSER):
        print(f"ERROR: Rust CSS parser not found at {RUST_PARSER}")
        print(f"  Run: cd tools/rust_css_parser && cargo build --release")
        sys.exit(1)
    
    rust_out, rust_err, ret = run_command(f"{RUST_PARSER} {css_file}")
    if ret != 0:
        print(f"Error running rust_css_parser (exit {ret})")
        if rust_err:
            print(f"  stderr: {rust_err.strip()}")
        sys.exit(1)
    
    rust_ir = extract_rules(rust_out)
    if not rust_ir:
        print("ERROR: Rust CSS reference produced no rules")
        sys.exit(1)

    # 3. Compare
    if salt_ir == rust_ir:
        print(f"✅ CSS Lexer Differential Test PASSED: {css_file} ({len(salt_ir)} rules)")
        sys.exit(0)
    else:
        print(f"❌ CSS Lexer Differential Test FAILED: {css_file}")
        print(f"   Salt: {len(salt_ir)} rules  |  Rust Reference: {len(rust_ir)} rules")
        import difflib
        diff = list(difflib.unified_diff(rust_ir, salt_ir, fromfile='rust_reference', tofile='prisimi', lineterm=''))
        for line in diff:
            print(f"  {line}")
        sys.exit(1)

if __name__ == "__main__":
    main()
