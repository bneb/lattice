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
    lines = [line.strip() for line in output.splitlines() if line.strip().startswith("NODE ")]
    return lines

def main():
    if len(sys.argv) < 2:
        print("Usage: python3 compare_layout.py <layout_json>")
        sys.exit(1)

    fixture = sys.argv[1]
    
    # Run Salt version
    salt_output = run_command(["./test_layout_diff"])
    salt_ir = normalize_ir(salt_output)

    # Run Rust reference version (passing JSON fixture)
    rust_output = run_command(["./tools/rust_layout/target/release/rust_layout", fixture])
    rust_ir = normalize_ir(rust_output)

    if salt_ir == rust_ir:
        print(f"✅ Layout Differential Test PASSED: {fixture}")
    else:
        print(f"❌ Layout Differential Test FAILED: {fixture}")
        print("--- Salt IR ---")
        print("\n".join(salt_ir))
        print("--- Rust IR ---")
        print("\n".join(rust_ir))
        sys.exit(1)

if __name__ == "__main__":
    main()
