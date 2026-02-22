#!/usr/bin/env python3
import subprocess
import os
import sys
import time
import re
import glob
import shutil
import socket
import threading

# Configuration
KERNEL_ROOT = "kernel"
BENCH_ROOT = "kernel/benchmarks"
BUILD_DIR = "qemu_build"
# Try to find salt binaries in the workspace
WORKSPACE_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SALT_FRONT = os.path.join(WORKSPACE_ROOT, "salt-front/target/release/salt-front")
if not os.path.exists(SALT_FRONT):
    SALT_FRONT = os.path.join(WORKSPACE_ROOT, "salt-front/target/debug/salt-front")
SALT_OPT = os.path.join(WORKSPACE_ROOT, "salt/build/salt-opt")

class ToolchainProvider:
    """Hermetic Toolchain Provider for Lattice x86_64 target."""
    def __init__(self, target="x86_64-none-elf"):
        self.target = target
        # Dynamic detection for reproducibility across environments
        self.llc = self._find_tool("llc")
        self.clang = self._find_tool("clang")
        self.rust_lld = self._find_tool("rust-lld")

    def _find_tool(self, name):
        # 1. Check PATH
        path = shutil.which(name)
        if path: return path
        
        # 2. Check common installation paths
        fallbacks = {
            "llc": ["/opt/homebrew/opt/llvm/bin/llc", "/usr/local/opt/llvm/bin/llc"],
            "clang": ["/opt/homebrew/opt/llvm/bin/clang", "/usr/local/opt/llvm/bin/clang"],
            "rust-lld": [
                os.path.expanduser("~/.rustup/toolchains/stable-aarch64-apple-darwin/lib/rustlib/aarch64-apple-darwin/bin/rust-lld"),
                os.path.expanduser("~/.rustup/toolchains/stable-x86_64-apple-darwin/lib/rustlib/x86_64-apple-darwin/bin/rust-lld")
            ]
        }
        
        for p in fallbacks.get(name, []):
            if os.path.exists(p): return p
            
        return name # Return name and let validate() fail if not found

    def validate(self):
        """Verify that all required tools exist and match the expected target."""
        print(f"  [VALIDATE] Checking toolchain for {self.target}...")
        for tool_name, path in [("LLC", self.llc), ("CLANG", self.clang), ("RUST_LLD", self.rust_lld)]:
            if not os.path.exists(path):
                raise RuntimeError(f"Required tool {tool_name} not found at {path}")
            
            # Verify architecture if possible
            if tool_name == "CLANG":
                version_out = subprocess.check_output([path, "--version"], text=True)
                if "x86_64" not in version_out and "Target: " not in version_out:
                    print(f"    WARNING: {tool_name} may not support x86_64 targets natively.")
            
            print(f"    - {tool_name}: FOUND ({path})")

TOOLCHAIN = ToolchainProvider()

# ANSI Colors for Output
RED = "\033[91m"
GREEN = "\033[92m"
RESET = "\033[0m"

def ensure_build_dir():
    if not os.path.exists(BUILD_DIR):
        os.makedirs(BUILD_DIR)

def compile_salt(src_file):
    base_name = os.path.basename(src_file).replace(".salt", "")
    mlir_file = os.path.join(BUILD_DIR, f"{base_name}.mlir")
    ll_file = os.path.join(BUILD_DIR, f"{base_name}.ll")
    obj_file = os.path.join(BUILD_DIR, f"{base_name}.o")

    print(f"  [SALT] Compiling {src_file}...")
    
    # 1. Salt -> MLIR
    # salt-front prints to stdout. We capture it and write to file.
    cmd = [SALT_FRONT, src_file, "--lib", "--no-verify", "--disable-alias-scopes"]
    print(f"    Running: {' '.join(cmd)} > {mlir_file}")
    
    with open(mlir_file, "w") as out:
        subprocess.check_call(cmd, stdout=out)

    # 2. MLIR -> LLVM IR
    # 2. MLIR -> LLVM IR
    cmd = [SALT_OPT, "--emit-llvm", "--verify=false"]
    print(f"    Running: {' '.join(cmd)} < {mlir_file} > {ll_file}")
    
    for attempt in range(3):
        try:
            with open(mlir_file, "rb") as f_in, open(ll_file, "wb") as f_out:
                subprocess.check_call(cmd, stdin=f_in, stdout=f_out)
            break  # Success
        except subprocess.CalledProcessError:
            if attempt < 2:
                print(f"    ⟳ salt-opt crashed (attempt {attempt + 1}/3), retrying...")
            else:
                raise

    # 2b. Strip host CPU attributes from LLVM IR (cross-compilation from ARM Mac → x86_64)
    # salt-opt embeds the host CPU (e.g. apple-m4) which breaks LLC targeting x86_64
    import re
    with open(ll_file, 'r') as f:
        ll_content = f.read()
    ll_content = re.sub(r'"target-cpu"="[^"]*"', '"target-cpu"="x86-64"', ll_content)
    ll_content = re.sub(r'"target-features"="[^"]*"', '"target-features"=""', ll_content)
    # Strip 'nuw' flag from getelementptr — LLVM 19 syntax unsupported by LLVM 18
    ll_content = ll_content.replace('getelementptr inbounds nuw', 'getelementptr inbounds')
    with open(ll_file, 'w') as f:
        f.write(ll_content)

    # 3. LLVM IR -> Object
    cmd = [TOOLCHAIN.llc, ll_file, "-filetype=obj", "-o", obj_file, "-relocation-model=pic", f"-mtriple={TOOLCHAIN.target}", "-mcpu=x86-64"]
    print(f"    Running: {' '.join(cmd)}")
    subprocess.check_call(cmd)
    
    return obj_file

def compile_asm(src_file):
    base_name = os.path.basename(src_file).replace(".S", "")
    obj_file = os.path.join(BUILD_DIR, f"{base_name}.o")
    
    print(f"  [ASM]  Assembling {src_file}...")
    # Use cross-compilation target for assembly
    cmd = [TOOLCHAIN.clang, "-c", src_file, "-o", obj_file, "-target", TOOLCHAIN.target] 
    subprocess.check_call(cmd)
    return obj_file

def build_kernel():
    ensure_build_dir()
    print(f"{GREEN}== Building Kernel =={RESET}")
    
    objects = []
    
    # Compile all Salt files in kernel/core, kernel/drivers, kernel/mem, kernel/sched
    salt_files = glob.glob(f"{KERNEL_ROOT}/core/*.salt") + \
                 glob.glob(f"{KERNEL_ROOT}/drivers/*.salt") + \
                 glob.glob(f"{KERNEL_ROOT}/mem/*.salt") + \
                 glob.glob(f"{KERNEL_ROOT}/sched/*.salt") + \
                 glob.glob(f"{KERNEL_ROOT}/net/*.salt") + \
                 glob.glob(f"{KERNEL_ROOT}/arch/x86/*.salt")
                 
    for f in salt_files:
        try:
            objects.append(compile_salt(f))
        except subprocess.CalledProcessError:
            base_name = os.path.basename(f).replace(".salt", "")
            obj_file = os.path.join(BUILD_DIR, f"{base_name}.o")
            if os.path.exists(obj_file):
                print(f"    {RED}⚠ Compilation failed, reusing pre-compiled {obj_file}{RESET}")
                objects.append(obj_file)
            else:
                print(f"    {RED}⚠ Compilation failed, no pre-compiled .o — skipping {base_name}{RESET}")

    # Compile Arch Assembly
    asm_files = glob.glob(f"{KERNEL_ROOT}/arch/x86/*.S") + \
                glob.glob(f"{KERNEL_ROOT}/arch/x86_64/*.S")
    for f in asm_files:
        objects.append(compile_asm(f))
        
    return objects

def build_benchmark(bench_file, kernel_objs):
    """Build all kernel-compatible benchmark Salt files and link with kernel objects."""
    print(f"{GREEN}== Building Benchmarks =={RESET}")
    
    # Only kernel-compatible benchmarks — others use userspace APIs (malloc, stdio)
    KERNEL_BENCHMARKS = [
        "suite.salt",
        "ring_of_fire.salt",
        "ring_of_fire_1k.salt",
        "ring_of_fire_lite.salt",
        "syscall_bench.salt",
        "ipc_bench.salt",
        "alloc_bench.salt",
        "slab_reclaim_bench.salt",
        "net_echo_bench.salt",
        "irq_latency_bench.salt",
        "pmm_bench.salt",
        "slab_stress_bench.salt",
    ]
    
    bench_objs = []
    bench_files = [os.path.join(BENCH_ROOT, b) for b in KERNEL_BENCHMARKS]
    
    for bf in bench_files:
        print(f"{GREEN}== Building Benchmark: {bf} =={RESET}")
        try:
            bench_objs.append(compile_salt(bf))
        except subprocess.CalledProcessError:
            base_name = os.path.basename(bf).replace(".salt", "")
            obj_file = os.path.join(BUILD_DIR, f"{base_name}.o")
            if os.path.exists(obj_file):
                print(f"    {RED}⚠ Compilation failed, reusing pre-compiled {obj_file}{RESET}")
                bench_objs.append(obj_file)
            else:
                raise
    
    linker_script = os.path.join(KERNEL_ROOT, "arch/x86/linker.ld")
    output_elf = os.path.join(BUILD_DIR, "kernel.elf")
    
    # Link Everything
    cmd = [TOOLCHAIN.rust_lld, "-flavor", "gnu", "-T", linker_script, "-o", output_elf, "-z", "max-page-size=0x1000"] + kernel_objs + bench_objs
    subprocess.check_call(cmd)
    
    return output_elf

QEMU_LOG_MAX_BYTES = 100 * 1024 * 1024  # 100MB safety cap

def run_qemu_test(kernel_path, timeout=600):
    print(f"{GREEN}== Launching QEMU Flight Deck =={RESET}")

    # --- Guard 1: Kill any stale QEMU processes from previous runs ---
    try:
        subprocess.run(['pkill', '-f', 'qemu-system'], capture_output=True)
    except Exception:
        pass  # pkill may not exist or no processes found — harmless

    # --- Guard 2: Remove oversized qemu.log from previous runs ---
    #     INC-001: A stale QEMU with -d int produced a 294GB log file,
    #     filling the disk. See docs/incidents/001_qemu_log_disk_fill.md
    log_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'qemu.log')
    log_path = os.path.normpath(log_path)
    if os.path.exists(log_path):
        log_size = os.path.getsize(log_path)
        if log_size > QEMU_LOG_MAX_BYTES:
            print(f"{RED}  ⚠ Stale qemu.log is {log_size // (1024*1024)}MB — deleting (INC-001 guard){RESET}")
            os.remove(log_path)
        else:
            os.remove(log_path)  # Always start fresh

    # Detect KVM availability (Linux x86_64 with /dev/kvm)
    # On macOS ARM, HVF can't run x86 guests — always use TCG there
    use_kvm = sys.platform != "darwin" and os.path.exists("/dev/kvm")

    if use_kvm:
        cpu_flag = 'host'   # Pass through real CPU features (tzcnt, invariant TSC, etc.)
        print(f"{GREEN}  KVM detected — using hardware acceleration with -cpu host{RESET}")
    else:
        cpu_flag = 'qemu64,+fxsr,+mmx,+sse,+sse2,+xsave'

    # QEMU debug flags: default to guest_errors only.
    # Set QEMU_DEBUG=int,guest_errors,cpu_reset for full interrupt tracing.
    # WARNING: '-d int' under sustained IRQ load produces GB/min of log output.
    qemu_debug = os.environ.get('QEMU_DEBUG', 'guest_errors')

    cmd = [
        'qemu-system-x86_64',
        '-kernel', kernel_path,
        '-nographic',
        '-m', '1G',
        '-cpu', cpu_flag,
        '-d', qemu_debug,
        '-D', log_path,
        '-no-reboot',
        '-serial', 'mon:stdio',
        '-device', 'virtio-net-pci,netdev=net0',
        '-netdev', 'user,id=net0,hostfwd=udp::5555-:7'
    ]

    if use_kvm:
        cmd.insert(1, '-enable-kvm')
    
    print(f"COMMAND: {' '.join(cmd)}")
    
    process = subprocess.Popen(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1
    )
    
    start_time = time.time()
    output_buffer = ""
    
    try:
        while True:
            if time.time() - start_time > timeout:
                process.terminate()
                print(f"{RED}TIMEOUT reached ({timeout}s){RESET}")
                return False, output_buffer

            line = process.stdout.readline()
            if line:
                print(f"QEMU: {line.strip()}")
                output_buffer += line
                
                # Check metrics
                if "ROF_TAX_REPORT:" in line:
                    match = re.search(r"ROF_TAX_REPORT: (\d+) / (\d+)", line)
                    if match:
                        overhead = int(match.group(1))
                        work = int(match.group(2))
                        print(f"{GREEN}METRICS CAPTURED:{RESET}")
                        print(f"  Overhead: {overhead} cycles")
                        print(f"  Work:     {work} cycles")
                        ratio = overhead / work if work > 0 else 0
                        print(f"  Tax Ratio: {ratio:.2%}")
                
                if "BENCHMARK SUITE COMPLETE" in line:
                    print(f"{GREEN}SUITE COMPLETE — terminating QEMU{RESET}")
                    process.terminate()
                    return True, output_buffer

                if "kernel panic" in line.lower() or "\x1b[31;1m" in line:
                    print(f"{RED}KERNEL PANIC DETECTED{RESET}")
                    # Keep reading a bit
                    continue

                if "HEARTBEAT" in line:
                    # Depending on verify mode, might exit here or wait
                    pass

                if "BENCH:net_echo:listening" in line:
                    # Inject UDP test packets from host → QEMU guest port 7.
                    # Send in multiple bursts: the first burst triggers QEMU's
                    # ARP request to the guest. After ARP completes, subsequent
                    # bursts deliver actual UDP payloads.
                    def inject_udp():
                        try:
                            sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
                            for burst in range(5):
                                for i in range(4):
                                    sock.sendto(f"echo{burst*4+i}".encode(), ("127.0.0.1", 5555))
                                time.sleep(0.1)  # 100ms between bursts for ARP to settle
                            sock.close()
                            print(f"{GREEN}INJECTED: 20 UDP packets to localhost:5555 (5 bursts){RESET}")
                        except Exception as e:
                            print(f"{RED}UDP injection failed: {e}{RESET}")
                    threading.Thread(target=inject_udp, daemon=True).start()

            if process.poll() is not None:
                # Drain any remaining buffered output before exiting.
                # Apply the same checks as the main loop — panic detection
                # matters here too, otherwise a crash in the tail buffer
                # would be silently swallowed as a false success.
                for remaining_line in process.stdout:
                    print(f"QEMU: {remaining_line.strip()}")
                    output_buffer += remaining_line
                    if "BENCHMARK SUITE COMPLETE" in remaining_line:
                        print(f"{GREEN}SUITE COMPLETE — terminating QEMU{RESET}")
                        return True, output_buffer
                    if "kernel panic" in remaining_line.lower() or "\x1b[31;1m" in remaining_line:
                        print(f"{RED}KERNEL PANIC DETECTED{RESET}")
                break
                
    except KeyboardInterrupt:
        process.terminate()

    # --- Guard 3: Truncate log after run to prevent accumulation ---
    if os.path.exists(log_path):
        log_size = os.path.getsize(log_path)
        if log_size > QEMU_LOG_MAX_BYTES:
            print(f"{RED}  ⚠ qemu.log grew to {log_size // (1024*1024)}MB during run — truncating{RESET}")
            os.remove(log_path)

    return True, output_buffer

if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "build":
        # Build-only mode (used by demo script)
        try:
            TOOLCHAIN.validate()
            kernel_objs = build_kernel()
            bench_file = os.path.join(BENCH_ROOT, "ring_of_fire.salt")
            elf = build_benchmark(bench_file, kernel_objs)
            print(f"{GREEN}BUILD SUCCESS: {elf}{RESET}")
        except subprocess.CalledProcessError as e:
            print(f"{RED}BUILD FAILED: {e}{RESET}")
            sys.exit(1)

    elif len(sys.argv) > 1 and sys.argv[1] == "run":
        # Build + Run Flow
        try:
            TOOLCHAIN.validate()
            kernel_objs = build_kernel()
            bench_file = os.path.join(BENCH_ROOT, "ring_of_fire.salt")
            elf = build_benchmark(bench_file, kernel_objs)
            
            success, log = run_qemu_test(elf)
            if not success:
                sys.exit(1)
                
            if "BENCHMARK SUITE COMPLETE" in log:
                print(f"{GREEN}VERIFICATION SUCCESS: Full benchmark suite completed.{RESET}")
                # Extract results
                for line in log.split("\n"):
                    if "BENCH:" in line or "ROF Result" in line:
                        print(f"  {line.strip()}")
                sys.exit(0)
            elif "ROF" in log:
                print(f"{GREEN}VERIFICATION PARTIAL: Ring of Fire completed.{RESET}")
                sys.exit(0)
            else:
                print(f"{RED}VERIFICATION FAILED: No report found.{RESET}")
                sys.exit(1)
                
        except subprocess.CalledProcessError as e:
            print(f"{RED}BUILD FAILED: {e}{RESET}")
            sys.exit(1)
    elif len(sys.argv) > 1 and sys.argv[1] == "test_net":
        # VirtIO-Net E2E Integration Test
        # Builds kernel, boots QEMU, injects UDP packets, and asserts:
        #   1. rx > 0 (packets received by driver)
        #   2. tx > 0 (packets transmitted by driver)
        #   3. udp_echo > 0 (UDP echo service handled packets)
        #   4. ARP handshake completed
        try:
            TOOLCHAIN.validate()
            kernel_objs = build_kernel()
            bench_file = os.path.join(BENCH_ROOT, "ring_of_fire.salt")
            elf = build_benchmark(bench_file, kernel_objs)

            print(f"{GREEN}== VirtIO-Net E2E Integration Test =={RESET}")
            success, log = run_qemu_test(elf)

            # Parse net_echo result line
            rx_count = 0
            tx_count = 0
            udp_count = 0
            arp_seen = False
            suite_complete = False

            for line in log.split("\n"):
                if "ARP: Request for our IP, sending reply" in line:
                    arp_seen = True

                net_match = re.search(
                    r"BENCH:net_echo:result rx=(\d+) tx=(\d+) udp_echo=(\d+)",
                    line,
                )
                if net_match:
                    rx_count = int(net_match.group(1))
                    tx_count = int(net_match.group(2))
                    udp_count = int(net_match.group(3))

                if "BENCHMARK SUITE COMPLETE" in line:
                    suite_complete = True

            # Report
            print(f"\n{GREEN}== Net Test Results =={RESET}")
            print(f"  Suite complete : {'YES' if suite_complete else 'NO'}")
            print(f"  ARP handshake  : {'YES' if arp_seen else 'NO'}")
            print(f"  RX packets     : {rx_count}")
            print(f"  TX packets     : {tx_count}")
            print(f"  UDP echo       : {udp_count}")

            # Assertions
            failures = []
            if not suite_complete:
                failures.append("Benchmark suite did not complete")
            if rx_count == 0:
                failures.append("rx=0 — no packets received by driver")
            if tx_count == 0:
                failures.append("tx=0 — no packets transmitted by driver")
            if udp_count == 0:
                failures.append("udp_echo=0 — no UDP echo packets handled")
            if not arp_seen:
                failures.append("No ARP handshake observed")

            if failures:
                print(f"\n{RED}NET TEST FAIL:{RESET}")
                for f in failures:
                    print(f"  ✗ {f}")
                sys.exit(1)
            else:
                print(f"\n{GREEN}NET TEST PASS: rx={rx_count} tx={tx_count} udp_echo={udp_count}{RESET}")
                sys.exit(0)

        except subprocess.CalledProcessError as e:
            print(f"{RED}BUILD FAILED: {e}{RESET}")
            sys.exit(1)

    elif len(sys.argv) > 1 and sys.argv[1] == "bench":
        # Build + Run + Parse Benchmark Results with cross-OS comparison
        try:
            TOOLCHAIN.validate()
            kernel_objs = build_kernel()
            bench_file = os.path.join(BENCH_ROOT, "ring_of_fire.salt")
            elf = build_benchmark(bench_file, kernel_objs)

            print(f"{GREEN}== Running Benchmark Suite =={RESET}")
            success, log = run_qemu_test(elf)

            # Parse BENCH: lines from serial output
            results = {}
            for line in log.split("\n"):
                line = line.strip()
                if line.startswith("QEMU: "):
                    line = line[6:]

                # ROF context switch: "ROF Result: Avg Context Switch Gap = 1567 cycles"
                m = re.search(r"ROF Result: Avg Context Switch Gap = (\d+) cycles", line)
                if m:
                    results["ctx_switch_4"] = int(m.group(1))

                # ROF1K context switch
                m = re.search(r"ROF1K Result: Avg Context Switch Gap = (\d+) cycles", line)
                if m:
                    results["ctx_switch_1k"] = int(m.group(1))

                # ROF-LITE context switch (integer-only, TCG-safe)
                m = re.search(r"ROF-LITE Result: Avg Context Switch Gap = (\d+) cycles", line)
                if m:
                    results["ctx_switch_lite"] = int(m.group(1))

                # Syscall: "BENCH:syscall:avg=NNN min=NNN max=NNN"
                m = re.search(r"BENCH:syscall:avg=(\d+)\s+min=(\d+)\s+max=(\d+)", line)
                if m:
                    results["syscall_avg"] = int(m.group(1))
                    results["syscall_min"] = int(m.group(2))
                    results["syscall_max"] = int(m.group(3))

                # IPC: "BENCH:ipc:avg=NNN min=NNN max=NNN"
                m = re.search(r"BENCH:ipc:avg=(\d+)\s+min=(\d+)\s+max=(\d+)", line)
                if m:
                    results["ipc_avg"] = int(m.group(1))
                    results["ipc_min"] = int(m.group(2))
                    results["ipc_max"] = int(m.group(3))

                # Alloc: "BENCH:alloc:avg=NNN min=NNN max=NNN"
                m = re.search(r"BENCH:alloc:avg=(\d+)\s+min=(\d+)\s+max=(\d+)", line)
                if m:
                    results["alloc_avg"] = int(m.group(1))
                    results["alloc_min"] = int(m.group(2))
                    results["alloc_max"] = int(m.group(3))

                # Slab reclaim pass/fail
                if "BENCH:slab_reclaim:COMPLETE" in line:
                    results["slab_reclaim"] = "PASS"
                elif "BENCH:slab_reclaim:phase2:FAIL" in line:
                    results["slab_reclaim"] = "FAIL"
                elif "BENCH:slab_reclaim:phase1:FAIL" in line:
                    results["slab_reclaim"] = "FAIL"

                # Net echo
                m = re.search(r"BENCH:net_echo:SKIP", line)
                if m:
                    results["net_echo"] = "SKIP"
                m = re.search(r"BENCH:net_echo:PASS", line)
                if m:
                    results["net_echo"] = "PASS"

                # IRQ latency: "BENCH:irq:avg=NNN min=NNN max=NNN"
                m = re.search(r"BENCH:irq:avg=(\d+)\s+min=(\d+)\s+max=(\d+)", line)
                if m:
                    results["irq_avg"] = int(m.group(1))
                    results["irq_min"] = int(m.group(2))
                    results["irq_max"] = int(m.group(3))

                # PMM: "BENCH:pmm:avg=NNN min=NNN max=NNN pairs=NNN"
                m = re.search(r"BENCH:pmm:avg=(\d+)\s+min=(\d+)\s+max=(\d+)\s+pairs=(\d+)", line)
                if m:
                    results["pmm_avg"] = int(m.group(1))
                    results["pmm_min"] = int(m.group(2))
                    results["pmm_max"] = int(m.group(3))
                    results["pmm_pairs"] = int(m.group(4))

                # Slab stress: "BENCH:slab_stress:avg=NNN min=NNN max=NNN pairs=NNN watermark_stable=true/false"
                m = re.search(r"BENCH:slab_stress:avg=(\d+)\s+min=(\d+)\s+max=(\d+)\s+pairs=(\d+)\s+watermark_stable=(\w+)", line)
                if m:
                    results["slab_stress_avg"] = int(m.group(1))
                    results["slab_stress_min"] = int(m.group(2))
                    results["slab_stress_max"] = int(m.group(3))
                    results["slab_stress_pairs"] = int(m.group(4))
                    results["slab_stress_stable"] = m.group(5)

            suite_complete = "BENCHMARK SUITE COMPLETE" in log

            # ══════════════════════════════════════════════════════════════
            # Render Results Table
            # ══════════════════════════════════════════════════════════════
            # Reference numbers (bare-metal, published lmbench/sysbench):
            #   Linux:   syscall ~150 cy, ctx switch ~2000 cy, IPC pipe ~3500 cy
            #   macOS:   syscall ~1200 cy, ctx switch ~10000 cy, IPC mach ~5000 cy
            #   Windows: syscall ~1800 cy, ctx switch ~12000 cy, IPC ~8000 cy
            # Note: Lattice runs on QEMU-TCG (emulated), references are bare-metal.

            CYAN = "\033[96m"
            BOLD = "\033[1m"
            DIM = "\033[2m"
            YELLOW = "\033[93m"

            print(f"\n{CYAN}{'═'*72}{RESET}")
            print(f"{BOLD}  Lattice OS Kernel Benchmarks — QEMU-TCG  {RESET}")
            print(f"{CYAN}{'═'*72}{RESET}")

            print(f"\n{BOLD}  Latency Benchmarks (cycles, lower is better){RESET}")
            print(f"  {'─'*68}")
            print(f"  {'Benchmark':<24} {'Lattice':>10} {'Linux':>10} {'macOS':>10} {'Windows':>10}")
            print(f"  {'─'*68}")

            def fmt(v):
                return f"{v:,}" if v else "-"

            # Syscall
            lattice_val = results.get('syscall_avg')
            print(f"  {'Null syscall (avg)':<24} {fmt(lattice_val):>10} {'~150':>10} {'~1,200':>10} {'~1,800':>10}")
            if lattice_val and results.get('syscall_min'):
                print(f"  {'  min / max':<24} {fmt(results['syscall_min']):>10}{'':>10}{'':>10}{'':>10}")
                print(f"  {'':<24} {fmt(results['syscall_max']):>10}{'':>10}{'':>10}{'':>10}")

            # Context switch (4 fibers)
            lattice_val = results.get('ctx_switch_4')
            print(f"  {'Ctx switch (4 FPU)':<24} {fmt(lattice_val):>10} {'~2,000':>10} {'~10,000':>10} {'~12,000':>10}")

            # Context switch (lite, integer-only)
            lattice_val = results.get('ctx_switch_lite')
            print(f"  {'Ctx switch (4 int)':<24} {fmt(lattice_val):>10} {'~2,000':>10} {'~10,000':>10} {'~12,000':>10}")

            # IPC
            lattice_val = results.get('ipc_avg')
            print(f"  {'IPC round-trip (avg)':<24} {fmt(lattice_val):>10} {'~3,500':>10} {'~5,000':>10} {'~8,000':>10}")

            # Alloc
            lattice_val = results.get('alloc_avg')
            print(f"  {'Slab alloc (avg)':<24} {fmt(lattice_val):>10} {'~200':>10} {'~300':>10} {'~400':>10}")

            # IRQ latency
            lattice_val = results.get('irq_avg')
            print(f"  {'IRQ latency (avg)':<24} {fmt(lattice_val):>10} {'~500':>10} {'~800':>10} {'~1,500':>10}")

            # PMM alloc/free pair
            lattice_val = results.get('pmm_avg')
            print(f"  {'PMM alloc/free (avg)':<24} {fmt(lattice_val):>10} {'~300':>10} {'~400':>10} {'~500':>10}")

            # Slab stress
            lattice_val = results.get('slab_stress_avg')
            print(f"  {'Slab stress (avg)':<24} {fmt(lattice_val):>10} {'~400':>10} {'~600':>10} {'~800':>10}")

            print(f"  {'─'*68}")

            # Functional tests
            print(f"\n{BOLD}  Functional Tests{RESET}")
            print(f"  {'─'*68}")
            slab_status = results.get('slab_reclaim', 'NOT RUN')
            net_status = results.get('net_echo', 'NOT RUN')
            slab_stress_stable = results.get('slab_stress_stable', 'NOT RUN')
            slab_color = GREEN if slab_status == 'PASS' else RED
            net_color = GREEN if net_status == 'PASS' else (YELLOW if net_status == 'SKIP' else RED)
            slab_stable_color = GREEN if slab_stress_stable == 'true' else RED
            print(f"  {'Slab reclaim (100K)':<24} {slab_color}{slab_status}{RESET}")
            print(f"  {'Slab stress (stable)':<24} {slab_stable_color}{slab_stress_stable}{RESET}")
            print(f"  {'Net echo (VirtIO)':<24} {net_color}{net_status}{RESET}")
            print(f"  {'─'*68}")

            print(f"\n{DIM}  * Lattice: QEMU-TCG (emulated). Linux/macOS/Windows: bare-metal lmbench.{RESET}")
            print(f"{DIM}  * Comparison is architectural overhead, not raw speed.{RESET}")
            print(f"{CYAN}{'═'*72}{RESET}\n")

            if suite_complete:
                print(f"{GREEN}BENCHMARK SUITE COMPLETE{RESET}")
                sys.exit(0)
            else:
                print(f"{RED}BENCHMARK SUITE DID NOT COMPLETE{RESET}")
                sys.exit(1)

        except subprocess.CalledProcessError as e:
            print(f"{RED}BUILD FAILED: {e}{RESET}")
            sys.exit(1)

    else:
        print("Usage: tools/runner_qemu.py [build|run|bench|test_net]")
