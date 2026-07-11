#!/usr/bin/env python3
"""Static analysis: boot.S multiboot register preservation.

Verifies the invariant that EAX (magic) and EBX (info_ptr) from
multiboot entry are correctly preserved through boot and passed
to kmain. Catches the class of bug where boot setup code clobbers
a register that the kmain call still reads from.
"""
import re
import sys
import os

BOOT_S = os.path.join(
    os.path.dirname(os.path.abspath(__file__)),
    '..', 'kernel', 'arch', 'x86', 'boot.S'
)

# Instructions that write to their first operand (Intel syntax)
WRITE_OPS = (
    r'mov|movabs|xor|lea|add|sub|and|or|shr|shl|inc|dec|pop|not|neg'
)
WRITE_RE = re.compile(
    rf'^\s*({WRITE_OPS})\s+(e?b[xp]|rb[xp])\b', re.I
)


def load_lines():
    with open(BOOT_S) as f:
        return f.readlines()


def strip_comment(line):
    idx = line.find('#')
    return line[:idx].strip() if idx >= 0 else line.strip()


def test_magic_saved_at_entry(lines):
    """EAX saved to EBP as first instruction after start:"""
    found_start = False
    for line in lines:
        s = strip_comment(line)
        if s == 'start:':
            found_start = True
            continue
        if found_start and s:
            assert 'mov ebp, eax' in s.lower(), (
                f"First instruction after start: must be "
                f"'mov ebp, eax', got: {s}"
            )
            return
    assert False, "start: label not found"


def test_ebp_not_clobbered(lines):
    """EBP/RBP not written between start: and call kmain"""
    in_range = False
    for i, line in enumerate(lines):
        s = strip_comment(line)
        if s == 'start:':
            in_range = True
            continue
        if in_range and 'call kmain' in s:
            return
        if not in_range or not s:
            continue
        if 'mov ebp, eax' in s.lower():
            continue  # The initial save is allowed
        m = re.match(
            rf'^\s*({WRITE_OPS})\s+(ebp|rbp)\b', s, re.I
        )
        if m:
            assert False, f"Line {i+1}: EBP/RBP clobbered: {s}"
    assert False, "call kmain not found after start:"


def test_ebx_not_clobbered(lines):
    """EBX/RBX not written between start: and call kmain"""
    in_range = False
    for i, line in enumerate(lines):
        s = strip_comment(line)
        if s == 'start:':
            in_range = True
            continue
        if in_range and 'call kmain' in s:
            return
        if not in_range or not s:
            continue
        m = re.match(
            rf'^\s*({WRITE_OPS})\s+(ebx|rbx)\b', s, re.I
        )
        if m:
            assert False, f"Line {i+1}: EBX/RBX clobbered: {s}"
    assert False, "call kmain not found after start:"


def test_kmain_args_correct(lines):
    """kmain called with mov edi,ebp (magic) and mov rsi,rbx (info)"""
    for i, line in enumerate(lines):
        if 'call kmain' not in strip_comment(line):
            continue
        # Check the 5 preceding non-blank lines for the setup
        window = [
            strip_comment(lines[j]).lower()
            for j in range(max(0, i - 5), i)
            if strip_comment(lines[j])
        ]
        has_magic = any('mov edi, ebp' in w or 'mov edi,ebp' in w
                        for w in window)
        has_info = any('mov rsi, rbx' in w or 'mov rsi,rbx' in w
                       for w in window)
        assert has_magic, "kmain magic arg must come from EBP"
        assert has_info, "kmain info_ptr arg must come from RBX"
        return
    assert False, "call kmain not found"


def main():
    lines = load_lines()
    tests = [
        test_magic_saved_at_entry,
        test_ebp_not_clobbered,
        test_ebx_not_clobbered,
        test_kmain_args_correct,
    ]
    failures = 0
    for test in tests:
        try:
            test(lines)
            print(f"  PASS: {test.__doc__}")
        except AssertionError as e:
            print(f"  FAIL: {test.__doc__}")
            print(f"        {e}")
            failures += 1

    print()
    if failures:
        print(f"{failures}/{len(tests)} test(s) failed")
        sys.exit(1)
    print(f"All {len(tests)} tests passed")


if __name__ == '__main__':
    main()
