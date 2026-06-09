#!/usr/bin/env python3
"""
Unified E2E Benchmark Harness for Salt / KeuOS

This framework automates the execution of rigorous, high-fidelity end-to-end macro benchmarks
across the Salt project (HTTP Server, TCP Echo, Lettuce, Basalt).
It collects statistically significant telemetry (throughput, latency, memory, binary size)
and outputs a unified Markdown report and JSON artifact.
"""

import os
import subprocess
import time
import json
import statistics
import sys
import signal
from typing import List, Dict, Any, Optional

class Telemetry:
    def __init__(self):
        self.iterations: List[Dict[str, float]] = []
        self.binary_size_bytes: int = 0
        self.peak_rss_kb: int = 0
        
    def add_iteration(self, metrics: Dict[str, float]):
        self.iterations.append(metrics)
        
    def get_aggregated(self) -> Dict[str, Any]:
        if not self.iterations:
            return {}
        
        agg = {
            "binary_size_bytes": self.binary_size_bytes,
            "peak_rss_kb": self.peak_rss_kb
        }
        
        # Aggregate all float metrics across iterations
        keys = self.iterations[0].keys()
        for key in keys:
            values = [i[key] for i in self.iterations if key in i]
            if values:
                agg[f"{key}_mean"] = statistics.mean(values)
                if len(values) > 1:
                    agg[f"{key}_stdev"] = statistics.stdev(values)
                    agg[f"{key}_variance"] = statistics.variance(values)
                    agg[f"{key}_min"] = min(values)
                    agg[f"{key}_max"] = max(values)
                    sorted_vals = sorted(values)
                    idx = int(0.99 * len(sorted_vals))
                    if idx == len(sorted_vals): idx -= 1
                    agg[f"{key}_p99"] = sorted_vals[idx]
                else:
                    agg[f"{key}_stdev"] = 0.0
                    agg[f"{key}_variance"] = 0.0
                    agg[f"{key}_min"] = values[0]
                    agg[f"{key}_max"] = values[0]
                    agg[f"{key}_p99"] = values[0]
                    
        return agg

class BenchmarkSuite:
    name = "Unnamed Suite"
    
    def __init__(self, workspace_root: str):
        self.workspace_root = workspace_root
        self.results: Dict[str, Telemetry] = {}
        
    def run_command(self, cmd: List[str], cwd: Optional[str] = None) -> str:
        cwd = cwd or self.workspace_root
        try:
            result = subprocess.run(cmd, cwd=cwd, check=True, capture_output=True, text=True)
            return result.stdout
        except subprocess.CalledProcessError as e:
            print(f"Command failed: {' '.join(cmd)}")
            print(f"Stdout: {e.stdout}")
            print(f"Stderr: {e.stderr}")
            raise
            
    def get_binary_size(self, path: str) -> int:
        full_path = os.path.join(self.workspace_root, path)
        if os.path.exists(full_path):
            return os.path.getsize(full_path)
        return 0
        
    def get_process_rss(self, pid: int) -> int:
        """Returns peak RSS in KB for the given PID."""
        try:
            out = subprocess.check_output(["ps", "-o", "rss=", "-p", str(pid)], text=True)
            return int(out.strip())
        except (subprocess.CalledProcessError, ValueError):
            return 0
            
    def run(self):
        """Override to implement suite execution."""
        raise NotImplementedError()

    def report(self) -> Dict[str, Any]:
        """Returns aggregated telemetry for all targets in this suite."""
        return {target: t.get_aggregated() for target, t in self.results.items()}


class BenchmarkRunner:
    def __init__(self, workspace_root: str):
        self.workspace_root = workspace_root
        self.suites: List[BenchmarkSuite] = []
        
    def add_suite(self, suite: BenchmarkSuite):
        self.suites.append(suite)
        
    def run_all(self):
        all_results = {}
        
        # 1. Build the compiler once if needed
        print("Ensuring Salt compiler is built...")
        subprocess.run(
            ["cargo", "build", "--release"], 
            cwd=os.path.join(self.workspace_root, "salt-front"), 
            check=True,
            stdout=subprocess.DEVNULL
        )
        
        for suite in self.suites:
            print(f"\n======================================")
            print(f" Running Suite: {suite.name}")
            print(f"======================================")
            suite.run()
            all_results[suite.name] = suite.report()
            
        self.generate_json(all_results)
        self.generate_markdown(all_results)
        
    def generate_json(self, results: Dict[str, Any]):
        out_path = os.path.join(self.workspace_root, "benchmarks", "e2e_results.json")
        with open(out_path, "w") as f:
            json.dump(results, f, indent=2)
        print(f"\nGenerated JSON report: {out_path}")
            
    def generate_markdown(self, results: Dict[str, Any]):
        out_path = os.path.join(self.workspace_root, "benchmarks", "BENCHMARKS_E2E.md")
        with open(out_path, "w") as f:
            f.write("# Rigorous E2E Benchmarks\n\n")
            f.write("Automated, multi-iteration, high-fidelity benchmarks across the KeuOS macro-applications.\n\n")
            
            for suite_name, targets in results.items():
                f.write(f"## {suite_name}\n\n")
                if not targets:
                    f.write("No targets ran successfully.\n\n")
                    continue
                
                # Dynamic columns based on metrics
                metrics_set = set()
                for t_name, metrics in targets.items():
                    metrics_set.update(metrics.keys())
                    
                # We specifically want to highlight Throughput and Latency, then Mem/Size
                # Filter down to the _mean values
                primary_metrics = [m for m in metrics_set if m.endswith("_mean")]
                
                # Write table header
                header = ["Target", "Binary Size (KB)", "Peak RSS (KB)"]
                for m in primary_metrics:
                    header.append(m.replace("_mean", ""))
                    
                f.write("| " + " | ".join(header) + " |\n")
                f.write("|" + "|".join(["---"] * len(header)) + "|\n")
                
                for t_name, metrics in targets.items():
                    row = [f"**{t_name}**"]
                    row.append(f"{metrics.get('binary_size_bytes', 0) / 1024:.1f}")
                    row.append(f"{metrics.get('peak_rss_kb', 0)}")
                    
                    for m in primary_metrics:
                        val = metrics.get(m, 0.0)
                        row.append(f"{val:,.2f}")
                        
                    f.write("| " + " | ".join(row) + " |\n")
                f.write("\n")
                
        print(f"Generated Markdown report: {out_path}")

if __name__ == "__main__":
    print("This is the base module. Please run one of the suite scripts or main runner.")
