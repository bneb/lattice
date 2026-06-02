import sys

with open("/Users/kevin/.gemini/antigravity/brain/26d5d608-d300-46cb-8f05-daeb642a0675/task.md", "r") as f:
    content = f.read()

content = content.replace("- [x] Integrate VFS layer with the core Reactor event loop. & VirtIO", "- [x] Integrate VFS layer with the core Reactor event loop.\n\n## Phase 3: Network Stack (`netd`) & VirtIO")

with open("/Users/kevin/.gemini/antigravity/brain/26d5d608-d300-46cb-8f05-daeb642a0675/task.md", "w") as f:
    f.write(content)
