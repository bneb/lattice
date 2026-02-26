import glob
import os
import subprocess
import sys
import time

def main():
    print("=== Cleaning previous test build artifacts ===")
    
    # We want to link all qemu_build/*.o MINUS suite.o
    # Since df_test_runner.o replaces suite.o, we compile it now.
    
    salt_front = "salt-front/target/release/salt-front"
    salt_opt = "salt/build/salt-opt"
    llc = "/opt/homebrew/opt/llvm/bin/llc"
    lld = "/Users/kevin/.rustup/toolchains/stable-aarch64-apple-darwin/lib/rustlib/aarch64-apple-darwin/bin/rust-lld"
    sed_fix = 's/"target-cpu"="[^"]*"/"target-cpu"="x86-64"/g; s/"target-features"="[^"]*"/"target-features"=""/g; s/getelementptr inbounds nuw/getelementptr inbounds/g'

    os.environ["DYLD_LIBRARY_PATH"] = "/opt/homebrew/lib"
    
    # Compile df_test_runner
    print("=== Compiling df_test_runner ===")
    src = "kernel/core/df_test_runner.salt"
    out = "df_test_runner"
    
    subprocess.run(f"{salt_front} {src} --lib --no-verify --disable-alias-scopes > /tmp/{out}.mlir", shell=True, check=True)
    subprocess.run(f"{salt_opt} --emit-llvm --verify=false < /tmp/{out}.mlir 2>/dev/null | sed '{sed_fix}' > /tmp/{out}.ll", shell=True, check=True)
    subprocess.run(f"{llc} /tmp/{out}.ll -filetype=obj -o qemu_build/{out}.o -relocation-model=pic -mtriple=x86_64-none-elf -mcpu=x86-64", shell=True, check=True)
    
    print("=== Linking ===")
    all_objs = glob.glob("qemu_build/*.o")
    # Filter out suite.o and bench.o or whatever might conflict
    link_objs = [o for o in all_objs if not o.endswith("suite.o") and not o.endswith("bench.o")]
    
    # Check if df_test_runner.o is in link_objs
    if "qemu_build/df_test_runner.o" not in link_objs:
        link_objs.append("qemu_build/df_test_runner.o")
        
    cmd = [lld, "-flavor", "gnu", "-T", "kernel/arch/x86/linker.ld", "-o", "qemu_build/kernel_df.elf", "-z", "max-page-size=0x1000"] + link_objs
    subprocess.run(cmd, check=True)
    
    print("=== Running QEMU ===")
    
    start = time.time()
    success = False
    lines = []
    
    qemu_cmd = [
        "qemu-system-x86_64", "-kernel", "qemu_build/kernel_df.elf", 
        "-nographic", "-m", "512M", "-cpu", "qemu64,+fxsr,+mmx,+sse,+sse2,+xsave", 
        "-no-reboot", "-serial", "mon:stdio"
    ]
    
    proc = subprocess.Popen(qemu_cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=False) # raw bytes
    
    print("QEMU Launched. Streaming output:")
    line_so_far = b""
    
    try:
        while time.time() - start < 5:
            c = proc.stdout.read(1)
            if c:
                sys.stdout.buffer.write(c)
                sys.stdout.buffer.flush()
                line_so_far += c
                # Every newline check for the string
                if c == b'\n':
                    decoded = line_so_far.decode('utf-8', errors='ignore')
                    if "[FATAL] DOUBLE FAULT (#DF)" in decoded or "#DF!" in decoded:
                        success = True
                        break
                    line_so_far = b""
            if proc.poll() is not None:
                break
    except KeyboardInterrupt:
        pass
        
    proc.terminate()
    try:
        proc.wait(timeout=3)
    except:
        proc.kill()
        
    if success:
        print("\nTEST PASSED: Verified isolated Double Fault Panic.")
        sys.exit(0)
    else:
        print("\nTEST FAILED: Did not see terminal #DF trace.")
        sys.exit(1)

if __name__ == "__main__":
    main()
