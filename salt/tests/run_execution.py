import sys
import subprocess
import os

def run_cmd(cmd):
    print(f"[RUN] {cmd}")
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
    if result.returncode != 0:
        print(f"[FAIL] {cmd}")
        print(f"STDOUT:\n{result.stdout}")
        print(f"STDERR:\n{result.stderr}")
        return False
    return True

def main():
    if len(sys.argv) < 2:
        print("Usage: python3 run_execution.py <salt_file>")
        sys.exit(1)

    salt_file = os.path.abspath(sys.argv[1])
    basename = os.path.basename(salt_file).replace(".salt", "")
    base_dir = os.path.dirname(salt_file)
    
    # Locate Binaries relative to this script
    script_dir = os.path.dirname(os.path.abspath(__file__))
    # Script is in keuos/salt/tests
    # Root is keuos
    project_root = os.path.abspath(os.path.join(script_dir, "../../"))
    
    salt_front_bin = os.path.join(project_root, "salt-front", "target", "release", "salt-front")
    salt_opt_bin = os.path.join(project_root, "salt", "build", "salt-opt")
    
    if not os.path.exists(salt_front_bin):
        print(f"Error: salt-front binary not found at {salt_front_bin}")
        sys.exit(1)
    if not os.path.exists(salt_opt_bin):
        print(f"Error: salt-opt binary not found at {salt_opt_bin}")
        sys.exit(1)

    mlir_file = os.path.join(base_dir, f"{basename}.mlir")
    ll_file = os.path.join(base_dir, f"{basename}.ll")
    exe_file = os.path.join(base_dir, f"{basename}")

    # 1. Salt -> MLIR
    print("--- Step 1: Frontend (Salt -> MLIR) ---")
    if not run_cmd(f"{salt_front_bin} {salt_file} > {mlir_file}"):
        sys.exit(1)

    # 2. MLIR -> LLVM IR
    # Note: Using --emit-llvm because direct object emission has issues on some setups
    print("--- Step 2: Backend (MLIR -> LLVM IR) ---")
    if not run_cmd(f"{salt_opt_bin} --emit-llvm --verify=false --output {ll_file} {mlir_file}"):
        sys.exit(1)
        
    # 3. LLVM IR -> Executable (via Clang)
    print("--- Step 3: Compilation (LLVM IR -> Executable) ---")
    if not run_cmd(f"clang {ll_file} -o {exe_file}"):
        sys.exit(1)
        
    # 4. Execute
    print(f"--- Step 4: Execution (./{basename}) ---")
    try:
        res = subprocess.run(exe_file, capture_output=True, text=True)
        print(f"Exit Code: {res.returncode}")
        
        # Test Case Verification
        if basename == "hello" and res.returncode == 42:
            print("SUCCESS: hello.salt returned 42")
        elif basename == "verification_pass" and res.returncode == 0:
             print("SUCCESS: verification_pass executed (Exit 0)")
        elif basename == "opt_option_test" and res.returncode == 1:
             print("SUCCESS: opt_option_test Niche Optimization Verified (is_some(ptr) == 1)")
        elif basename == "verification_fail":
            pass # Expect verification error, not execution
        else:
            print(f"Execution finished with code {res.returncode}")

        sys.exit(res.returncode)
    except Exception as e:
        print(f"[FAIL] Execution failed: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
