#!/usr/bin/env python3
import subprocess
import os
import sys
import json
import re

def run_test(salt_file):
    salt_front_cmd = "saltc"
    if os.path.exists("salt-front/target/release/saltc"):
        salt_front_cmd = "./salt-front/target/release/saltc"
        
    try:
        mlir_out = subprocess.check_output(
            [salt_front_cmd, salt_file], stderr=subprocess.STDOUT
        ).decode()
    except subprocess.CalledProcessError as e:
        return {"status": "FAILED", "reason": "Compilation error", "output": e.output.decode()}
    except FileNotFoundError:
        return {"status": "FAILED", "reason": "saltc not found"}

    with open(salt_file, 'r') as f:
        checks = [line.split("CHECK:")[1].strip() for line in f if "CHECK:" in line]

    for pattern in checks:
        if not re.search(pattern, mlir_out):
            return {"status": "FAILED", "reason": f"Pattern not found: {pattern}"}
            
    return {"status": "PASSED"}

def main():
    test_dir = "tests"
    results = {}
    passed = 0
    failed = 0
    
    for root, dirs, files in os.walk(test_dir):
        for file in files:
            if file.endswith(".salt"):
                filepath = os.path.join(root, file)
                res = run_test(filepath)
                results[filepath] = res
                if res["status"] == "PASSED":
                    passed += 1
                else:
                    failed += 1
                    
    print(f"Total: {passed + failed}, Passed: {passed}, Failed: {failed}")
    with open("tests_report.json", "w") as f:
        json.dump(results, f, indent=2)

if __name__ == "__main__":
    main()
