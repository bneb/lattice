#!/usr/bin/env python3
import os
import sys

import argparse

# Ensure LLVM tools are in PATH for macOS homebrew installations
llvm_paths = ["/opt/homebrew/opt/llvm/bin", "/usr/local/opt/llvm/bin"]
for p in llvm_paths:
    if os.path.isdir(p) and p not in os.environ.get("PATH", ""):
        os.environ["PATH"] = p + os.pathsep + os.environ.get("PATH", "")

from benchmarks.e2e.runner import BenchmarkRunner
from benchmarks.e2e.suites.micro_bench import MicroSuite
from benchmarks.e2e.suites.lettuce_bench import LettuceSuite
from benchmarks.e2e.suites.http_bench import HttpSuite
from benchmarks.e2e.suites.c10m_bench import C10MSuite
from benchmarks.e2e.suites.basalt_bench import BasaltSuite

def main():
    parser = argparse.ArgumentParser(description="KeuOS E2E Benchmark Runner")
    parser.add_argument("--suite", action="append", help="Specific suite to run (micro, lettuce, http, c10m, basalt). Can be specified multiple times. If omitted, runs all suites.")
    args = parser.parse_args()

    workspace_root = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
    os.chdir(workspace_root)
    
    runner = BenchmarkRunner(workspace_root)
    
    # Map friendly names to suite classes
    available_suites = {
        "micro": MicroSuite,
        "lettuce": LettuceSuite,
        "http": HttpSuite,
        "c10m": C10MSuite,
        "basalt": BasaltSuite
    }
    
    suites_to_run = args.suite if args.suite else available_suites.keys()
    
    for name in suites_to_run:
        if name in available_suites:
            runner.add_suite(available_suites[name](workspace_root))
        else:
            print(f"Warning: Unknown suite '{name}'")
            
    if not runner.suites:
        print("No valid suites selected. Exiting.")
        sys.exit(1)
    
    try:
        runner.run_all()
    except KeyboardInterrupt:
        print("\nBenchmark run aborted by user.")
        sys.exit(1)
        
if __name__ == "__main__":
    main()
