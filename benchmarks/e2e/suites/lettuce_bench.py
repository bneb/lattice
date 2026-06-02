import os
import re
import time
import subprocess
from typing import Dict
from ..runner import BenchmarkSuite, Telemetry

class LettuceSuite(BenchmarkSuite):
    name = "Lettuce (Redis-Compatible Data Store)"
    
    def run(self):
        # 1. Compile Salt version
        print("Compiling Lettuce (Salt)...")
        script_path = os.path.join(self.workspace_root, "scripts", "run_test.sh")
        salt_file = os.path.join(self.workspace_root, "lettuce", "src", "server.salt")
        
        self.run_command([script_path, salt_file, "--compile-only"])
        
        # 2. Setup baseline (Redis C)
        salt_bin = "/tmp/salt_build/server"
        targets = {
            "Salt (MLIR/LLVM)": {"cmd": [salt_bin], "bin_path": salt_bin}
        }
        
        # Try to find redis-server for baseline
        try:
            redis_bin = subprocess.check_output(["which", "redis-server"], text=True).strip()
            targets["C (Redis 7.x Baseline)"] = {"cmd": [redis_bin, "--save", "", "--appendonly", "no"], "bin_path": redis_bin}
        except subprocess.CalledProcessError:
            print("WARNING: redis-server not found, skipping C baseline.")
            
        for t_name, t_info in targets.items():
            print(f"\n--- Benchmarking {t_name} ---")
            telemetry = Telemetry()
            telemetry.binary_size_bytes = self.get_binary_size(t_info["bin_path"])
            
            # Start server
            env = os.environ.copy()
            env["DYLD_LIBRARY_PATH"] = "/opt/homebrew/lib"
            
            # Ensure port is clear
            subprocess.run("lsof -ti:6379 | xargs kill -9 2>/dev/null || true", shell=True)
            time.sleep(0.5)
            
            proc = subprocess.Popen(t_info["cmd"], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            try:
                # Wait for readiness
                ready = False
                for _ in range(20):
                    if subprocess.run(["redis-cli", "PING"], capture_output=True).returncode == 0:
                        ready = True
                        break
                    time.sleep(0.25)
                    
                if not ready:
                    print(f"Server {t_name} failed to start.")
                    continue
                    
                # Run iterations
                # 1 Warmup
                print("Warming up...")
                self.run_command(["redis-benchmark", "-p", "6379", "-t", "ping,set,get", "-c", "50", "-n", "10000", "-q"])
                
                # 3 measured iterations
                for i in range(3):
                    print(f"Iteration {i+1}/3...")
                    out = self.run_command(["redis-benchmark", "-p", "6379", "-t", "ping,set,get", "-c", "50", "-n", "100000", "-q"])
                    metrics = self.parse_redis_benchmark(out)
                    telemetry.add_iteration(metrics)
                    
                telemetry.peak_rss_kb = self.get_process_rss(proc.pid)
                self.results[t_name] = telemetry
                
            finally:
                proc.terminate()
                proc.wait()
                subprocess.run("lsof -ti:6379 | xargs kill -9 2>/dev/null || true", shell=True)
                
    def parse_redis_benchmark(self, output: str) -> Dict[str, float]:
        metrics = {}
        # Expected format: "GET: 233644.86 requests per second"
        for line in output.strip().split("\n"):
            match = re.match(r"^([A-Z_]+):\s+([0-9.]+)\s+requests per second", line)
            if match:
                cmd = match.group(1)
                rps = float(match.group(2))
                metrics[f"{cmd}_throughput_rps"] = rps
        return metrics
