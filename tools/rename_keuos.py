import os

replacements = [
    ("IsolatedArena", "IsolatedArena"),
    ("isolated_arena", "isolated_arena"),
    ("Hardware-Fenced Reclaim", "Hardware-Fenced Reclaim"),
    ("KeuOS Microkernel", "KeuOS Microkernel"),
    ("KeuOS", "KeuOS"),
    ("System ABI", "System ABI"),
    ("KeuOS Distribution", "KeuOS Distribution"),
    ("KeuOS Foundation", "KeuOS Foundation"),
    ("KeuOS Architecture", "KeuOS Architecture"),
    ("KeuOS Ring 3", "KeuOS Ring 3"),
    ("KeuOS C2", "KeuOS C2"),
    ("KeuOS Scheduler", "KeuOS Scheduler"),
    ("KeuOS", "KeuOS"),
    ("keuos", "keuos")
]

for root, dirs, files in os.walk("."):
    if ".git" in root or "target" in root or ".venv" in root: continue
    
    # Rename files if they contain keuos
    for f in files:
        if "keuos" in f.lower():
            old_path = os.path.join(root, f)
            new_f = f.replace("keuos", "keuos").replace("KeuOS", "KeuOS")
            new_path = os.path.join(root, new_f)
            os.rename(old_path, new_path)
            print(f"Renamed file {old_path} -> {new_path}")

# Walk again after renames
for root, dirs, files in os.walk("."):
    if ".git" in root or "target" in root or ".venv" in root: continue
    for f in files:
        if not f.endswith((".salt", ".c", ".rs", ".md", ".py", ".sh", ".json", "Makefile", "toml")): continue
        filepath = os.path.join(root, f)
        
        try:
            with open(filepath, "r") as fh:
                content = fh.read()
            
            new_content = content
            for old, new in replacements:
                new_content = new_content.replace(old, new)
            
            if new_content != content:
                with open(filepath, "w") as fh:
                    fh.write(new_content)
                print(f"Updated content in {filepath}")
        except Exception:
            pass

