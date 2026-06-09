import sys
content = open("kernel/arch/x86_64/syscall_entry_fast.S").read()

def inject_after(marker, letter, char_hex):
    global content
    asm = marker + f"""
    mov dx, 0x3F8
    mov al, {char_hex}
    out dx, al
"""
    # only replace first occurrence
    content = content.replace(marker, asm, 1)

inject_after("mov rsp, gs:[96]", "D", "0x44")
inject_after("call handle_syscall", "E", "0x45")
inject_after("add rsp, 8", "F", "0x46")

def inject_before(marker, letter, char_hex):
    global content
    asm = f"""
    mov dx, 0x3F8
    mov al, {char_hex}
    out dx, al
""" + marker
    content = content.replace(marker, asm, 1)

inject_before("sysretq\n", "G", "0x47")

open("kernel/arch/x86_64/syscall_entry_fast.S", "w").write(content)
