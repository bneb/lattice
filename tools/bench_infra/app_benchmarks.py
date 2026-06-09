import subprocess
import os
import re

def run_ml_benchmark(workspace_root):
    print("Running ML Benchmark...")
    script_path = os.path.join(workspace_root, "benchmarks", "ml", "benchmark.sh")
    res = subprocess.run([script_path, "--all"], capture_output=True, text=True, env=os.environ.copy())
    if res.returncode != 0:
        return res.stdout + "\nML Benchmark Failed:\n" + res.stderr
    return res.stdout

def run_echo_benchmark(workspace_root):
    print("Running TCP Echo Benchmark...")
    script_path = os.path.join(workspace_root, "benchmarks", "c10m", "benchmark_echo.sh")
    if not os.path.exists(script_path):
        print(f"Skipping TCP Echo Benchmark: {script_path} not found.")
        return "N/A"
    # Use small duration and conns for standard run, e.g. 9000 2 100
    res = subprocess.run([script_path, "9000", "2", "100"], capture_output=True, text=True, env=os.environ.copy())
    if res.returncode != 0:
        return res.stdout + "\nTCP Echo Benchmark Failed:\n" + res.stderr
    return res.stdout

def parse_echo_results(stdout):
    results = {}
    current_target = None
    for line in stdout.split('\n'):
        if "Benchmarking:" in line:
            if "C / kqueue" in line: current_target = "C"
            elif "Rust / Tokio" in line: current_target = "Rust"
            elif "Salt / kqueue" in line: current_target = "Salt"
        
        if current_target and "Rate:" in line:
            rate_match = re.search(r'Rate:\s+(\d+)\s+conn/s', line)
            if rate_match:
                results[current_target] = rate_match.group(1)
                current_target = None
    return results

def append_app_reports(markdown_path, ml_output, echo_output):
    echo_results = parse_echo_results(echo_output)
    
    with open(markdown_path, "a") as f:
        f.write("\n\n## Application Performance (TCP Echo)\n\n")
        f.write("| Implementation | Rate (conn/s) |\n")
        f.write("|---|---|\n")
        for lang in ["C", "Rust", "Salt"]:
            rate = echo_results.get(lang, "N/A")
            f.write(f"| {lang} | {rate} |\n")
        
        f.write("\n\n## Application Performance (ML Training)\n\n")
        f.write("```\n")
        # Keep only the result lines to avoid dumping the whole build log
        lines = ml_output.split('\n')
        for line in lines[-20:]: # Just dump the end of the log
            f.write(line + "\n")
        f.write("```\n")
