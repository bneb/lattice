import os
import re
import time
import subprocess
from typing import Dict
from ..runner import BenchmarkSuite, Telemetry

class HttpSuite(BenchmarkSuite):
    name = "HTTP Server (Request Routing)"
    
    def run(self):
        # 1. Compile targets
        print("Compiling HTTP servers...")
        script_path = os.path.join(self.workspace_root, "scripts", "run_test.sh")
        salt_file = os.path.join(self.workspace_root, "examples", "http_server.salt")
        self.run_command([script_path, salt_file, "--compile-only"])
        salt_bin = "/tmp/salt_build/http_server"
        
        c_src = os.path.join(self.workspace_root, "benchmarks", "c_bench_server.c")
        c_bin = "/tmp/http_server_c"
        self.run_command(["clang", "-O3", "-march=native", c_src, "-o", c_bin])
        
        node_script = os.path.join(self.workspace_root, "benchmarks", "node_http_server.js")
        
        targets = {
            "Salt (MLIR/LLVM, dynamic)": {"cmd": [salt_bin], "bin_path": salt_bin},
            "C (clang -O3, static)": {"cmd": [c_bin], "bin_path": c_bin},
            "Node.js (http module)": {"cmd": ["node", node_script], "bin_path": None}
        }
        
        for t_name, t_info in targets.items():
            print(f"\n--- Benchmarking {t_name} ---")
            telemetry = Telemetry()
            if t_info["bin_path"]:
                telemetry.binary_size_bytes = self.get_binary_size(t_info["bin_path"])
                
            # Start server
            subprocess.run("lsof -ti:8080 | xargs kill -9 2>/dev/null || true", shell=True)
            time.sleep(0.5)
            
            proc = subprocess.Popen(t_info["cmd"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            try:
                # Wait for readiness
                ready = False
                for _ in range(20):
                    if subprocess.run(["curl", "-s", "http://localhost:8080/health"], capture_output=True).returncode == 0:
                        ready = True
                        break
                    time.sleep(0.25)
                    
                if not ready:
                    print(f"Server {t_name} failed to start.")
                    continue
                    
                # Warmup
                print("Warming up...")
                self.run_command(["wrk", "-t2", "-c100", "-d2s", "http://localhost:8080/health"])
                
                # 3 iterations
                for i in range(3):
                    print(f"Iteration {i+1}/3...")
                    out = self.run_command(["wrk", "-t2", "-c100", "-d10s", "http://localhost:8080/health"])
                    metrics = self.parse_wrk(out)
                    telemetry.add_iteration(metrics)
                    
                telemetry.peak_rss_kb = self.get_process_rss(proc.pid)
                self.results[t_name] = telemetry
                
            finally:
                proc.terminate()
                proc.wait()
                subprocess.run("lsof -ti:8080 | xargs kill -9 2>/dev/null || true", shell=True)

    def parse_wrk(self, output: str) -> Dict[str, float]:
        metrics = {}
        # Parse Requests/sec
        req_match = re.search(r"Requests/sec:\s+([0-9.]+)", output)
        if req_match:
            metrics["throughput_rps"] = float(req_match.group(1))
            
        # Parse Latency (Avg)
        lat_match = re.search(r"Latency\s+([0-9.]+)(us|ms|s)", output)
        if lat_match:
            val = float(lat_match.group(1))
            unit = lat_match.group(2)
            if unit == "ms":
                val *= 1000.0
            elif unit == "s":
                val *= 1000000.0
            metrics["latency_avg_us"] = val
            
        return metrics
