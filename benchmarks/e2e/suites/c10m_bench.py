import os
import re
import time
import subprocess
import shutil
from typing import Dict
from ..runner import BenchmarkSuite, Telemetry

class C10MSuite(BenchmarkSuite):
    name = "C10M TCP Echo (Pulse Cannon)"
    
    def run(self):
        # 1. Compile Pulse Cannon
        print("Compiling Pulse Cannon load generator...")
        cannon_src = os.path.join(self.workspace_root, "benchmarks", "c10m", "stress_echo.c")
        cannon_bin = "/tmp/pulse_cannon"
        self.run_command(["clang", "-O3", cannon_src, "-o", cannon_bin])
        
        # 2. Compile Targets
        print("Compiling C10M Echo Servers...")
        c_src = os.path.join(self.workspace_root, "benchmarks", "c10m", "echo_c.c")
        c_bin = "/tmp/echo_c"
        self.run_command(["clang", "-O3", c_src, "-o", c_bin])
        
        # Compile Rust via Cargo
        rust_proj_dir = "/tmp/echo_rust_proj"
        os.makedirs(os.path.join(rust_proj_dir, "src"), exist_ok=True)
        shutil.copy(
            os.path.join(self.workspace_root, "benchmarks", "c10m", "echo_rust.rs"),
            os.path.join(rust_proj_dir, "src", "main.rs")
        )
        with open(os.path.join(rust_proj_dir, "Cargo.toml"), "w") as f:
            f.write("""[package]
name = "echo_rust"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1", features = ["full"] }

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
""")
        self.run_command(["cargo", "build", "--release"], cwd=rust_proj_dir)
        rust_bin = os.path.join(rust_proj_dir, "target", "release", "echo_rust")
        
        # Compile Salt
        script_path = os.path.join(self.workspace_root, "scripts", "run_test.sh")
        salt_file = os.path.join(self.workspace_root, "benchmarks", "c10m", "echo_salt.salt")
        bridge_file = os.path.join(self.workspace_root, "benchmarks", "c10m", "echo_salt_bridge.c")
        self.run_command([script_path, salt_file, "--bridge", bridge_file, "--compile-only"])
        salt_bin = "/tmp/salt_build/echo_salt"
        
        targets = {
            "Salt (MLIR/LLVM, kqueue)": {"cmd": [salt_bin, "9000"], "bin_path": salt_bin},
            "C (clang -O3, kqueue)": {"cmd": [c_bin, "9000"], "bin_path": c_bin},
            "Rust (Tokio async)": {"cmd": [rust_bin, "9000"], "bin_path": rust_bin}
        }
        
        for t_name, t_info in targets.items():
            print(f"\n--- Benchmarking {t_name} ---")
            telemetry = Telemetry()
            telemetry.binary_size_bytes = self.get_binary_size(t_info["bin_path"])
            
            subprocess.run("lsof -ti:9000 | xargs kill -9 2>/dev/null || true", shell=True)
            time.sleep(0.5)
            
            proc = subprocess.Popen(t_info["cmd"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            try:
                # Wait for readiness
                time.sleep(1)
                
                # Warmup
                print("Warming up...")
                self.run_command([cannon_bin, "127.0.0.1", "9000", "50", "10000"])
                
                # 3 iterations
                for i in range(3):
                    print(f"Iteration {i+1}/3...")
                    out = self.run_command([cannon_bin, "127.0.0.1", "9000", "100", "100000"])
                    metrics = self.parse_cannon(out)
                    telemetry.add_iteration(metrics)
                    
                telemetry.peak_rss_kb = self.get_process_rss(proc.pid)
                self.results[t_name] = telemetry
                
            finally:
                proc.terminate()
                proc.wait()
                subprocess.run("lsof -ti:9000 | xargs kill -9 2>/dev/null || true", shell=True)

    def parse_cannon(self, output: str) -> Dict[str, float]:
        metrics = {}
        t_match = re.search(r"Throughput:\s+([0-9]+)\s+packets/sec", output)
        if t_match:
            metrics["throughput_rps"] = float(t_match.group(1))
            
        l_match = re.search(r"Avg Latency:\s+([0-9.]+)\s+µs", output)
        if l_match:
            metrics["latency_avg_us"] = float(l_match.group(1))
            
        return metrics
