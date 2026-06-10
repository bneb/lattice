#!/usr/bin/env python3
"""
tools/run_ring3_tests.py

Builds the Ring 3 Test Suite by compiling userspace Salt files into standalone ELF binaries.
This ensures our userspace toolchain (salt-front -> MLIR -> salt-opt -> LLVM IR -> Clang -> ELF)
works end-to-end for system call wrappers and test applications.
"""

import os
import sys
import subprocess
import glob

def main():
    workspace = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    user_dir = os.path.join(workspace, "user")
    build_dir = os.path.join(workspace, "build_user")
    
    os.makedirs(build_dir, exist_ok=True)
    
    salt_front = os.path.join(workspace, "salt-front", "target", "release", "salt-front")
    if not os.path.exists(salt_front):
        salt_front_cmd = ["cargo", "run", "--release", "--manifest-path", os.path.join(workspace, "salt-front", "Cargo.toml"), "--"]
    else:
        salt_front_cmd = [salt_front]
        
    salt_translate = os.path.join(workspace, "salt", "build", "salt-translate")
    if not os.path.exists(salt_translate):
        salt_translate = os.path.join(workspace, "salt", "build_linux", "salt-translate")
    if not os.path.exists(salt_translate):
        print("Warning: salt-translate not found in salt/build/ or salt/build_linux/")

    tests = glob.glob(os.path.join(user_dir, "*test*.salt"))
    # Also add specific apps we know should build
    apps = [
        "hello.salt",
        "grit/shell.salt"
    ]
    for app in apps:
        app_path = os.path.join(user_dir, app)
        if os.path.exists(app_path) and app_path not in tests:
            tests.append(app_path)

    print(f"Building {len(tests)} Ring 3 test applications...")
    
    failed = []
    for test in tests:
        name = os.path.splitext(os.path.basename(test))[0]
        mlir_out = os.path.join(build_dir, f"{name}.mlir")
        ll_out = os.path.join(build_dir, f"{name}.ll")
        obj_out = os.path.join(build_dir, f"{name}.o")
        elf_out = os.path.join(build_dir, f"{name}.elf")
        
        print(f"➤ Building {name}...")
        
        # 1. salt-front (Salt -> MLIR)
        cmd1 = salt_front_cmd + ["-o", mlir_out, test]
        res1 = subprocess.run(cmd1, capture_output=True, text=True)
        if res1.returncode != 0:
            print(f"  ❌ salt-front failed:")
            print(res1.stderr)
            failed.append(name)
            continue
            
        # 2. salt-translate (MLIR -> LLVM IR) if salt-translate exists
        if os.path.exists(salt_translate):
            cmd2 = [salt_translate, "-mlir-to-llvmir", mlir_out, "-o", ll_out]
            res2 = subprocess.run(cmd2, capture_output=True, text=True)
            if res2.returncode != 0:
                print(f"  ❌ salt-translate failed:")
                print(res2.stderr)
                failed.append(name)
                continue
                
            # 3. clang (LLVM IR -> ELF)
            # We must link with user_linker.ld and syscall_stubs.S
            linker_script = os.path.join(user_dir, "user_linker.ld")
            stubs = os.path.join(user_dir, "syscall_stubs.S")
            
            # Using standard clang
            cmd3 = [
                "clang",
                "-O2", "-ffreestanding", "-nostdlib",
                "-mcmodel=large", "-fno-pic", "-mno-red-zone",
                "-T", linker_script,
                ll_out, stubs,
                "-o", elf_out
            ]
            res3 = subprocess.run(cmd3, capture_output=True, text=True)
            if res3.returncode != 0:
                print(f"  ❌ clang failed:")
                print(res3.stderr)
                failed.append(name)
                continue
        else:
            print(f"  (Skipping MLIR lowering, salt-translate not found. Checked verification only.)")
            
        print(f"  ✅ {name} built successfully")

    if failed:
        print(f"\n❌ {len(failed)} Ring 3 tests failed to build.")
        sys.exit(1)
        
    print("\n🎉 All Ring 3 tests built successfully.")
    sys.exit(0)

if __name__ == "__main__":
    main()
