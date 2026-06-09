import json
import os

class Reporter:
    def __init__(self, results: dict):
        self.results = results
        
    def write_json(self, path: str):
        with open(path, "w") as f:
            json.dump(self.results, f, indent=2)

    def write_markdown_e2e(self, path: str):
        lines = [
            "# Rigorous E2E Benchmarks",
            "",
            "Automated, multi-iteration, high-fidelity benchmarks across the KeuOS macro-applications.",
            "",
            "## Microbenchmarks (Algorithms & Data Structures)",
            "",
            "| Target | Binary Size (KB) | Peak RSS (KB) | execution_time_s |",
            "|---|---|---|---|"
        ]
        
        targets = []
        for bench_name, langs in self.results.items():
            for lang, metrics in langs.items():
                if metrics["ret_code"] == 0 and metrics["time_s"] > 0:
                    targets.append((bench_name, lang, metrics))
        
        targets.sort(key=lambda x: (x[0], x[1]))
        
        for bench, lang, m in targets:
            lang_display = "C" if lang == "c" else ("Rust" if lang == "rs" else "Salt")
            name = f"{bench} ({lang_display})"
            size = f"{m['binary_size_kb']:.1f}"
            rss = f"{m['peak_rss_kb']}"
            time = f"{m['time_s']:.2f}"
            lines.append(f"| **{name}** | {size} | {rss} | {time} |")
            
        with open(path, "w") as f:
            f.write("\n".join(lines) + "\n")
