import os
import glob
import subprocess
import time
from typing import Dict, Any, List
from ..runner import BenchmarkSuite, Telemetry

class MicroSuite(BenchmarkSuite):
    name = "Microbenchmarks (Algorithms & Data Structures)"

    def count_loc(self, path: str) -> int:
        # Simple line counter ignoring empty lines
        if not os.path.exists(path):
            return 0
        loc = 0
        with open(path, 'r', encoding='utf-8', errors='ignore') as f:
            for line in f:
                stripped = line.strip()
                if stripped and not stripped.startswith('//') and not stripped.startswith('#'):
                    loc += 1
        return loc

    def run(self):
        bin_dir = os.path.join(self.workspace_root, "benchmarks", "bin")
        os.makedirs(bin_dir, exist_ok=True)
        
        bench_dir = os.path.join(self.workspace_root, "benchmarks")
        salt_files = glob.glob(os.path.join(bench_dir, "*.salt"))
        
        # Build Salt compiler
        self.run_command(["cargo", "build", "--release"], cwd=os.path.join(self.workspace_root, "salt-front"))
        sf_bin = os.path.join(self.workspace_root, "salt-front", "target", "release", "salt-front")
        runtime_c = os.path.join(self.workspace_root, "salt-front", "runtime.c")

        for s_file in sorted(salt_files):
            name = os.path.splitext(os.path.basename(s_file))[0]
            print(f"\n--- Benchmarking Micro: {name} ---")
            
            # Paths
            c_src = os.path.join(bench_dir, f"{name}.c")
            rs_src = os.path.join(bench_dir, f"{name}.rs")
            salt_src = s_file
            
            c_bin = os.path.join(bin_dir, f"{name}_c")
            rs_bin = os.path.join(bin_dir, f"{name}_rs")
            salt_bin = os.path.join(bin_dir, f"{name}_salt")

            targets = []

            # Compile C
            if os.path.exists(c_src):
                try:
                    self.run_command(["clang", "-O3", "-march=native", "-ffast-math", c_src, "-o", c_bin])
                    targets.append(("C", c_bin, c_src))
                except Exception as e:
                    print(f"Failed to compile C for {name}")

            # Compile Rust
            if os.path.exists(rs_src):
                try:
                    self.run_command(["rustc", "-C", "opt-level=3", rs_src, "-o", rs_bin])
                    targets.append(("Rust", rs_bin, rs_src))
                except Exception as e:
                    print(f"Failed to compile Rust for {name}")

            # Compile Salt
            try:
                # Same MLIR compilation steps from benchmark.sh
                mlir_clean = os.path.join(bin_dir, f"{name}_clean.mlir")
                with open(mlir_clean, "w") as f:
                    subprocess.run([sf_bin, salt_src, "--release"], stdout=f, cwd=os.path.join(self.workspace_root, "salt-front"), check=True)
                
                mlir_opt = os.path.join(bin_dir, f"{name}.opt.mlir")
                self.run_command([
                    "mlir-opt", "--convert-linalg-to-loops", "--expand-strided-metadata",
                    "--affine-loop-tile=tile-size=4", "--lower-affine", "--convert-scf-to-cf",
                    "--canonicalize", "--sroa", "--mem2reg", "--canonicalize",
                    "--finalize-memref-to-llvm", "--convert-arith-to-llvm", "--convert-math-to-llvm",
                    "--convert-func-to-llvm", "--convert-cf-to-llvm", "--reconcile-unrealized-casts",
                    mlir_clean, "-o", mlir_opt
                ])
                
                ll_file = os.path.join(bin_dir, f"{name}.ll")
                self.run_command(["mlir-translate", "--mlir-to-llvmir", mlir_opt, "-o", ll_file])
                
                ll_opt_file = os.path.join(bin_dir, f"{name}_opt.ll")
                self.run_command(["opt", "-O3", ll_file, "-S", "-o", ll_opt_file])

                bridge_c = os.path.join(bench_dir, f"{name}_bridge.c")
                if os.path.exists(bridge_c):
                    self.run_command(["clang", "-O3", ll_opt_file, bridge_c, runtime_c, "-o", salt_bin])
                else:
                    self.run_command(["clang", "-O3", ll_opt_file, runtime_c, "-o", salt_bin])

                targets.append(("Salt", salt_bin, salt_src))
            except Exception as e:
                print(f"Failed to compile Salt for {name}: {e}")

            # Run targets
            for t_lang, t_bin, t_src in targets:
                t_name = f"{name} ({t_lang})"
                telemetry = Telemetry()
                telemetry.binary_size_bytes = self.get_binary_size(t_bin)
                
                # We can't use wrk, we just measure execution time.
                # Average of 3 runs.
                for i in range(3):
                    time.sleep(0.1)
                    start = time.perf_counter()
                    
                    try:
                        # Capture output so we can get peak rss via ps/time if needed, but python's time is fine for exec time.
                        proc = subprocess.Popen([t_bin], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                        proc.wait()
                        elapsed = time.perf_counter() - start
                        
                        # Use a trick to get peak RSS: we can't easily poll, so we could use /usr/bin/time -l
                        time_out = subprocess.check_output(["/usr/bin/time", "-l", t_bin], stderr=subprocess.STDOUT, text=True)
                        import re
                        rss_match = re.search(r"(\d+)\s+maximum resident set size", time_out)
                        rss_kb = int(rss_match.group(1)) // 1024 if rss_match else 0
                        
                        metrics = {"execution_time_s": elapsed}
                        telemetry.add_iteration(metrics)
                        if rss_kb > telemetry.peak_rss_kb:
                            telemetry.peak_rss_kb = rss_kb
                            
                    except Exception as e:
                        print(f"Failed to run {t_name}")
                        
                self.results[t_name] = telemetry
