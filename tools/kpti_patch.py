import os
import glob
import re

files = glob.glob('kernel/arch/x86/*.S') + glob.glob('kernel/arch/x86_64/*.S')

kpti_enter = """    swapgs
    /* KPTI CR3 Switch to Kernel PML4 */
    push rax
    mov rax, cr3
    mov gs:[128], rax
    mov rax, gs:[64]
    bts rax, 63
    mov cr3, rax
    pop rax"""

kpti_exit = """    /* KPTI CR3 Switch to User PML4 */
    push rax
    mov rax, gs:[128]
    bts rax, 63
    mov cr3, rax
    pop rax
    swapgs"""

for fpath in files:
    if fpath.endswith('syscall_entry_fast.S'): continue
    with open(fpath, 'r') as f:
        content = f.read()

    original = content
    # Replace entry swapgs
    content = re.sub(r'([ \t]+)swapgs\n([ \t]*\.L_[a-zA-Z0-9_]+from_kernel:)', 
                     lambda m: kpti_enter.replace('    ', m.group(1)) + '\n' + m.group(2), 
                     content)

    # Replace exit swapgs
    content = re.sub(r'([ \t]+)swapgs\n([ \t]*\.L_[a-zA-Z0-9_]+return_to_kernel:)', 
                     lambda m: kpti_exit.replace('    ', m.group(1)) + '\n' + m.group(2), 
                     content)

    if content != original:
        # Prepend .section .kpti_trampoline_text, "ax" before the global handler declaration
        # But wait, it's easier to just add it at the top of the file
        if '.kpti_trampoline_text' not in content:
            # find first .global
            lines = content.split('\n')
            for i, line in enumerate(lines):
                if line.startswith('.global ') and '_wrapper' in line:
                    lines.insert(i, '.section .kpti_trampoline_text, "ax"')
                    break
            content = '\n'.join(lines)
        
        with open(fpath, 'w') as f:
            f.write(content)
        print(f"Patched {fpath}")

