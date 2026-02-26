---
description: Prefer Python scripts over shell commands for complex operations
---

# Shell Command Best Practices

When executing complex multi-step operations, file manipulations, or batch processing:

1. **Always prefer Python scripts** over complex shell one-liners, heredocs, or for-loops
2. Write the Python script to `/tmp/<descriptive_name>.py` first using `write_to_file`
3. Then execute it with `python3 /tmp/<script>.py`

## Why

- Shell heredocs hang in zsh when piped through tool interfaces
- Complex `for` loops with subshells (`$()`) break in zsh
- Python is deterministic, debuggable, and handles string escaping natively

## Examples

### ❌ Bad: Shell heredoc
```bash
cat >> file.salt << 'EOF'
@no_mangle
pub fn wrapper() { inner(); }
EOF
```

### ❌ Bad: Complex shell for-loop  
```bash
for f in $(find . -name "*.salt"); do grep "pattern" "$f"; done
```

### ✅ Good: Python script
```python
#!/usr/bin/env python3
with open('file.salt', 'a') as f:
    f.write('\n@no_mangle\npub fn wrapper() { inner(); }\n')
```
