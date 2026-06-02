import os
import subprocess
import time
import re
from dataclasses import dataclass

@dataclass
class BenchmarkMetrics:
    time_s: float
    peak_rss_kb: int
    binary_size_kb: float
    stdout: str = ""
    stderr: str = ""
    ret_code: int = 0

def get_binary_size_kb(path: str) -> float:
    if not path or not os.path.exists(path):
        return 0.0
    return os.path.getsize(path) / 1024.0

def run_and_collect(cmd: str, binary_path: str = None, iterations: int = 3, warmup: int = 1) -> BenchmarkMetrics:
    # Warmup
    for _ in range(warmup):
        subprocess.run(cmd, shell=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    
    times = []
    peak_rss = 0
    stdout = ""
    stderr = ""
    ret = 0
    
    for i in range(iterations):
        # /usr/bin/time -l is macOS specific for peak RSS
        time_cmd = f"/usr/bin/time -l {cmd}"
        start = time.perf_counter()
        res = subprocess.run(time_cmd, shell=True, capture_output=True, text=True)
        elapsed = time.perf_counter() - start
        times.append(elapsed)
        
        # parse peak rss from stderr
        rss_match = re.search(r'(\d+)\s+maximum resident set size', res.stderr)
        if rss_match:
            rss_kb = int(rss_match.group(1)) // 1024
            peak_rss = max(peak_rss, rss_kb)
            
        if i == iterations - 1: # save output of last run
            stdout = res.stdout
            stderr = res.stderr
            ret = res.returncode
            
    avg_time = sum(times) / len(times) if times else 0
    bin_size = get_binary_size_kb(binary_path) if binary_path else 0.0
    
    return BenchmarkMetrics(
        time_s=avg_time,
        peak_rss_kb=peak_rss,
        binary_size_kb=bin_size,
        stdout=stdout,
        stderr=stderr,
        ret_code=ret
    )
