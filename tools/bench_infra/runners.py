import os
import subprocess
from .collectors import run_and_collect, BenchmarkMetrics

class BaseRunner:
    def __init__(self, workspace_root: str):
        self.workspace_root = workspace_root
        
    def compile(self, source_path: str) -> str:
        """Compiles the source and returns the binary path. Returns None if it fails."""
        return None
        
    def get_run_cmd(self, binary_path: str) -> str:
        return binary_path

    def run(self, source_path: str, iterations: int = 3, warmup: int = 1) -> BenchmarkMetrics:
        bin_path = self.compile(source_path)
        if bin_path is None or not os.path.exists(bin_path):
            return BenchmarkMetrics(0, 0, 0, ret_code=-1, stderr="Compilation failed or binary not found.")
        cmd = self.get_run_cmd(bin_path)
        return run_and_collect(cmd, bin_path, iterations, warmup)

class CRunner(BaseRunner):
    def compile(self, source_path: str) -> str:
        if not os.path.exists(source_path): return None
        basename = os.path.splitext(os.path.basename(source_path))[0]
        bin_path = f"/tmp/{basename}_c"
        cmd = f"clang -O3 -ffast-math -march=native {source_path} -o {bin_path}"
        res = subprocess.run(cmd, shell=True, capture_output=True)
        return bin_path if res.returncode == 0 else None

class RustRunner(BaseRunner):
    def compile(self, source_path: str) -> str:
        if not os.path.exists(source_path): return None
        basename = os.path.splitext(os.path.basename(source_path))[0]
        bin_path = f"/tmp/{basename}_rs"
        cmd = f"rustc -C opt-level=3 -C target-cpu=native {source_path} -o {bin_path}"
        res = subprocess.run(cmd, shell=True, capture_output=True)
        return bin_path if res.returncode == 0 else None

class SaltRunner(BaseRunner):
    def compile(self, source_path: str) -> str:
        if not os.path.exists(source_path): return None
        basename = os.path.splitext(os.path.basename(source_path))[0]
        bin_path = f"/tmp/salt_build/{basename}"
        script_path = os.path.join(self.workspace_root, "scripts", "run_test.sh")
        cmd = ["zsh", script_path, "--benchmark", "--compile-only", source_path]
        res = subprocess.run(cmd, capture_output=True)
        if res.returncode != 0:
            print(f"  [SALT] compile failed: {res.stderr.decode()[:200]}")
            return None
        return bin_path

class NodeRunner(BaseRunner):
    def compile(self, source_path: str) -> str:
        # Node does not compile to binary, just return source_path
        if not os.path.exists(source_path): return None
        return source_path
        
    def get_run_cmd(self, binary_path: str) -> str:
        return f"node {binary_path}"

class PyTorchRunner(BaseRunner):
    def compile(self, source_path: str) -> str:
        if not os.path.exists(source_path): return None
        return source_path

    def get_run_cmd(self, binary_path: str) -> str:
        # Use venv if it exists
        venv_python = os.path.join(os.path.dirname(binary_path), ".venv", "bin", "python3")
        if os.path.exists(venv_python):
            return f"{venv_python} {binary_path}"
        return f"python3 {binary_path}"
