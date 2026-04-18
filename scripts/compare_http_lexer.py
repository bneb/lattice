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
    # Only keep fields that are common to both Salt and Rust
    # Common fields: IS_CHUNKED, BODY_LEN, BODY_HEAD, CONTENT_LENGTH
    common_fields = ["IS_CHUNKED", "BODY_LEN", "BODY_HEAD", "CONTENT_LENGTH"]
    lines = []
    for line in output.splitlines():
        line = line.strip()
        if "=" in line:
            key = line.split("=")[0]
            if key in common_fields:
                lines.append(line)
        elif line.startswith("=="):
            lines.append(line)
    
    # Filter out potential debug logs
    return [l for l in lines if not l.startswith("Lexer parsing bytes")]

def main():
    if len(sys.argv) < 2:
        print("Usage: python3 compare_http_lexer.py <fixture_path>")
        sys.exit(1)

    fixture = sys.argv[1]
    
    # Run Salt version
    env = os.environ.copy()
    env["HTTP_TEST_FILE"] = fixture
    salt_output = run_command(["./test_http_lexer_diff"], env=env)
    salt_ir = normalize_ir(salt_output)

    # Run Rust reference version
    rust_output = run_command(["./tools/rust_http_parser/target/release/rust_http_parser", fixture])
    rust_ir = normalize_ir(rust_output)

    if salt_ir == rust_ir:
        print(f"✅ HTTP Lexer Differential Test PASSED: {fixture}")
    else:
        print(f"❌ HTTP Lexer Differential Test FAILED: {fixture}")
        
        # Write diff to temporary files for convenience
        with open("/tmp/salt_http_ir.txt", "w") as f: f.write("\n".join(salt_ir))
        with open("/tmp/rust_http_ir.txt", "w") as f: f.write("\n".join(rust_ir))
        
        print("--- Diff (Rust-Reference vs Salt-Prisimi) ---")
        subprocess.run(["diff", "-u", "/tmp/rust_http_ir.txt", "/tmp/salt_http_ir.txt"])
        
        sys.exit(1)

if __name__ == "__main__":
    main()
