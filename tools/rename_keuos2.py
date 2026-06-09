import os

replacements = [
    ("KEUOS", "KEUOS"),
    ("KeuOS", "KeuOS"),
    ("keuos", "keuos")
]

for root, dirs, files in os.walk("."):
    if ".git" in root or "target" in root or ".venv" in root: continue
    
    # Rename files if they contain keuos in any case
    for f in files:
        if "keuos" in f.lower():
            old_path = os.path.join(root, f)
            new_f = f.replace("keuos", "keuos").replace("KeuOS", "KeuOS").replace("KEUOS", "KEUOS")
            new_path = os.path.join(root, new_f)
            os.rename(old_path, new_path)
            print(f"Renamed file {old_path} -> {new_path}")

# Walk again after renames
for root, dirs, files in os.walk("."):
    if ".git" in root or "target" in root or ".venv" in root: continue
    for f in files:
        if not f.endswith((".salt", ".c", ".rs", ".md", ".py", ".sh", ".json", "Makefile", "toml", "m", "h", "html")): continue
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

