#!/usr/bin/env python3
"""
tools/verify_user.py

A gating script to find and verify all userspace `.salt` files.
This ensures that formal verification (Z3) is checked for userspace contributions.
"""

import os
import sys
import subprocess
import glob

def find_salt_files(base_dir):
    return glob.glob(os.path.join(base_dir, "**", "*.salt"), recursive=True)

def main():
    workspace = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    user_dir = os.path.join(workspace, "user")
    
    # We rely on salt-front being built
    salt_front = os.path.join(workspace, "salt-front", "target", "release", "salt-front")
    
    if not os.path.exists(salt_front):
        # Fallback to cargo run if binary doesn't exist
        print(f"Warning: {salt_front} not found. Ensure `cargo build --release` is run.")
        print("Using `cargo run` as fallback...")
        salt_front_cmd = ["cargo", "run", "--release", "--manifest-path", os.path.join(workspace, "salt-front", "Cargo.toml"), "--"]
    else:
        salt_front_cmd = [salt_front]
        
    files_to_check = find_salt_files(user_dir)
    print(f"Found {len(files_to_check)} userspace .salt files to verify.")
    
    failed = []
    
    for file in files_to_check:
        print(f"Verifying {os.path.relpath(file, workspace)}...")
        
        # We just need to typecheck/verify. salt-front will run Z3.
        # If the file compiles to MLIR, it means Z3 checks passed.
        cmd = salt_front_cmd + ["--lib", file]
        
        result = subprocess.run(cmd, capture_output=True, text=True)
        if result.returncode != 0:
            print(f"❌ Verification failed for {os.path.basename(file)}")
            print(result.stderr)
            failed.append(file)
        else:
            print(f"✅ Pass")
            
    if failed:
        print(f"\n{len(failed)} files failed verification.")
        sys.exit(1)
        
    print("\n🎉 All userspace files passed formal verification.")
    sys.exit(0)

if __name__ == "__main__":
    main()
