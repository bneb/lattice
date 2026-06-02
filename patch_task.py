import sys

with open("/Users/kevin/.gemini/antigravity/brain/26d5d608-d300-46cb-8f05-daeb642a0675/task.md", "r") as f:
    lines = f.readlines()

new_lines = []
skip = False
for line in lines:
    if line.startswith("- `[x]` Review `kernel/keuos/hw/pcie_enum.salt`") or line.startswith("## Verification"):
        skip = True
    
    if skip and line.startswith("## Phase 1:"):
        skip = False
        
    if not skip:
        new_lines.append(line)

with open("/Users/kevin/.gemini/antigravity/brain/26d5d608-d300-46cb-8f05-daeb642a0675/task.md", "w") as f:
    f.writelines(new_lines)

