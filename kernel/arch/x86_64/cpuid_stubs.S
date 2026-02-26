.intel_syntax noprefix

# =============================================================================
# cpuid_stubs.S — CPUID instruction wrappers for Salt
# =============================================================================

# cpuid_leaf1_ecx() -> u64
# Returns ECX from CPUID leaf 1 (feature flags, bit 31 = hypervisor)
.global cpuid_leaf1_ecx
cpuid_leaf1_ecx:
    push rbx
    mov eax, 1
    cpuid
    mov rax, rcx
    pop rbx
    ret

# cpuid_hypervisor_id() -> u64
# Returns EBX from CPUID leaf 0x40000000 (first 4 bytes of hypervisor brand)
# KVM: "KVMK" = 0x4b564d4b
# Microsoft: "Micr" = 0x7263694d
.global cpuid_hypervisor_id
cpuid_hypervisor_id:
    push rbx
    mov eax, 0x40000000
    cpuid
    mov rax, rbx
    pop rbx
    ret
