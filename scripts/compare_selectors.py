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
    lines = [line.strip() for line in output.splitlines() if line.strip().startswith("==") or line.strip().startswith("MATCH ")]
    return lines

def main():
    if len(sys.argv) < 3:
        print("Usage: python3 compare_selectors.py <selector> <html_fixture>")
        sys.exit(1)

    selector = sys.argv[1]
    fixture = sys.argv[2]
    
    # Run Salt version
    env = os.environ.copy()
    env["SELECTOR_STRING"] = selector
    salt_output = run_command(["./test_selectors_diff"], env=env)
    salt_ir = normalize_ir(salt_output)

    # Run Rust reference version
    rust_output = run_command(["./tools/rust_selectors/target/release/rust_selectors", fixture, selector])
    rust_ir = normalize_ir(rust_output)

    if salt_ir == rust_ir:
        print(f"✅ Selectors Differential Test PASSED: '{selector}' on {fixture}")
    else:
        print(f"❌ Selectors Differential Test FAILED: '{selector}' on {fixture}")
        print("--- Salt IR ---")
        print("\n".join(salt_ir))
        print("--- Rust IR ---")
        print("\n".join(rust_ir))
        sys.exit(1)

if __name__ == "__main__":
    main()
