import subprocess
import sys
import os

def run_command(cmd, env=None):
    result = subprocess.run(cmd, capture_output=True, text=True, env=env)
    if result.returncode != 0:
        print(f"Error running {' '.join(cmd)}:")
        print(result.stderr)
        sys.exit(1)
    return result.stdout

def normalize_ir(output):
    lines = [line.strip() for line in output.splitlines() if line.strip().startswith("==") or line.strip().startswith("TOKEN ")]
    return lines

def main():
    if len(sys.argv) < 2:
        print("Usage: python3 compare_js_lexer.py <fixture_path>")
        sys.exit(1)

    fixture = sys.argv[1]
    
    # Run Salt version
    env = os.environ.copy()
    env["JS_TEST_FILE"] = fixture
    salt_output = run_command(["./test_js_lexer_diff"], env=env)
    salt_ir = normalize_ir(salt_output)

    # Run Python reference version (using venv in root)
    venv_python = os.path.join(os.getcwd(), ".venv/bin/python3")
    script_path = os.path.join(os.getcwd(), "tools/python_js_lexer/reference_js_lexer.py")
    rust_output = run_command([venv_python, script_path, fixture])
    rust_ir = normalize_ir(rust_output)

    if salt_ir == rust_ir:
        print(f"✅ JS Lexer Differential Test PASSED: {fixture}")
    else:
        print(f"❌ JS Lexer Differential Test FAILED: {fixture}")
        print("--- Salt IR ---")
        print("\n".join(salt_ir))
        print("--- Python IR ---")
        print("\n".join(rust_ir))
        
        # Write diff
        with open("/tmp/salt_js_ir.txt", "w") as f: f.write("\n".join(salt_ir))
        with open("/tmp/python_js_ir.txt", "w") as f: f.write("\n".join(rust_ir))
        subprocess.run(["diff", "-u", "/tmp/python_js_ir.txt", "/tmp/salt_js_ir.txt"])
        
        sys.exit(1)

if __name__ == "__main__":
    main()
