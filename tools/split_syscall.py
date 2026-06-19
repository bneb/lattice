#!/usr/bin/env python3
"""Split kernel/core/syscall.salt into dispatch + handlers.

Usage:
  python3 tools/split_syscall.py
  python3 tools/runner_qemu.py build   # verify
"""

import re, os, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "kernel", "core", "syscall.salt")
OUT = os.path.join(ROOT, "kernel", "core", "syscall_handlers.salt")
DISPATCH_LINES = 176  # lines 1-176: imports + dispatch + is_valid_user_ptr

def clean_body(body: str) -> str:
    """Replace import-based calls with @no_mangle extern fn names."""
    # Order matters: specific patterns before general ones
    reps = [
        # kernel.core.pmm.xxx (must come before pmm.xxx)
        (r'kernel\.core\.pmm\.alloc\(\)', 'pmm_alloc()'),
        (r'kernel\.core\.pmm\.free\(', 'pmm_free('),
        # kernel.core.memory.xxx
        (r'kernel\.core\.memory\.phys_to_virt_raw\(', 'memory_phys_to_virt_raw('),
        (r'kernel\.core\.memory\.virt_to_phys_raw\(', 'memory_virt_to_phys_raw('),
        # kernel.sys.vfs.xxx
        (r'kernel\.sys\.vfs\.vfs_lookup\(', 'sys_vfs_lookup('),
        # process.xxx calls
        (r'process\.current\(\)', 'process_current()'),
        (r'process\.get_pml4\(', 'process_get_pml4('),
        (r'process\.get_fd_table\(', 'process_get_fd_table('),
        (r'process\.get_brk_base\(', 'process_get_brk_base('),
        (r'process\.get_brk_current\(', 'process_get_brk_current('),
        (r'process\.get_mmap_base\(', 'process_get_mmap_base('),
        (r'process\.set_brk_current\(', 'process_set_brk_current('),
        (r'process\.set_mmap_base\(', 'process_set_mmap_base('),
        (r'process\.get_vma_list_head\(', 'process_get_vma_list_head('),
        (r'process\.get_state\(', 'process_get_state('),
        (r'process\.get_count\(\)', 'process_get_count()'),
        (r'process\.dec_count\(\)', 'process_dec_count()'),
        (r'process\.set_state\(', 'process_set_state('),
        (r'process\.get_kernel_rsp\(', 'process_get_kernel_rsp('),
        (r'process\.get_kernel_stack_top\(', 'process_get_kernel_stack_top('),
        (r'process\.get_parent_pid\(', 'process_get_parent_pid('),
        (r'process\.set_ipc_msg\(', 'process_set_ipc_msg('),
        (r'process\.get_pid\(', 'process_get_pid('),
        (r'process\.get_ipc_sender\(', 'process_get_ipc_sender('),
        (r'process\.get_ipc_msg0\(', 'process_get_ipc_msg0('),
        (r'process\.set_fd_table\(', 'process_set_fd_table('),
        (r'process\.set_current\(', 'process_set_current('),
        # pmm.xxx
        (r'pmm\.alloc\(\)', 'pmm_alloc()'),
        (r'pmm\.free\(', 'pmm_free('),
        # memory.xxx
        (r'memory\.phys_to_virt_raw\(', 'memory_phys_to_virt_raw('),
        (r'memory\.virt_to_phys_raw\(', 'memory_virt_to_phys_raw('),
        # user_paging.xxx
        (r'user_paging\.translate_user_addr\(', 'user_paging_translate_user_addr('),
        (r'user_paging\.get_user_pte\(', 'user_paging_get_user_pte('),
        (r'user_paging\.destroy_user_pml4\(', 'user_paging_destroy_user_pml4('),
        (r'user_paging\.unmap_user_page\(', 'user_paging_unmap_user_page('),
        (r'user_paging\.map_user_page_extern\(', 'map_user_page_extern('),
        # vfs.xxx
        (r'vfs\.vfs_lookup\(', 'sys_vfs_lookup('),
        (r'vfs\.vfs_close\(', 'vfs_vfs_close('),
        # ring_abi.xxx
        (r'pulse\.is_empty\(\)', 'pulse_is_empty()'),
        (r'ring_abi\.destroy_ring\(', 'ring_abi_destroy_ring('),
        (r'ring_abi\.drain_sq\(', 'ring_abi_drain_sq('),
        # serial.xxx — kept as import-based calls (kernel.drivers.serial import)
    ]
    for pat, rep in reps:
        body = re.sub(pat, rep, body)
    # Clean up intermediate forms (e.g., kernel.core.pmm_alloc from order issues)
    body = re.sub(r'kernel\.core\.pmm_alloc\(', 'pmm_alloc(', body)
    body = re.sub(r'kernel\.core\.pmm_free\(', 'pmm_free(', body)
    body = re.sub(r'kernel\.core\.memory_phys_to_virt_raw\(', 'memory_phys_to_virt_raw(', body)
    body = re.sub(r'kernel\.core\.memory_virt_to_phys_raw\(', 'memory_virt_to_phys_raw(', body)
    body = re.sub(r'kernel\.sys\.sys_vfs_lookup\(', 'sys_vfs_lookup(', body)
    body = re.sub(r'vfs\.sys_vfs_lookup\(', 'sys_vfs_lookup(', body)
    return body


def add_no_mangle(body: str) -> str:
    """Add @no_mangle to every handler function called from dispatch."""
    fns = ['sys_write', 'sys_read', 'sys_open', 'sys_brk', 'sys_mmap',
           'sys_ipc_send', 'sys_ipc_recv', 'sys_shm_grant', 'sys_spawn', 'sys_wait',
           'process_exit', 'schedule_next', 'timer_preempt', 'syscall_set_kernel_pml4']
    for fn in fns:
        body = re.sub(rf'^(pub )?fn {fn}\b', f'@no_mangle\npub fn {fn}', body, flags=re.M)
        body = re.sub(rf'^pub fn {fn}\b', f'@no_mangle\npub fn {fn}', body, flags=re.M)
    return body


def main():
    # Read original from git (not current file, which may be already trimmed)
    import subprocess
    result = subprocess.run(['git', 'show', 'HEAD~1:kernel/core/syscall.salt'],
                          capture_output=True, text=True, cwd=ROOT)
    if result.returncode != 0:
        result = subprocess.run(['git', 'show', 'HEAD:kernel/core/syscall.salt'],
                              capture_output=True, text=True, cwd=ROOT)
    lines = result.stdout.split('\n')
    if len(lines) < 200:
        print("Error: could not read original syscall.salt from git")
        sys.exit(1)

    # Extract dispatch (lines 1-176) and body (lines 177+)
    dispatch = '\n'.join(lines[:DISPATCH_LINES])
    body_raw = '\n'.join(lines[DISPATCH_LINES:])

    # Clean and annotate body
    body = clean_body(body_raw)
    body = add_no_mangle(body)

    # Write handlers file
    header = '''package kernel.core.syscall_handlers
use std.core.ptr.Ptr
import kernel.drivers.serial
import kernel.sys.vfs
import kernel.core.memory
extern fn is_valid_user_ptr(ptr: u64, len: u64) -> bool;
extern fn process_current() -> u64;
extern fn process_get_pml4(slot: u64) -> u64;
extern fn process_get_fd_table(slot: u64) -> u64;
extern fn process_get_brk_base(slot: u64) -> u64;
extern fn process_get_brk_current(slot: u64) -> u64;
extern fn process_get_mmap_base(slot: u64) -> u64;
extern fn process_set_brk_current(slot: u64, val: u64);
extern fn process_set_mmap_base(slot: u64, val: u64);
extern fn process_get_vma_list_head(slot: u64) -> u64;
extern fn process_get_state(slot: u64) -> u64;
extern fn process_get_count() -> u64;
extern fn process_dec_count();
extern fn process_set_state(slot: u64, state: u64);
extern fn process_get_kernel_rsp(slot: u64) -> u64;
extern fn process_get_kernel_stack_top(slot: u64) -> u64;
extern fn process_get_parent_pid(slot: u64) -> u64;
extern fn process_set_ipc_msg(slot: u64, sender: u64, msg0: u64, msg1: u64, msg2: u64);
extern fn process_get_pid(slot: u64) -> u64;
extern fn process_get_ipc_sender(slot: u64) -> u64;
extern fn process_get_ipc_msg0(slot: u64) -> u64;
extern fn process_set_fd_table(slot: u64, phys: u64);
extern fn process_set_current(slot: u64);
extern fn pmm_alloc() -> u64;
extern fn pmm_free(page: u64);
extern fn memory_phys_to_virt_raw(phys: u64) -> u64;
extern fn memory_virt_to_phys_raw(virt: u64) -> u64;
extern fn user_paging_translate_user_addr(pml4: u64, vaddr: u64) -> u64;
extern fn user_paging_get_user_pte(pml4: u64, vaddr: u64) -> u64;
extern fn user_paging_destroy_user_pml4(pml4: u64);
extern fn user_paging_unmap_user_page(pml4: u64, vaddr: u64);
extern fn map_user_page_extern(pml4: u64, vaddr: u64, paddr: u64, flags: u64);
extern fn sys_vfs_lookup(path_ptr: u64) -> u64;
extern fn vfs_vfs_close(inode_addr: u64);
extern fn pulse_is_empty() -> bool;
extern fn ring_abi_destroy_ring(slot: u64);
extern fn ring_abi_drain_sq(slot: u64);
extern fn schedule_next();
extern fn proc_context_switch(old_rsp_ptr: u64, new_rsp: u64, new_cr3: u64, new_rsp0: u64);
const PAGE_SIZE: u64 = 4096;
global DISCARD_RSP: [u64; 1] = [0];
'''
    with open(OUT, 'w') as f:
        f.write(header + body)

    # Update syscall.salt: keep only dispatch + add extern fns
    extern_fns = '''extern fn sys_read(fd: u64, buf: u64, len: u64) -> u64;
extern fn sys_write(fd: u64, buf: u64, len: u64) -> u64;
extern fn sys_open(path: u64, flags: u64) -> u64;
extern fn sys_brk(new_brk: u64) -> u64;
extern fn sys_mmap(length: u64, prot: u64) -> u64;
extern fn sys_ipc_send(target_pid: u64, msg0: u64, msg1: u64, msg2: u64) -> u64;
extern fn sys_ipc_recv() -> u64;
extern fn sys_shm_grant(target_pid: u64, src_vaddr: u64, dst_vaddr: u64, num_pages: u64) -> u64;
extern fn sys_spawn(path_ptr: u64, path_len: u64) -> u64;
extern fn sys_wait(target_pid: u64) -> u64;
'''
    with open(SRC, 'w') as f:
        f.write(dispatch + '\n' + extern_fns)

    body_lines = len(body.split('\n'))
    dispatch_lines = len(dispatch.split('\n'))
    print(f"syscall.salt: {dispatch_lines} lines (was {len(lines)})")
    print(f"syscall_handlers.salt: {body_lines + header.count(chr(10))} lines")
    print("Done. Run: python3 tools/runner_qemu.py build")


if __name__ == '__main__':
    main()
