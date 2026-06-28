#!/usr/bin/env python3
import subprocess
import re
import sys
import os

def run_test(salt_file):
    print(f"[TEST] {salt_file}")
    
    # 0. Find saltc
    salt_front_cmd = "saltc"
    if os.path.exists("salt-front/target/release/saltc"):
        salt_front_cmd = "./salt-front/target/release/saltc"
    
    # 1. Compile to MLIR
    try:
        # Check if file exists
        if not os.path.exists(salt_file):
            print(f"FAILED: File not found {salt_file}")
            return False

        mlir_out = subprocess.check_output(
            [salt_front_cmd, salt_file], stderr=subprocess.STDOUT
        ).decode()
    except subprocess.CalledProcessError as e:
        print(f"FAILED: Compilation error\n{e.output.decode()}")
        return False
    except FileNotFoundError:
        print("FAILED: saltc not found in PATH")
        return False

    # 2. Extract CHECK comments from the Salt file
    with open(salt_file, 'r') as f:
        checks = [line.split("CHECK:")[1].strip() for line in f if "CHECK:" in line]

    if not checks:
        print("WARNING: No CHECK patterns found via // CHECK:")
        return True # Or False depending on strictness, but let's pass for now if just checking compilation

    # 3. Verify patterns in MLIR
    for pattern in checks:
        if not re.search(pattern, mlir_out):
            print(f"FAILED: Pattern not found -> {pattern}")
            # print("Output was:\n" + mlir_out) 
            return False
    
    print("PASSED")
    return True

if __name__ == "__main__":
    if not os.path.exists("tests/regression"):
        print("tests/regression directory not found")
        sys.exit(0)

    results = [run_test(os.path.join("tests/regression", f)) 
               for f in os.listdir("tests/regression") if f.endswith(".salt")]
    
    if not results:
        print("No regression tests found.")
        sys.exit(0)

    sys.exit(0 if all(results) else 1)
