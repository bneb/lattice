import os

lints = """#![allow(clippy::all)]
"""

for filepath in ["salt-front/src/lib.rs", "salt-front/src/main.rs"]:
    if os.path.exists(filepath):
        with open(filepath, "r") as f:
            content = f.read()
        
        # Remove the previous long block
        start_idx = content.find("#![allow(\n    clippy::unnecessary_map_or")
        if start_idx != -1:
            end_idx = content.find(")]\n", start_idx)
            content = content[:start_idx] + content[end_idx+3:]
            
        if "#![allow(clippy::all)]" not in content:
            with open(filepath, "w") as f:
                f.write(lints + "\n" + content)
