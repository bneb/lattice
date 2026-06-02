#!/bin/bash
set -e

WORKSPACE_ROOT=$(cd "$(dirname "$0")" && pwd)
cd "$WORKSPACE_ROOT"

export PYTHONPATH="$WORKSPACE_ROOT"

# Forward arguments to main.py
python3 -m tools.bench_infra.main --all
