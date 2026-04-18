#!/usr/bin/env python3
import sys
import re

def fnv1a(data):
    hash = 2166136261
    for byte in data:
        hash ^= byte
        hash = (hash * 16777619) & 0xFFFFFFFF
    return hash

def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <css_file>")
        sys.exit(1)

    with open(sys.argv[1], 'r') as f:
        css = f.read()

    # Very simple regex-based CSS parser to match Prisimi's capabilities
    # 1. Strip comments
    css = re.sub(r'/\*.*?\*/', '', css, flags=re.DOTALL)
    
    # 2. Find rules
    rules = re.findall(r'([^{]+)\s*\{(.*?)\}', css, flags=re.DOTALL)
    
    print("== CSS LEXER IR ")
    for prelude, block in rules:
        prelude = prelude.strip()
        if not prelude: continue
        
        # Determine hash and specificity
        if prelude.startswith('.'):
            hash = fnv1a(prelude[1:].encode('utf-8'))
            spec = 10
        elif prelude.startswith('#'):
            hash = fnv1a(prelude[1:].encode('utf-8'))
            spec = 100
        else:
            hash = fnv1a(prelude.encode('utf-8'))
            spec = 1
            
        display = 255
        flex_grow = -1
        
        # Parse declarations
        decls = re.findall(r'([^:;]+)\s*:\s*([^;\}]+)', block)
        for name, value in decls:
            name = name.strip().lower()
            value = value.strip().lower()
            
            if name == "display":
                if value == "none": display = 0
                elif value == "block": display = 1
                elif value == "flex": display = 2
                elif value == "inline": display = 3
            elif name == "flex-grow":
                try:
                    flex_grow = int(value)
                except:
                    pass
        
        print(f"RULE H={hash} S={spec} D={display} FG={flex_grow}")

if __name__ == "__main__":
    main()
