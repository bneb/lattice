import sys
import os
sys.path.append(os.getcwd())
from tools.bench_infra.runners import SaltRunner
runner = SaltRunner(os.getcwd())
metrics = runner.run("benchmarks/hashmap_bench.salt", iterations=1, warmup=0)
print(f"Ret code: {metrics.ret_code}")
print(f"Stderr: {metrics.stderr}")
