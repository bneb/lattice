#!/usr/bin/env python3
"""
KeuOS Docker Build Runner — Reproducible Linux builds via Docker.

Usage:
    python3 tools/docker_build.py build       # Build salt-front + salt-opt in Docker
    python3 tools/docker_build.py test        # Build + run Rust tests (salt-front, salt-lsp)
    python3 tools/docker_build.py image       # Build/rebuild the Docker image only
    python3 tools/docker_build.py shell       # Drop into interactive Docker shell
    python3 tools/docker_build.py status      # Check if Docker image exists and is up to date

Requires Docker to be installed and running.
"""

import subprocess
import sys
import os
import hashlib
import time

# ─── Configuration ────────────────────────────────────────────────────────────

IMAGE_NAME = "keuos-dev"
WORKSPACE_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# MLIR/LLVM paths inside the Docker container (Ubuntu 24.04, LLVM 21)
MLIR_DIR = "/usr/lib/llvm-21/lib/cmake/mlir"
LLVM_DIR = "/usr/lib/llvm-21/lib/cmake/llvm"

# ANSI colors
RED = "\033[91m"
GREEN = "\033[92m"
CYAN = "\033[96m"
YELLOW = "\033[93m"
RESET = "\033[0m"


def _run(cmd, check=True, capture=False, **kwargs):
    """Run a command with logging."""
    print(f"{CYAN}$ {' '.join(cmd) if isinstance(cmd, list) else cmd}{RESET}")
    if capture:
        result = subprocess.run(cmd, capture_output=True, text=True, **kwargs)
        if check and result.returncode != 0:
            print(f"{RED}FAILED (exit {result.returncode}){RESET}")
            print(result.stderr)
            sys.exit(1)
        return result
    else:
        result = subprocess.run(cmd, **kwargs)
        if check and result.returncode != 0:
            print(f"{RED}FAILED (exit {result.returncode}){RESET}")
            sys.exit(1)
        return result


def _docker_run(script, interactive=False):
    """Run a bash script inside the keuos-dev Docker container.

    Mounts the workspace at /workspace. Uses --rm for automatic cleanup.
    """
    cmd = [
        "docker", "run", "--rm",
        "-v", f"{WORKSPACE_ROOT}:/workspace",
        "-w", "/workspace",
    ]
    if interactive:
        cmd += ["-it"]
    cmd += [IMAGE_NAME, "bash", "-c", script]
    return _run(cmd, check=True)


def _image_exists():
    """Check if the keuos-dev Docker image exists."""
    result = subprocess.run(
        ["docker", "image", "inspect", IMAGE_NAME],
        capture_output=True
    )
    return result.returncode == 0


# ─── Commands ─────────────────────────────────────────────────────────────────

def cmd_image():
    """Build or rebuild the Docker image from the project Dockerfile."""
    print(f"{GREEN}═══ Building Docker Image: {IMAGE_NAME} ═══{RESET}")
    _run(["docker", "build", "-t", IMAGE_NAME, "."], cwd=WORKSPACE_ROOT)
    print(f"{GREEN}═══ Docker image '{IMAGE_NAME}' ready ═══{RESET}")


def cmd_build():
    """Build salt-front (Rust) and salt-opt (C++/MLIR) inside Docker.

    This is the primary build verification command. It will:
    1. Build salt-front via cargo build --release
    2. Build salt-opt via cmake + ninja with LLVM 21/MLIR
    3. Report build success or failure with error context
    """
    if not _image_exists():
        print(f"{YELLOW}Docker image '{IMAGE_NAME}' not found — building...{RESET}")
        cmd_image()

    print(f"{GREEN}═══ RED/GREEN Gate: Building in Docker (LLVM 21) ═══{RESET}")

    script = """
set -euo pipefail

echo "══ Step 1/2: Building salt-front (Rust) ══"
cd /workspace/salt-front
cargo build --release 2>&1
echo ""
echo "✅ salt-front: OK"

echo ""
echo "══ Step 2/2: Building salt-opt (C++/MLIR/LLVM 21) ══"
cd /workspace/salt
rm -rf build_linux
mkdir -p build_linux && cd build_linux
cmake .. -G Ninja \
    -DCMAKE_BUILD_TYPE=Release \
    -DMLIR_DIR=__MLIR_DIR__ \
    -DLLVM_DIR=__LLVM_DIR__ \
    2>&1
ninja salt-opt 2>&1
echo ""
echo "✅ salt-opt: OK"
echo ""
echo "══════════════════════════════════════════════════"
echo "  BUILD RESULT: GREEN ✅"
echo "══════════════════════════════════════════════════"
""".replace("__MLIR_DIR__", MLIR_DIR).replace("__LLVM_DIR__", LLVM_DIR)

    result = subprocess.run(
        ["docker", "run", "--rm",
         "-v", f"{WORKSPACE_ROOT}:/workspace",
         "-w", "/workspace",
         IMAGE_NAME, "bash", "-c", script],
    )

    if result.returncode != 0:
        print(f"\n{RED}══════════════════════════════════════════════════{RESET}")
        print(f"{RED}  BUILD RESULT: RED ❌{RESET}")
        print(f"{RED}══════════════════════════════════════════════════{RESET}")
        sys.exit(1)


def cmd_test():
    """Build all components and run Rust test suites inside Docker.

    Runs:
    - cargo test for salt-front (compiler frontend)
    - cargo test for salt-lsp (language server)
    """
    if not _image_exists():
        print(f"{YELLOW}Docker image '{IMAGE_NAME}' not found — building...{RESET}")
        cmd_image()

    print(f"{GREEN}═══ Running Tests in Docker ═══{RESET}")

    script = """
set -euo pipefail

echo "══ Testing salt-front ══"
cd /workspace/salt-front
cargo test --release 2>&1
echo "✅ salt-front tests: PASS"

echo ""
echo "══ Testing salt-lsp ══"
cd /workspace/tools/salt-lsp
cargo test --release 2>&1
echo "✅ salt-lsp tests: PASS"
"""

    _docker_run(script)
    print(f"\n{GREEN}═══ All Tests PASS ═══{RESET}")


def cmd_shell():
    """Drop into an interactive Docker shell for debugging."""
    if not _image_exists():
        print(f"{YELLOW}Docker image '{IMAGE_NAME}' not found — building...{RESET}")
        cmd_image()

    print(f"{CYAN}Entering Docker shell (keuos-dev)...{RESET}")
    _docker_run("bash", interactive=True)


def cmd_status():
    """Report Docker image status and toolchain versions."""
    if not _image_exists():
        print(f"{RED}Docker image '{IMAGE_NAME}' does not exist.{RESET}")
        print(f"Run: python3 tools/docker_build.py image")
        sys.exit(1)

    print(f"{GREEN}Docker image '{IMAGE_NAME}' exists.{RESET}")
    _docker_run("clang --version | head -1 && llc --version | head -1 && cmake --version | head -1 && rustc --version")


# ─── Main ─────────────────────────────────────────────────────────────────────

COMMANDS = {
    "build": cmd_build,
    "test": cmd_test,
    "image": cmd_image,
    "shell": cmd_shell,
    "status": cmd_status,
}

if __name__ == "__main__":
    if len(sys.argv) < 2 or sys.argv[1] not in COMMANDS:
        print(__doc__)
        print(f"Available commands: {', '.join(COMMANDS.keys())}")
        sys.exit(1)

    COMMANDS[sys.argv[1]]()
