import argparse
import os
import sys
from .runners import CRunner, RustRunner, SaltRunner, NodeRunner, PyTorchRunner
from .reporters import Reporter
from .app_benchmarks import run_ml_benchmark, run_echo_benchmark, append_app_reports

def get_benchmarks(specific_bench=None):
    if specific_bench:
        return [specific_bench]
    return [
        'binary_trees', 'fannkuch', 'forest', 'hashmap_bench', 'lru_cache', 'trie',
        'buffered_writer_perf', 'fstring_perf', 'writer_perf', 'matmul', 'sieve',
        'longest_consecutive', 'sudoku_solver', 'window_access', 'vector_add',
        'fib', 'global_counter', 'binary_tree_path', 'string_hashmap_bench',
        'bitwise', 'trapping_rain_water', 'merge_sorted_lists',
        'bench_ecs_epoch_reclaim', 'bench_ecs_event_pipeline', 'bench_ecs_ipc_resolve',
        'bench_ecs_lookup', 'bench_ecs_scheduler', 'bench_ecs_spawn',
        'chase_lev_bench', 'coverage_gap', 'coverage_push', 'dll_salt',
        'http_parser_bench', 'promotion_matrix', 'sliding_window_bench',
        'syntactic_chaos', 'yield_validation'
    ]

def main():
    parser = argparse.ArgumentParser(description="KeuOS Benchmark Infrastructure")
    parser.add_argument('--bench', type=str, help='Specific benchmark to run')
    parser.add_argument('--all', action='store_true', help='Run all benchmarks')
    parser.add_argument('--iterations', type=int, default=3)
    parser.add_argument('--out-json', type=str, default="benchmark_results.json")
    parser.add_argument('--out-md-e2e', type=str, default="BENCHMARKS_E2E.md")
    
    args = parser.parse_args()
    if not args.bench and not args.all:
        print("Specify --bench <name> or --all")
        sys.exit(1)

    workspace_root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    bench_dir = os.path.join(workspace_root, "benchmarks")
    
    benches = get_benchmarks(args.bench)
    
    runners = {
        "c": CRunner(workspace_root),
        "rs": RustRunner(workspace_root),
        "salt": SaltRunner(workspace_root),
        "py": PyTorchRunner(workspace_root)
    }
    
    print(f"{'Benchmark':<25} | {'C (ms)':<10} | {'Rust (ms)':<10} | {'Salt (ms)':<10} | {'PyTorch/Node (ms)':<18} | {'Parity':<10}")
    print("-" * 75)
    
    results = {}
    
    for bench in benches:
        results[bench] = {}
        row_display = {}
        for ext, runner in runners.items():
            src_path = os.path.join(bench_dir, f"{bench}.{ext}")
            metrics = runner.run(src_path, iterations=args.iterations)
            
            results[bench][ext] = {
                "time_s": metrics.time_s,
                "peak_rss_kb": metrics.peak_rss_kb,
                "binary_size_kb": metrics.binary_size_kb,
                "ret_code": metrics.ret_code
            }
            
            if metrics.ret_code == 0 and os.path.exists(src_path):
                row_display[ext] = f"{metrics.time_s * 1000:.2f}"
            else:
                row_display[ext] = "N/A"
                
        parity = "OK"
        if results[bench]["c"]["ret_code"] == 0 and results[bench]["salt"]["ret_code"] == 0:
            if results[bench]["c"]["ret_code"] != results[bench]["salt"]["ret_code"]:
                parity = "FAIL"
                
        c_disp = row_display.get("c", "N/A")
        rs_disp = row_display.get("rs", "N/A")
        salt_disp = row_display.get("salt", "N/A")
        py_disp = row_display.get("py", "N/A")
        print(f"{bench:<25} | {c_disp:<10} | {rs_disp:<10} | {salt_disp:<10} | {py_disp:<18} | {parity:<10}")

    rep = Reporter(results)
    json_path = os.path.join(bench_dir, args.out_json)
    md_path = os.path.join(bench_dir, args.out_md_e2e)
    rep.write_json(json_path)
    rep.write_markdown_e2e(md_path)
    
    if args.all:
        ml_out = run_ml_benchmark(workspace_root)
        echo_out = run_echo_benchmark(workspace_root)
        append_app_reports(md_path, ml_out, echo_out)

    print(f"Reports generated in {bench_dir}")

if __name__ == "__main__":
    main()
