import os
import re
import subprocess
from typing import Dict
from ..runner import BenchmarkSuite, Telemetry

class BasaltSuite(BenchmarkSuite):
    name = "Basalt (LLM Inference)"
    
    def run(self):
        print("Compiling Basalt LLM Inference Engine...")
        script_path = os.path.join(self.workspace_root, "scripts", "build_basalt.sh")
        self.run_command([script_path, "--build-only"])
        salt_bin = "/tmp/salt_build/basalt"
        
        print("Compiling llama2.c Baseline...")
        c_src = os.path.join(self.workspace_root, ".bench_basalt", "llama2.c", "run.c")
        c_bin = "/tmp/llama2c"
        self.run_command(["clang", "-O3", "-ffast-math", "-march=native", c_src, "-o", c_bin, "-lm"])
        
        model = os.path.join(self.workspace_root, ".bench_basalt", "models", "stories15M.bin")
        tok = os.path.join(self.workspace_root, ".bench_basalt", "models", "tokenizer.bin")
        
        targets = {
            "Salt (MLIR/LLVM, Basalt)": {"cmd": [salt_bin, model, tok], "bin_path": salt_bin, "parser": self.parse_salt},
            "C (clang -O3 -ffast-math, llama2.c)": {"cmd": [c_bin, model, "-z", tok, "-n", "256"], "bin_path": c_bin, "parser": self.parse_c}
        }
        
        for t_name, t_info in targets.items():
            print(f"\n--- Benchmarking {t_name} ---")
            telemetry = Telemetry()
            telemetry.binary_size_bytes = self.get_binary_size(t_info["bin_path"])
            
            # Warmup
            print("Warming up (discarded run)...")
            try:
                self.run_command(t_info["cmd"])
            except subprocess.CalledProcessError as e:
                print(f"Warmup failed: {e}")
                continue
            
            # 5 iterations
            for i in range(5):
                print(f"Iteration {i+1}/5...")
                out = self.run_command(t_info["cmd"])
                metrics = t_info["parser"](out)
                telemetry.add_iteration(metrics)
                
            # RSS is hard to get during execution for a fast-terminating binary,
            # so we'll leave it at 0 unless we wrap it in a profiler.
            self.results[t_name] = telemetry
            
    def parse_c(self, output: str) -> Dict[str, float]:
        metrics = {}
        # C output: achieved tok/s: 877.123
        match = re.search(r"achieved tok/s:\s+([0-9.]+)", output)
        if match:
            metrics["throughput_toks"] = float(match.group(1))
        return metrics
        
    def parse_salt(self, output: str) -> Dict[str, float]:
        metrics = {}
        # Salt output: tok/s: 870.45
        match = re.search(r"tok/s:\s+([0-9.]+)", output)
        if match:
            metrics["throughput_toks"] = float(match.group(1))
        return metrics
