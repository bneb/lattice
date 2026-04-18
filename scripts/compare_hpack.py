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
    lines = [line.strip() for line in output.splitlines() if line.strip().startswith("==") or line.strip().startswith("HDR ")]
    return lines

def main():
    if len(sys.argv) < 2:
        print("Usage: python3 compare_hpack.py <fixture_path>")
        sys.exit(1)

    fixture = sys.argv[1]
    
    # Run Salt version
    env = os.environ.copy()
    env["HPACK_TEST_FILE"] = fixture
    salt_output = run_command(["./test_hpack_diff"], env=env)
    salt_ir = normalize_ir(salt_output)

    # Run Rust reference version
    rust_output = run_command(["./tools/rust_hpack_decoder/target/release/rust_hpack_decoder", fixture])
    rust_ir = normalize_ir(rust_output)

    if salt_ir == rust_ir:
        print(f"✅ HPACK Differential Test PASSED: {fixture}")
    else:
        print(f"❌ HPACK Differential Test FAILED: {fixture}")
        print("--- Salt IR ---")
        print("\n".join(salt_ir))
        print("--- Rust IR ---")
        print("\n".join(rust_ir))
        
        # Write diff
        with open("/tmp/salt_hpack_ir.txt", "w") as f: f.write("\n".join(salt_ir))
        with open("/tmp/rust_hpack_ir.txt", "w") as f: f.write("\n".join(rust_ir))
        subprocess.run(["diff", "-u", "/tmp/rust_hpack_ir.txt", "/tmp/salt_hpack_ir.txt"])
        
        sys.exit(1)

if __name__ == "__main__":
    main()
