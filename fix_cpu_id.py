import os
import glob

# Files to fix
files = [
    "kernel/benchmarks/sip_ipc_ring.salt",
    "kernel/benchmarks/slab_reclaim_bench.salt",
    "kernel/benchmarks/smp_bench.salt",
    "kernel/core/async_test.salt",
    "kernel/core/flight_recorder.salt",
    "kernel/core/panic.salt",
    "kernel/core/pmm.salt",
    "kernel/core/preempt_test.salt",
    "kernel/core/sched_isolate.salt",
    "kernel/lib/ebr.salt",
    "kernel/mem/slab.salt",
    "kernel/mem/slab_cache.salt",
]

for file_path in files:
    full_path = os.path.join("/Users/kevin/projects/lattice", file_path)
    with open(full_path, "r") as f:
        content = f.read()

    # 1. Add import kernel.core.percpu if not present
    if "import kernel.core.percpu" not in content:
        # Find the first import or package declaration to insert after
        lines = content.split('\n')
        insert_idx = 0
        for i, line in enumerate(lines):
            if line.startswith("import") or line.startswith("package"):
                insert_idx = i + 1
        
        lines.insert(insert_idx, "import kernel.core.percpu")
        content = '\n'.join(lines)
        
    # 2. Remove extern fn get_cpu_id() -> u64;
    content = content.replace("extern fn get_cpu_id() -> u64;", "")
    
    # 3. Replace get_cpu_id( with percpu.get_cpu_id(
    content = content.replace("get_cpu_id(", "percpu.get_cpu_id(")
    
    # Write back
    with open(full_path, "w") as f:
        f.write(content)
    
    print(f"Fixed {file_path}")
